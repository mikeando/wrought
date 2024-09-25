use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use anyhow::{anyhow,Context};
use backend::{Backend, DummyBackend};
use bridge::{Bridge, DummyBridge};
use clap::{Parser, Subcommand};

pub mod backend;
pub mod binary16;
pub mod bridge;
pub mod event_log;
pub mod events;
pub mod metadata;
pub mod scripting_luau;
pub mod fs_utils;

use binary16::ContentHash;
use event_log::{DummyEventLog, EventLog};
use events::{Event, EventGroup};
use events::{EventType, GetMetadataEvent, SetMetadataEvent, WriteFileEvent};

use metadata::MetadataEntry;
use metadata::MetadataKey;
use scripting_luau::run_script;

pub struct Wrought {
    backend: Arc<Mutex<dyn Backend>>,
}

impl Wrought {
    pub fn begin_script<N, F>(&mut self, name: N, f: F)
    where
        N: Into<String>,
        F: FnOnce(&mut MicroService) -> anyhow::Result<()>,
    {
        let mut m = MicroService::new(self.backend.clone());
        println!("Wrought::begin_script - runnning {}", name.into());
        f(&mut m).unwrap();
        eprintln!("Wrough::begin_script - logged events =\n{:#?}", m.events);
    }

    pub fn new(backend: Arc<Mutex<dyn Backend>>) -> Wrought {
        Wrought { backend }
    }
}

pub struct MicroService {
    pub events: Vec<Event>,
    pub backend: Arc<Mutex<dyn Backend>>,
}

impl MicroService {
    pub fn new(backend: Arc<Mutex<dyn Backend>>) -> MicroService {
        MicroService {
            events: vec![],
            backend,
        }
    }

    pub fn get_metadata<P: AsRef<Path>, K: Into<MetadataKey>>(
        &mut self,
        path: P,
        key: K,
    ) -> anyhow::Result<Option<MetadataEntry>> {
        let key = key.into();
        let value = self
            .backend
            .lock()
            .unwrap()
            .get_metadata(path.as_ref(), &key)?;

        let event = Event {
            id: 0,
            group_id: 0,
            event_type: EventType::GetMetadata(GetMetadataEvent {
                path: path.as_ref().to_path_buf(),
                key,
                value: value.clone(),
            }),
        };
        self.events.push(event);
        Ok(value)
    }

    pub fn set_metadata<P: AsRef<Path>, K: Into<MetadataKey>, V: Into<MetadataEntry>>(
        &mut self,
        path: P,
        key: K,
        value: V,
    ) -> anyhow::Result<()> {
        let key = key.into();
        let value = Some(value.into());
        let before_value =
            self.backend
                .lock()
                .unwrap()
                .set_metadata(path.as_ref(), &key, &value)?;
        let event = Event {
            id: 0,
            group_id: 0,
            event_type: EventType::SetMetadata(SetMetadataEvent {
                path: path.as_ref().to_path_buf(),
                key,
                before_value,
                after_value: value.clone(),
            }),
        };
        self.events.push(event);
        Ok(())
    }

    pub fn write_file<P: AsRef<Path>>(&mut self, path: P, value: &[u8]) -> anyhow::Result<()> {
        let (before_hash, after_hash) = self
            .backend
            .lock()
            .unwrap()
            .write_file(path.as_ref(), value)?;
        self.events.push(Event {
            id: 0,
            group_id: 0,
            event_type: EventType::WriteFile(WriteFileEvent {
                path: path.as_ref().to_path_buf(),
                before_hash,
                after_hash: Some(after_hash),
            }),
        });
        Ok(())
    }
}

pub fn hello_world(wrought: &mut Wrought) {
    wrought.begin_script("hello world", |m: &mut MicroService| {
        if let Some(md) = m.get_metadata("index.md", "name")? {
            m.write_file(
                "hello.txt",
                format!("greetings, {}", md.as_string()).as_bytes(),
            )?;
        } else {
            m.set_metadata("index.md", "name", "Unknown")?;
            m.write_file("hello.txt", "greetings!".as_bytes())?;
        }
        Ok(())
    });
}

/// Search for a pattern in a file and display the lines that contain it.
#[derive(Parser)]
struct Cli {
    /// pick a different project root
    #[arg(long)]
    project_root: Option<PathBuf>,

