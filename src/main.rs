use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use anyhow::{anyhow, bail, Context};
use backend::{Backend, DummyBackend};
use bridge::{Bridge, DummyBridge};
use clap::{Parser, Subcommand};

pub mod backend;
pub mod binary16;
pub mod bridge;
pub mod content_store;
pub mod event_log;
pub mod events;
pub mod file_history;
pub mod fs_utils;
pub mod llm;
pub mod metadata;
pub mod scripting_luau;

use binary16::ContentHash;
use content_store::{ContentStore, DummyContentStore};
use event_log::{DummyEventLog, EventLog};
use events::{Event, EventGroup};
use events::{EventType, GetMetadataEvent, SetMetadataEvent, WriteFileEvent};

use file_history::FileHistoryEntry;
use llm::{InvalidLLM, OpenAILLM, LLM};
use metadata::MetadataEntry;
use metadata::MetadataKey;
use scripting_luau::run_script;
use xfs::Xfs;

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
    History(HistoryCmd),
    ContentStoreShow(ContentStoreShowCmd),
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

#[derive(Debug, Parser)]
struct HistoryCmd {
    path: PathBuf,
}

//TODO: Make this a sub-command on a ContentStore function
#[derive(Debug, Parser)]
struct ContentStoreShowCmd {
    hash: String,
}

fn find_first_existing_parent(
    fs: &dyn xfs::Xfs,
    starting_dir: &Path,
) -> anyhow::Result<Option<PathBuf>> {
    let mut current_dir = starting_dir;

    loop {
        if fs.exists(current_dir) {
            return Ok(Some(current_dir.to_path_buf()));
        }

        let parent_dir = current_dir.parent();
        match parent_dir {
            Some(parent) => current_dir = parent,
            None => return Ok(None),
        }
    }
}