    /// Command to run
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    FileStatus(FileStatusCmd),
    Init(InitCmd),
    RunScript(RunScriptCmd),
    Status(StatusCmd),
    HelloWorld,
}

#[derive(Debug, Parser)]
struct InitCmd {
    path: PathBuf,
    #[arg(long)]
    package: String,
}

#[derive(Debug, Parser)]
struct StatusCmd {}

#[derive(Debug, Parser)]
struct FileStatusCmd {
    path: PathBuf,
}

#[derive(Debug, Parser)]
struct RunScriptCmd {
    script_name: String,
}

fn find_first_existing_parent(starting_dir: &Path) -> anyhow::Result<Option<PathBuf>> {
    let mut current_dir = starting_dir;

    loop {
        if current_dir.exists() {
            return Ok(Some(current_dir.to_path_buf()));
        }

        let parent_dir = current_dir.parent();
        match parent_dir {
            Some(parent) => current_dir = parent,
            None => return Ok(None),
        }
    }
}

fn find_marker_dir(starting_dir: &Path, marker: &str) -> anyhow::Result<Option<PathBuf>> {
    let starting_dir = starting_dir.canonicalize()?;
    let mut current_dir: &Path = &starting_dir;

    loop {
        let marker_path = current_dir.join(marker);
        if marker_path.is_dir() {
            return Ok(Some(current_dir.to_path_buf()));
        }

        let parent_dir = current_dir.parent();
        match parent_dir {
            Some(parent) => current_dir = parent,
            None => return Ok(None),
        }
    }
}

fn cmd_init(cmd: &InitCmd) -> anyhow::Result<()> {
    let path = &cmd.path;

    // Check the target is not already in a project.
    let check = || -> anyhow::Result<Option<PathBuf>> {
        let existing_parent =
            find_first_existing_parent(path).context("in find_first_existing_parent")?;
        let Some(existing_parent) = existing_parent else {
            return Ok(None);
        };
        find_marker_dir(&existing_parent, ".wrought").context("in find_marker_dir")
    };

    if let Some(parent_path) = check().unwrap() {
        panic!(
            "Path '{}' is part of project with root '{}'",
            path.display(),
            parent_path.display()
        );
    }

    // TODO: Make this configurable.
    let src_package_dir = PathBuf::from("./resources/packages/");
    let project_package_dir = path.join(".wrought").join("packages");

    fs::create_dir_all(path).unwrap();
    fs::create_dir_all(path.join(".wrought")).unwrap();
    DummyEventLog::init(path.join(".wrought").join("wrought.db")).unwrap();
    fs::create_dir_all(&project_package_dir).unwrap();

    let project_package = project_package_dir.join(&cmd.package);
    fs::create_dir_all(&project_package).unwrap();

    fs_utils::copy_dir_all_with_filters(src_package_dir.join(&cmd.package), &project_package, |_,_| true, |_,_| true)?;

    // Now if there is an init script we should run it.
    println!("Running init scripts");

    let bridge = create_bridge(path)?;

    if project_package.join("init.luau").is_file() {
        run_script(bridge.clone(), &project_package.join("init.luau"))?;
        // TODO: Does this belong in the bridge?
        let event_log = create_event_log(path).unwrap();
        if let Some(event_group) = bridge.lock().unwrap().get_event_group() {
            event_log
                .lock()
                .unwrap()
                .add_event_group(&event_group)
                .unwrap();
        };
    } else {
        println!(
            "No init script at '{}'",
            project_package.join("init.luau").display()
        );
    }
    Ok(())
}

#[derive(Debug)]
struct PackageStatus {

}

impl PackageStatus {
    pub fn read_from(p: &Path) -> anyhow::Result<PackageStatus> {
        Err(anyhow!("PackageStatus::read({}) not yet implemented", p.display()))
    }
}

struct Package {
    path: PathBuf,
}

impl Package {
    fn statuses(&self) -> Vec<anyhow::Result<PackageStatus>> {
        let status_dir = self.path.join("status");
        let mut result = vec![];
        let rd = match fs::read_dir(&status_dir){
            Ok(rd) => rd,
            Err(e) => {
                return vec![ Err(e).with_context(|| format!("reading directory {:?}", status_dir))];
            },
        };
        for entry in rd {
            let de = match entry {
                Ok(de) => de,
                Err(e) => {
                    result.push(Err(e).with_context(|| format!("getting directory entry from {:?}", status_dir)));
                    continue;
                },
            };
            let md = match de.metadata() {
                Ok(md) => md,
                Err(e) => {
                    result.push(Err(e).with_context(|| format!("getting metadata for {:?}", de.path())));
                    continue;
                },
            };
            if (!md.is_file()) {
                result.push(Err(anyhow!("status entry {:?} is not a file", de.path())));
                continue;
            }

            result.push(PackageStatus::read_from(&de.path()));
        }
        result
    } 
}

struct PackageDirectory {
    path: PathBuf,
}

impl PackageDirectory {
    fn packages(&self) -> Vec<anyhow::Result<Package>> {
        let mut result = vec![];
        let rd = match fs::read_dir(&self.path){
            Ok(rd) => rd,
            Err(e) => {
                return vec![ Err(e).with_context(|| format!("reading directory .wrought/packages"))];
            },
        };
        for entry in rd {
            let de = match entry {
                Ok(de) => de,
                Err(e) => {
                    result.push(Err(e).with_context(|| format!("getting directory entry")));
                    continue;
                },
            };
            let md = match de.metadata() {
                Ok(md) => md,
                Err(e) => {
                    result.push(Err(e).with_context(|| format!("getting metadata for {:?}", de.path())));
                    continue;
                },
            };
            if (!md.is_dir()) {
                result.push(Err(anyhow!("package directory entry {:?} is not a directory", de.path())));
                continue;
            }
            result.push(Ok(Package{path: de.path()}));
        }
        result
    }
}



fn cmd_status(project_root: &Path, _cmd: StatusCmd) -> anyhow::Result<()> {
    let package_dir = PackageDirectory { path: project_root.join(".wrought").join("packages") };
    let packages = package_dir.packages();
    let package_statuses: Vec<_> = packages.into_iter().flat_map(|package| {
        let v = match package {
            Ok(v) => v,
            Err(e) => return vec![Err(e)],
        };
        v.statuses()
    }).collect();

    for status in package_statuses {
        match status {
            Ok(status) => eprintln!("{:?}", status),
            Err(e) => eprintln!("ERROR: {:?}", e),
        }
    }

    Ok(())
}

fn cmd_run_script(
    bridge: Arc<Mutex<dyn Bridge>>,
    project_root: &Path,
    cmd: RunScriptCmd,
) -> anyhow::Result<()> {
    let script_path = project_root
        .join(".wrought")
        .join("packages")
        .join(cmd.script_name);
    run_script(bridge, &script_path)?;
    Ok(())
}

pub fn create_backend(path: &Path) -> anyhow::Result<Arc<Mutex<dyn Backend>>> {
    Ok(Arc::new(Mutex::new(DummyBackend {
        root: path.canonicalize()?,
    })))
}

pub fn create_event_log(path: &Path) -> anyhow::Result<Arc<Mutex<dyn EventLog>>> {
    Ok(Arc::new(Mutex::new(
        DummyEventLog::open(path.join(".wrought").join("wrought.db")).unwrap(),
    )))
}

pub fn create_bridge(path: &Path) -> anyhow::Result<Arc<Mutex<dyn Bridge>>> {
    let backend = create_backend(path)?;
    Ok(Arc::new(Mutex::new(DummyBridge {
        root: path.canonicalize()?,
        backend,
        event_group: EventGroup::empty(),
    })))
}

fn main() {
    let args = Cli::parse();

    // Have to handle Init differntly as it doesn't care about the project_root already
    // existing etc.
    if let Command::Init(cmd) = &args.command {
        cmd_init(cmd).unwrap();
        return;
    }

    // Check the project_root exists
    let project_root = match &args.project_root {
        Some(p) => {
            if !p.join(".wrought").is_dir() {
                panic!("specified project root {} has no .wrought subdirectory - it is not a valid root", p.display());
            }
            p.clone()
        }
        None => match find_marker_dir(&PathBuf::from("."), ".wrought") {
            Ok(Some(p)) => p,
            Ok(None) => panic!("Unable to find project root for current directory"),
            Err(e) => panic!("Error looking for project root: {}", e),
        },
    };
    eprintln!("Using project root: '{}'", project_root.display());

    match args.command {
        Command::FileStatus(cmd) => {
            let backend = DummyEventLog::open("wrought.db").unwrap();
            let status = get_single_file_status(&backend, &cmd.path).unwrap();
            print_single_file_status(&status);
        }
        Command::HelloWorld => {
            let backend = create_backend(&project_root).unwrap();
            let mut w = Wrought::new(backend);
            hello_world(&mut w);
        }
        Command::Status(cmd) => {
            cmd_status(&project_root, cmd).unwrap();
        }
        Command::RunScript(cmd) => {
            let bridge = create_bridge(&project_root).unwrap();
            cmd_run_script(bridge.clone(), &project_root, cmd).unwrap();
            let event_log = create_event_log(&project_root).unwrap();
            if let Some(event_group) = bridge.lock().unwrap().get_event_group() {
                event_log
                    .lock()
                    .unwrap()
                    .add_event_group(&event_group)
                    .unwrap();
            };
        }
        Command::Init(_) => unreachable!("`init` should already have been handled"),
    }
    // TODO: Should the bridge had access to this?
}