fn find_marker_dir(
    fs: &dyn xfs::Xfs,
    starting_dir: &Path,
    marker: &str,
) -> anyhow::Result<Option<PathBuf>> {
    let starting_dir = fs.canonicalize(starting_dir)?;
    let mut current_dir: &Path = &starting_dir;

    loop {
        let marker_path = current_dir.join(marker);
        if fs.is_dir(&marker_path) {
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
    let fs = Arc::new(Mutex::new(xfs::OsFs {}));
    let path = &cmd.path;

    // Check the target is not already in a project.
    let check = || -> anyhow::Result<Option<PathBuf>> {
        let existing_parent = find_first_existing_parent(&*fs.lock().unwrap(), path)
            .context("in find_first_existing_parent")?;
        let Some(existing_parent) = existing_parent else {
            return Ok(None);
        };
        find_marker_dir(&*fs.lock().unwrap(), &existing_parent, ".wrought")
            .context("in find_marker_dir")
    };

    if let Some(parent_path) = check().unwrap() {
        panic!(
            "Path '{}' is part of project with root '{}'",
            path.display(),
            parent_path.display()
        );
    }

    fs.lock().unwrap().create_dir_all(path).unwrap();
    fs.lock()
        .unwrap()
        .create_dir_all(&path.join(".wrought"))
        .unwrap();

    let mut writer = fs
        .lock()
        .unwrap()
        .writer(&path.join(".wrought").join("settings.toml"))?;
    writer.write_all(
        [
            "# General Project Settings",
            "",
            "# LLM Settings",
            "# Uncomment and set to enable LLM features",
            "# openai_api_key = \"PUT_YOUR_KEY_HERE\"",
            "",
        ]
        .join("\n")
        .as_bytes(),
    )?;

    let content_dir = path.join("_content");
    fs.lock().unwrap().create_dir_all(&content_dir).unwrap();

    // TODO: Make this configurable.
    let src_package_dir = PathBuf::from("./resources/packages/");
    let project_package_dir = path.join(".wrought").join("packages");

    fs.lock()
        .unwrap()
        .create_dir_all(&path.join(".wrought"))
        .unwrap();
    DummyEventLog::init(path.join(".wrought").join("wrought.db")).unwrap();
    fs.lock()
        .unwrap()
        .create_dir_all(&project_package_dir)
        .unwrap();

    let project_package = project_package_dir.join(&cmd.package);
    fs.lock().unwrap().create_dir_all(&project_package).unwrap();

    fs_utils::copy_dir_all_with_filters(
        &mut *fs.lock().unwrap(),
        src_package_dir.join(&cmd.package),
        &project_package,
        |_, _| true,
        |_, _| true,
    )?;

    // Now if there is an init script we should run it.
    println!("Running init scripts");

    let bridge = create_bridge(path)?;

    if project_package.join("init.luau").is_file() {
        run_script(bridge.clone(), fs, &project_package.join("init.luau"))?;
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
    path: PathBuf,
    content: String,
}

impl PackageStatus {
    pub fn read_from(fs: &dyn xfs::Xfs, p: &Path) -> anyhow::Result<PackageStatus> {
        let mut content = String::new();
        fs.reader(p)?.read_to_string(&mut content)?;
        Ok(PackageStatus {
            path: p.to_path_buf(),
            content,
        })
    }
}

struct Package {
    path: PathBuf,
}

impl Package {
    fn statuses(&self, fs: &dyn xfs::Xfs) -> Vec<anyhow::Result<PackageStatus>> {
        let status_dir = self.path.join("status");
        let mut result = vec![];

        let mut f = |fs: &dyn Xfs, entry: &dyn xfs::XfsDirEntry| -> anyhow::Result<()> {
            let md = match entry.metadata() {
                Ok(md) => md,
                Err(e) => {
                    result.push(
                        Err(e).with_context(|| format!("getting metadata for {:?}", entry.path())),
                    );
                    return Ok(());
                }
            };
            if !md.is_file() {
                result.push(Err(anyhow!(
                    "status entry {:?} is not a file",
                    entry.path()
                )));
                return Ok(());
            }

            result.push(PackageStatus::read_from(fs, &entry.path()));
            Ok(())
        };

        if let Err(e) = fs.on_each_entry(&status_dir, &mut f) {
            result.push(
                Err(e).with_context(|| format!("while reading statuses from {:?}", status_dir)),
            );
        }
        result
    }

    fn name(&self) -> String {
        self.path.file_name().unwrap().to_str().unwrap().to_string()
    }
}

struct PackageDirectory {
    path: PathBuf,
}

impl PackageDirectory {
    fn packages(&self, fs: &dyn xfs::Xfs) -> Vec<anyhow::Result<Package>> {
        let mut result = vec![];

        let mut f = |_fs: &dyn Xfs, entry: &dyn xfs::XfsDirEntry| -> anyhow::Result<()> {
            let md = match entry.metadata() {
                Err(e) => {
                    result.push(
                        Err(e).with_context(|| format!("getting metadata for {:?}", entry.path())),
                    );
                    return Ok(());
                }
                Ok(md) => md,
            };
            if !md.is_dir() {
                result.push(Err(anyhow!(
                    "package directory entry {:?} is not a directory",
                    entry.path()
                )));
                return Ok(());
            }
            result.push(Ok(Package { path: entry.path() }));
            Ok(())
        };

        if let Err(e) = fs.on_each_entry(&self.path, &mut f) {
            result.push(
                Err(e).with_context(|| format!("while reading packages from {:?}", self.path)),
            );
        }
        result
    }
}

fn cmd_status(project_root: &Path, _cmd: StatusCmd) -> anyhow::Result<()> {
    let fs = Arc::new(Mutex::new(xfs::OsFs {}));

    let package_dir = PackageDirectory {
        path: project_root.join(".wrought").join("packages"),
    };
    let packages = package_dir.packages(&*fs.lock().unwrap());
    for package in packages {
        match package {
            Ok(package) => {
                println!("{}", package.name());
                println!("---",);
                for status in package.statuses(&*fs.lock().unwrap()) {
                    match status {
                        Ok(status) => {
                            println!("* {}", status.path.file_name().unwrap().to_string_lossy());

                            let mut content: Vec<_> =
                                status.content.lines().map(|l| l.trim()).collect();
                            while let Some(c) = content.last() {
                                if !c.is_empty() {
                                    break;
                                }
                                content.pop();
                            }
                            for line in content {
                                println!("   | {}", line);
                            }
                        }
                        Err(e) => eprintln!("  * error : {:?}", e),
                    }
                }
            }
            Err(e) => {
                println!("- package error: {:?}\n", e)
            }
        }
    }
    Ok(())
}

fn cmd_run_script(
    bridge: Arc<Mutex<dyn Bridge>>,
    project_root: &Path,
    cmd: RunScriptCmd,
) -> anyhow::Result<()> {
    let fs = Arc::new(Mutex::new(xfs::OsFs {}));
    let script_path = project_root
        .join(".wrought")
        .join("packages")
        .join(cmd.script_name);
    run_script(bridge, fs, &script_path)?;
    Ok(())
}

pub fn create_backend(path: &Path) -> anyhow::Result<Arc<Mutex<dyn Backend>>> {
    let fs = Arc::new(Mutex::new(xfs::OsFs {}));
    let path = fs.lock().unwrap().canonicalize(path)?;
    let content_storage_path = path.join("_content");
    let content_store = Arc::new(Mutex::new(DummyContentStore::new(
        fs.clone(),
        content_storage_path,
    )));
    Ok(Arc::new(Mutex::new(DummyBackend {
        fs,
        root: path,
        content_store,
    })))
}

pub fn create_event_log(path: &Path) -> anyhow::Result<Arc<Mutex<dyn EventLog>>> {
    Ok(Arc::new(Mutex::new(
        DummyEventLog::open(path.join(".wrought").join("wrought.db")).unwrap(),
    )))
}

pub fn create_bridge(path: &Path) -> anyhow::Result<Arc<Mutex<dyn Bridge>>> {
    let fs = Arc::new(Mutex::new(xfs::OsFs {}));
    // Load up an settings in the project settings file - needed
    // to initialise the openAI LLM.
    let root = fs.lock().unwrap().canonicalize(path)?;
    let reader = fs
        .lock()
        .unwrap()
        .reader_if_exists(&root.join(".wrought").join("settings.toml"))?;
    let settings = match reader {
        Some(mut reader) => {
            let mut settings = String::new();
            reader.read_to_string(&mut settings)?;
            settings.parse::<toml::Table>()?
        }
        None => toml::Table::new(),
    };
    let backend = create_backend(path)?;
    let llm_cache_dir = root.join(".wrought").join("llm_cache");
    fs.lock().unwrap().create_dir_all(&llm_cache_dir)?;
    // TODO: Get this from somewhere...

    let openai_api_key = match settings.get("openai_api_key") {
        Some(openai_api_key) => Some(
            openai_api_key
                .as_str()
                .context("invalid setting: openai_api_key is not a string")?
                .to_string(),
        ),
        None => None,
    };
    let llm: Arc<Mutex<dyn LLM>> = match openai_api_key {
        Some(openai_api_key) => {
            let llm = OpenAILLM::create_with_key(openai_api_key, fs, &llm_cache_dir)?;
            Arc::new(Mutex::new(llm))
        }
        None => {
            let llm =
                InvalidLLM::create_with_error_message("no openAI key specified in settings file");
            Arc::new(Mutex::new(llm))
        }
    };

    Ok(Arc::new(Mutex::new(DummyBridge {
        root,
        backend,
        event_group: EventGroup::empty(),
        llm,
    })))
}

fn get_absolute_project_and_relative_file(
    fs: &dyn xfs::Xfs,
    working_dir: &Path,
    file_path: &Path,
    project_root: Option<&Path>,
) -> anyhow::Result<(PathBuf, PathBuf)> {
    eprintln!(
        "get_absolute_project_and_relative_file: working_dir={:?} file_path={:?} project_root={:?}",
        working_dir, file_path, project_root
    );

    assert!(working_dir.is_absolute());

    let file_path = if file_path.is_absolute() {
        file_path.to_path_buf()
    } else {
        working_dir.join(file_path)
    };

    //NOTE: We can't immediately cannonicalise file_path as it may not exits.

    // Now if we've explicitly specified a project_root, use that
    // and check the file is inside the project root, otherwise search for the project root.

    let project_root = match project_root {
        Some(p) => {
            let p = if p.is_absolute() {
                p.to_path_buf()
            } else {
                working_dir.join(p)
            };
            if !fs.is_dir(&p.join(".wrought")) {
                bail!("specified project root {} has no .wrought subdirectory - it is not a valid root", p.display());
            }
            fs.canonicalize(&p)?
        }
        None => {
            let parent = find_first_existing_parent(fs, &file_path)?;
            let parent = parent.with_context(|| {
                format!(
                    "Unable to find existing parent directory for {:?}",
                    file_path
                )
            })?;
            let parent = fs.canonicalize(&parent)?;
            let project_root = find_marker_dir(fs, &parent, ".wrought")?;
            project_root.with_context(|| {
                format!("Unable to find wrought root containing {:?}", file_path)
            })?
        }
    };
    eprintln!("using project_root = {:?}", project_root);

    let relative_file_path = file_path
        .strip_prefix(&project_root)
        .with_context(|| {
            format!(
                "file '{:?}' not inside project root '{:?}'",
                file_path, project_root
            )
        })?
        .to_path_buf();
    Ok((project_root, relative_file_path))
}

fn cmd_history(
    _cmd: HistoryCmd,
    fs: Arc<Mutex<dyn xfs::Xfs>>,
    event_log: Arc<Mutex<dyn EventLog>>,
    project_root: &Path,
    file_path: &Path,
) -> anyhow::Result<()> {
    let entries = file_history::file_history(fs, event_log, project_root, file_path)?;
    for e in entries {
        match e {
            FileHistoryEntry::Deleted => eprintln!("- nothing"),
            FileHistoryEntry::DeletedBy(cmd) => eprintln!("+ nothing : {}", cmd.0),
            FileHistoryEntry::UnknownHash(hash) => eprintln!("- {} : ???", hash),
            FileHistoryEntry::StoredHash(hash, cmd) => {
                eprintln!("+ {} : {}", hash, cmd.0)
            }
            FileHistoryEntry::LocalChanges(hash) => {
                eprintln!("- {} : local changes", hash)
            }
        }
    }
    Ok(())
}

fn cmd_content_store_show(
    cmd: ContentStoreShowCmd,
    content_store: Arc<Mutex<dyn ContentStore>>,
) -> anyhow::Result<()> {
    let hash = ContentHash::from_string(&cmd.hash)?;
    let content = content_store.lock().unwrap().retrieve(hash)?;
    let Some(content) = content else {
        return Err(anyhow!("Hash does not correspond to known content"));
    };
    print!("{}", String::from_utf8_lossy(&content));
    Ok(())
}

fn main() {
    let fs: Arc<Mutex<dyn xfs::Xfs>> = Arc::new(Mutex::new(xfs::OsFs {}));

    let working_dir = fs
        .lock()
        .unwrap()
        .canonicalize(&PathBuf::from("."))
        .unwrap();
    let args = Cli::parse();

    // Have to handle Init differntly as it doesn't care about the project_root already
    // existing etc.
    if let Command::Init(cmd) = &args.command {
        cmd_init(cmd).unwrap();
        return;
    }

    match args.command {
        Command::FileStatus(cmd) => {
            // resolve the path relative to the project root.
            let (project_root, file_path) = get_absolute_project_and_relative_file(
                &*fs.lock().unwrap(),
                &working_dir,
                &cmd.path,
                args.project_root.as_deref(),
            )
            .unwrap();
            let event_log = create_event_log(&project_root).unwrap();
            let status =
                get_single_file_status(&fs, &project_root, &event_log, &file_path).unwrap();
            print_single_file_status(&status);
        }
        Command::HelloWorld => {
            // Check the project_root exists
            let project_root = match &args.project_root {
                Some(p) => {
                    if !fs.lock().unwrap().is_dir(&p.join(".wrought")) {
                        panic!("specified project root {} has no .wrought subdirectory - it is not a valid root", p.display());
                    }
                    p.clone()
                }
                None => {
                    match find_marker_dir(&*fs.lock().unwrap(), &PathBuf::from("."), ".wrought") {
                        Ok(Some(p)) => p,
                        Ok(None) => panic!("Unable to find project root for current directory"),
                        Err(e) => panic!("Error looking for project root: {}", e),
                    }
                }
            };
            // eprintln!("Using project root: '{}'", project_root.display());

            let backend = create_backend(&project_root).unwrap();
            let mut w = Wrought::new(backend);
            hello_world(&mut w);
        }
        Command::Status(cmd) => {
            // Check the project_root exists
            let project_root = match &args.project_root {
                Some(p) => {
                    if !fs.lock().unwrap().is_dir(&p.join(".wrought")) {
                        panic!("specified project root {} has no .wrought subdirectory - it is not a valid root", p.display());
                    }
                    p.clone()
                }
                None => {
                    match find_marker_dir(&*fs.lock().unwrap(), &PathBuf::from("."), ".wrought") {
                        Ok(Some(p)) => p,
                        Ok(None) => panic!("Unable to find project root for current directory"),
                        Err(e) => panic!("Error looking for project root: {}", e),
                    }
                }
            };
            // eprintln!("Using project root: '{}'", project_root.display());

            cmd_status(&project_root, cmd).unwrap();
        }
        Command::History(cmd) => {
            // resolve the path relative to the project root.
            let (project_root, file_path) = get_absolute_project_and_relative_file(
                &*fs.lock().unwrap(),
                &working_dir,
                &cmd.path,
                args.project_root.as_deref(),
            )
            .unwrap();
            let event_log = create_event_log(&project_root).unwrap();
            cmd_history(cmd, fs, event_log, &project_root, &file_path).unwrap();
        }
        Command::ContentStoreShow(cmd) => {
            // resolve the path relative to the project root.
            // Has the user specified a path?
            let project_root = match args.project_root {
                Some(project_root) => fs
                    .lock()
                    .unwrap()
                    .canonicalize(&working_dir.join(project_root))
                    .unwrap(),
                None => find_marker_dir(&*fs.lock().unwrap(), &working_dir, ".wrought")
                    .unwrap()
                    .unwrap(),
            };

            let content_storage_path = project_root.join("_content");
            let content_store = Arc::new(Mutex::new(DummyContentStore::new(
                fs.clone(),
                content_storage_path,
            )));

            cmd_content_store_show(cmd, content_store).unwrap();
        }
        Command::RunScript(cmd) => {
            // Check the project_root exists
            let project_root = match &args.project_root {
                Some(p) => {
                    if !fs.lock().unwrap().is_dir(&p.join(".wrought")) {
                        panic!("specified project root {} has no .wrought subdirectory - it is not a valid root", p.display());
                    }
                    p.clone()
                }
                None => {
                    match find_marker_dir(&*fs.lock().unwrap(), &PathBuf::from("."), ".wrought") {
                        Ok(Some(p)) => p,
                        Ok(None) => panic!("Unable to find project root for current directory"),
                        Err(e) => panic!("Error looking for project root: {}", e),
                    }
                }
            };
            // eprintln!("Using project root: '{}'", project_root.display());

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

pub fn calculate_file_hash(fs: &dyn xfs::Xfs, p: &Path) -> anyhow::Result<Option<ContentHash>> {
    match fs.reader_if_exists(p) {
        Ok(Some(mut reader)) => Ok(Some(ContentHash::from_reader(&mut reader)?)),
        Ok(None) => Ok(None),
        Err(e) => Err(anyhow!(e)),
    }
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

pub fn get_single_file_status(
    fs: &Arc<Mutex<dyn xfs::Xfs>>,
    project_root: &Path,
    event_log: &Arc<Mutex<dyn EventLog>>,
    p: &Path,
) -> anyhow::Result<SingleFileStatusResult> {
    let event_log = event_log.lock().unwrap();

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

    let current_hash = calculate_file_hash(&*fs.lock().unwrap(), &project_root.join(p))?;
    eprintln!("Getting file hash for {:?} = {:?}", p, current_hash);

    let Some(event_group) = event_log.get_event_group(event.group_id)? else {
        unreachable!("get_last_write_event returned an event with invalid group_id");
    };

    let mut inputs = vec![];
    for e in &event_group.events {
        match &e.event_type {
            EventType::ReadFile(read_file_event) => {
                let path = read_file_event.path.clone();
                let current_hash =
                    calculate_file_hash(&*fs.lock().unwrap(), &project_root.join(&path))?;
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