// Things th emain app needs to be able to do.
// Get file status
//   * single file
//   * whole project / directory
//   - stale - one of the deps has changed
//   - modified - doesn't match last write state
// Ability to run a script or plugin
// Low level cmds to mark a file as OK, get from content store by hash, get/set metadata
// Check state of previous script run.
// Get file history

// Where do we store state/histort? There advantages to storing it as part of the filesystem,
// or in a sqlite database. So perhaps we need to abstract the idea of the backend?

pub fn calculate_file_hash(p: &Path) -> anyhow::Result<Option<ContentHash>> {
    // TODO: Handle errors other than p not existing better
    if !p.exists() {
        return Ok(None);
    }
    let content = std::fs::read(p)?;
    Ok(Some(ContentHash::from_content(&content)))
}

#[derive(Debug)]
pub struct SingleFileStatusResult {
    path: PathBuf,
    status: SingleFileStatus,
}

#[derive(Debug)]
enum SingleFileStatus {
    Untracked,
    TrackedFileStatus(TrackedFileStatus),
}

#[derive(Debug)]
struct TrackedFileInput {
    path: PathBuf,
    tracked_hash: Option<ContentHash>,
    current_hash: Option<ContentHash>,
}

#[derive(Debug)]
struct TrackedFileStatus {
    current_hash: Option<ContentHash>,
    tracked_hash: Option<ContentHash>,
    inputs: Vec<TrackedFileInput>,
    command: String,
    // Was the change set that produced this file, the most recent run of command?
    is_most_recent_run: bool,
}

impl TrackedFileStatus {
    pub fn changed(&self) -> bool {
        self.current_hash != self.tracked_hash
    }

    pub fn stale(&self) -> bool {
        for input in &self.inputs {
            if input.current_hash != input.tracked_hash {
                return true;
            }
        }
        false
    }
}

pub fn get_single_file_status<L: EventLog>(
    event_log: &L,
    p: &Path,
) -> anyhow::Result<SingleFileStatusResult> {
    // To get the file status we need to know the last write to it - which should return a hash
    // and the change-set-id that it was last changed in.
    // We can then compare the hash of the file with that in the change-set to determine if it has changed,
    // and compare the hash of all the inputs to determine if it is stale.

    let Some(event) = event_log.get_last_write_event(p)? else {
        // TODO: Do we want to differentiate between Untracked and doesn't exist locally,
        //       and untracked and does exist locally?
        return Ok(SingleFileStatusResult {
            path: p.to_owned(),
            status: SingleFileStatus::Untracked,
        });
    };

    let EventType::WriteFile(write_event) = event.event_type else {
        unreachable!("get_last_write_event returned a non WriteFile event!");
    };

    let current_hash = calculate_file_hash(p)?;

    let Some(event_group) = event_log.get_event_group(event.group_id)? else {
        unreachable!("get_last_write_event returned an event with invalid group_id");
    };

    let mut inputs = vec![];
    for e in &event_group.events {
        match &e.event_type {
            EventType::ReadFile(read_file_event) => {
                let path = read_file_event.path.clone();
                let current_hash = calculate_file_hash(&path)?;
                inputs.push(TrackedFileInput {
                    path,
                    tracked_hash: read_file_event.hash.clone(),
                    current_hash,
                });
            }
            _ => {}
        }
    }

    let t = TrackedFileStatus {
        current_hash,
        tracked_hash: write_event.after_hash,
        inputs,
        command: event_group.command,
        is_most_recent_run: event_group.is_most_recent_run,
    };

    Ok(SingleFileStatusResult {
        path: p.to_owned(),
        status: SingleFileStatus::TrackedFileStatus(t),
    })
}

pub fn print_single_file_status(result: &SingleFileStatusResult) {
    dbg!(&result);
    match &result.status {
        SingleFileStatus::Untracked => {
            println!("Untracked");
        }
        SingleFileStatus::TrackedFileStatus(t) => {
            let mut something_printed = false;
            if t.changed() {
                println!("Changed");
                something_printed = true;
            }
            if t.stale() {
                println!("Stale");
                something_printed = true;
            }
            if !something_printed {
                println!("OK")
            }
        }
    }
}

pub mod api {

    // Declare the extern functions that will be provided by the host
    #[link(wasm_import_module = "env")]
    extern "C" {
        // Returns a "descriptor"
        fn wrought_read_file(path_ptr: *const u8, path_len: usize) -> usize;
        fn wrought_write_file(
            path_ptr: *const u8,
            path_len: usize,
            content_ptr: *const u8,
            content_len: usize,
        ) -> usize;
        fn wrought_get_metadata(
            path_ptr: *const u8,
            path_len: usize,
            key_ptr: *const u8,
            key_len: usize,
        ) -> usize;
        fn wrought_set_metadata(
            path_ptr: *const u8,
            path_len: usize,
            key_ptr: *const u8,
            key_len: usize,
            value_ptr: *const u8,
            value_len: usize,
        ) -> usize;

        // returns 1 if the descriptor is for an error, 0 if it is for a result.
        // Either way, the data is obtained by repeated calls to wrought_descriptor_read.
        fn wrought_descriptor_is_err(rd: usize) -> usize;

        // reads part of a descriptor into a provided buffer, returns the amount of data
        // written. If it returns 0 then every thing has been read.
        fn wrought_descriptor_read(rd: usize, buf_ptr: *mut u8, buf_len: usize) -> usize;

        // close the descriptor as we don't need it any more.
        fn wrought_descriptor_close(rd: usize);
    }

    use std::path::Path;

    use serde::Deserialize;

    #[derive(Deserialize)]
    enum WroughtErrorCode {
        Unknown,
    }

    #[derive(Deserialize)]
    pub struct WroughtError {
        message: String,
        code: WroughtErrorCode,
    }

    type Result<T> = std::result::Result<T, WroughtError>;

    // This is what we're going to make available to the scripts
    pub struct WroughtApi {}

    impl WroughtApi {
        unsafe fn read_descriptor(rd: usize) -> Vec<u8> {
            let mut result = vec![];
            // TODO: Make this bigger?
            let mut buf = [0u8; 256];
            loop {
                let len = wrought_descriptor_read(rd, buf.as_mut_ptr(), buf.len());
                if len == 0 {
                    break;
                }
                result.copy_from_slice(&buf[0..len]);
            }
            result
        }

        pub fn read_file(&self, path: &Path) -> Result<Option<Vec<u8>>> {
            let (is_err, data) = unsafe {
                let p = format!("{}", path.display());
                let rd = wrought_read_file(p.as_ptr(), p.len());
                let is_err = wrought_descriptor_is_err(rd) == 1;
                let data = Self::read_descriptor(rd);
                wrought_descriptor_close(rd);
                (is_err, data)
            };
            if is_err {
                let e: WroughtError = serde_json::from_slice(&data).unwrap();
                Err(e)
            } else {
                let v: Option<Vec<u8>> = serde_json::from_slice(&data).unwrap();
                Ok(v)
            }
        }
        pub fn write_file(&self, path: &Path, content: &[u8]) -> Result<()> {
            let (is_err, data) = unsafe {
                let p = format!("{}", path.display());
                let rd = wrought_write_file(p.as_ptr(), p.len(), content.as_ptr(), content.len());
                let is_err = wrought_descriptor_is_err(rd) == 1;
                let data = Self::read_descriptor(rd);
                wrought_descriptor_close(rd);
                (is_err, data)
            };
            if is_err {
                let e: WroughtError = serde_json::from_slice(&data).unwrap();
                Err(e)
            } else {
                Ok(())
            }
        }
        pub fn get_metadata(&self, path: &Path, key: &str) -> Result<Option<Vec<u8>>> {
            todo!();
        }
        pub fn set_metadata(&self, path: &Path, key: &str, value: &[u8]) -> Result<()> {
            todo!();
        }
    }
}
