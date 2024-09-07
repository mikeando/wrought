use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use clap::{Parser, Subcommand};

pub mod binary16;

use binary16::ContentHash;

pub struct Wrought {
    backend: Arc<Mutex<dyn Backend>>,
}

impl Wrought {
    pub fn begin_script<N, F>(&mut self, name: N, f: F)
    where
        N: Into<String>,
        F: FnOnce(&mut MicroService),
    {
        let mut m = MicroService::new(self.backend.clone());
        println!("Wrought::begin_script - runnning {}", name.into());
        f(&mut m);
        eprintln!(
            "Wrough::begin_script - logged microactions =\n{:#?}",
            m.microactions
        );
    }

    pub fn new(backend: Arc<Mutex<dyn Backend>>) -> Wrought {
        Wrought { backend }
    }
}

// TODO: These are the same as Events below.
#[derive(Debug, Clone)]
pub enum MicroAction {
    GetMetadata(PathBuf, MetadataKey, Option<MetadataEntry>),
    SetMetadata(PathBuf, MetadataKey, Option<MetadataEntry>),
    WriteFile(PathBuf, ContentHash),
}

impl MicroAction {
    pub fn get_metadata(
        path: PathBuf,
        key: MetadataKey,
        value: Option<MetadataEntry>,
    ) -> MicroAction {
        MicroAction::GetMetadata(path, key, value)
    }
    pub fn set_metadata(
        path: PathBuf,
        key: MetadataKey,
        value: Option<MetadataEntry>,
    ) -> MicroAction {
        MicroAction::SetMetadata(path, key, value)
    }
}

pub struct MicroService {
    pub microactions: Vec<MicroAction>,
    pub backend: Arc<Mutex<dyn Backend>>,
}

impl MicroService {
    pub fn new(backend: Arc<Mutex<dyn Backend>>) -> MicroService {
        MicroService {
            microactions: vec![],
            backend,
        }
    }

    pub fn get_metadata<P: Into<PathBuf>, K: Into<MetadataKey>>(
        &mut self,
        path: P,
        key: K,
    ) -> Option<MetadataEntry> {
        let path = path.into();
        let key = key.into();
        let value = self.backend.lock().unwrap().get_metadata(&path, &key);
        self.microactions
            .push(MicroAction::get_metadata(path, key, value.clone()));
        value
    }

    pub fn set_metadata<P: Into<PathBuf>, K: Into<MetadataKey>, V: Into<MetadataEntry>>(
        &mut self,
        path: P,
        key: K,
        value: V,
    ) {
        let path = path.into();
        let key = key.into();
        let value = Some(value.into());
        self.backend
            .lock()
            .unwrap()
            .set_metadata(&path, &key, &value);
        self.microactions
            .push(MicroAction::set_metadata(path, key, value.clone()));
    }

    pub fn write_file<P: Into<PathBuf>>(&mut self, path: P, value: &[u8]) {
        let path = path.into();
        let hash = self.backend.lock().unwrap().write_file(&path, value);
        self.microactions.push(MicroAction::WriteFile(path, hash));
    }
}

#[derive(Debug, Clone)]
pub enum MetadataKey {
    StringKey(String),
}

impl From<&str> for MetadataKey {
    fn from(value: &str) -> Self {
        MetadataKey::StringKey(value.to_string())
    }
}

pub trait Backend {
    fn get_metadata(&self, path: &Path, key: &MetadataKey) -> Option<MetadataEntry>;
    fn set_metadata(&self, path: &Path, key: &MetadataKey, value: &Option<MetadataEntry>);
    fn write_file(&self, path: &Path, value: &[u8]) -> ContentHash;
}

#[derive(Debug, Clone)]
pub struct MetadataEntry {}

impl MetadataEntry {
    pub fn as_string(&self) -> String {
        todo!();
    }
}

impl From<&str> for MetadataEntry {
    fn from(value: &str) -> Self {
        MetadataEntry {}
    }
}

pub fn hello_world(wrought: &mut Wrought) {
    wrought.begin_script("hello world", |m: &mut MicroService| {
        if let Some(md) = m.get_metadata("index.md", "name") {
            m.write_file(
                "hello.txt",
                format!("greetings, {}", md.as_string()).as_bytes(),
            );
        } else {
            m.set_metadata("index.md", "name", "Unknown");
            m.write_file("hello.txt", "greetings!".as_bytes());
        }
    });
}

struct DummyBackend {}

impl Backend for DummyBackend {
    fn get_metadata(&self, path: &Path, key: &MetadataKey) -> Option<MetadataEntry> {
        eprintln!("DummyBackend::get_metadata({:?}, {:?})", path, key);
        None
    }
    fn set_metadata(&self, path: &Path, key: &MetadataKey, value: &Option<MetadataEntry>) {
        eprintln!(
            "DummyBackend::set_metadata({:?}, {:?}, {:?})",
            path, key, value
        );
    }
    fn write_file(&self, path: &Path, value: &[u8]) -> ContentHash {
        eprintln!(
            "DummyBackend::write_file({:?}, {:?})",
            path,
            String::from_utf8_lossy(value).to_string()
        );
        ContentHash::from_content(value)
    }
}

/// Search for a pattern in a file and display the lines that contain it.
#[derive(Parser)]
struct Cli {
    /// Command to run
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    FileStatus(FileStatusCmd),
    Init,
    HelloWorld,
}

#[derive(Debug, Parser)] // requires `derive` feature
struct FileStatusCmd {
    path: PathBuf,
}

#[derive(Debug)]
pub struct Event {
    id: u64,
    group_id: u64,
    event_type: EventType,
}

#[derive(Debug)]
pub enum EventType {
    WriteFile(WriteFileEvent),
    ReadFile(ReadFileEvent),
}

// Can actually represent create/modify/delete
#[derive(Debug)]
pub struct WriteFileEvent {
    path: PathBuf,
    before_hash: Option<ContentHash>,
    after_hash: Option<ContentHash>,
}

// When called on a missing file, hash=None
#[derive(Debug)]
pub struct ReadFileEvent {
    path: PathBuf,
    hash: Option<ContentHash>,
}

#[derive(Debug)]
pub struct EventGroup {
    command: String,
    events: Vec<Event>,
    is_most_recent_run: bool,
}

struct DummyStatusBackend {}

fn create_backend() -> anyhow::Result<DummyStatusBackend> {
    Ok(DummyStatusBackend {})
}

impl StatusBackend for DummyStatusBackend {
    // // TODO: Get rid of this - we can do it with the two low level functions listed below.
    // fn get_last_write_info(&self, p: &Path) -> anyhow::Result<Option<(ContentHash, ChangeSet)>> {

    //     // TODO: Provide a way to oconvert an event row into an Event object.
    //     eprintln!("event = {:?}", event);

    //     todo!();
    // }

    fn get_last_write_event(&self, p: &Path) -> anyhow::Result<Option<Event>> {
        use rusqlite::OpenFlags;
        //TODO: Move the conn into the instance.
        let conn = rusqlite::Connection::open_with_flags(
            "wrought.db",
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_URI
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        let mut stmt = conn.prepare("SELECT * FROM Events WHERE action_type='write' AND file_path=?1 ORDER BY id DESC LIMIT 1")?;
        let mut events = stmt.query([format!("{}", p.display())])?;
        let Some(event_row) = events.next()? else {
            return Ok(None);
        };
        let event = self.event_from_event_row(event_row)?;
        Ok(Some(event))
    }

    fn get_event_group(&self, group_id: u64) -> anyhow::Result<Option<EventGroup>> {
        use rusqlite::OpenFlags;
        //TODO: Move the conn into the instance.
        let conn = rusqlite::Connection::open_with_flags(
            "wrought.db",
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_URI
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;

        // Read the group data
        let mut stmt = conn.prepare("SELECT * FROM Groups where id=?1")?;
        let mut groups = stmt.query([group_id])?;
        let Some(group_row) = groups.next()? else {
            return Ok(None);
        };
        let mut group = self.group_from_group_row(group_row)?;

        // Now actually read the events it contains
        let mut stmt = conn.prepare("SELECT * FROM Events WHERE group_id=?1")?;
        let mut events = stmt.query([group_id])?;
        while let Some(event_row) = events.next()? {
            let event = self.event_from_event_row(event_row)?;
            group.events.push(event);
        }

        Ok(Some(group))
    }
}

impl DummyStatusBackend {
    fn init() -> anyhow::Result<()> {
        // For now we swallow errors if we cant remove the file.
        _ = fs::remove_file("wrought.bd");
        let conn = rusqlite::Connection::open("wrought.db")?;

        conn.execute(
            "create table Events (
                 id integer primary key,
                 group_id integer NOT NULL REFERENCES Groups(id),
                 action_type text NOT NULL,
                 file_path text
             )",
            (),
        )?;
        conn.execute(
            "create table Groups (
                 id integer primary key
             )",
            (),
        )?;
        Ok(())
    }

    fn event_from_event_row(&self, row: &rusqlite::Row) -> anyhow::Result<Event> {
        // Unpack the row
        let id: u64 = row.get("id")?;
        let group_id: u64 = row.get("group_id")?;
        let action_type: String = row.get("action_type")?;
        let event_type = match action_type.as_str() {
            "write" => {
                let file_path: String = row.get("file_path")?;
                let file_path = PathBuf::from(file_path);

                let before_hash: Option<String> = row.get("before_hash")?;
                let before_hash = match before_hash {
                    Some(s) => Some(ContentHash::from_string(&s)?),
                    None => None,
                };

                let after_hash: Option<String> = row.get("after_hash")?;
                let after_hash = match after_hash {
                    Some(s) => Some(ContentHash::from_string(&s)?),
                    None => None,
                };

                let write_file_event = WriteFileEvent {
                    path: file_path,
                    before_hash,
                    after_hash,
                };
                EventType::WriteFile(write_file_event)
            }
            "read" => {
                let file_path: String = row.get("file_path")?;
                let file_path = PathBuf::from(file_path);

                let before_hash: Option<String> = row.get("before_hash")?;
                let before_hash = match before_hash {
                    Some(s) => Some(ContentHash::from_string(&s)?),
                    None => None,
                };

                let read_file_event = ReadFileEvent {
                    path: file_path,
                    hash: before_hash,
                };
                EventType::ReadFile(read_file_event)
            }
            _ => {
                unreachable!("Invalid action_type='{}' encountered", action_type);
            }
        };

        Ok(Event {
            id,
            group_id,
            event_type,
        })
    }

    fn group_from_group_row(&self, row: &rusqlite::Row) -> anyhow::Result<EventGroup> {
        let command = row.get("command")?;
        // TODO: Fill in is_most_recent_run somehow?
        Ok(EventGroup {
            command,
            events: vec![],
            is_most_recent_run: true,
        })
    }
}

fn main() {
    let args = Cli::parse();
    match args.command {
        Command::FileStatus(cmd) => {
            let backend = create_backend().unwrap();
            let status = get_single_file_status(&backend, &cmd.path).unwrap();
            print_single_file_status(&status);
        }
        Command::HelloWorld => {
            let mut w = Wrought::new(Arc::new(Mutex::new(DummyBackend {})));
            hello_world(&mut w);
        }
        Command::Init => {
            DummyStatusBackend::init().unwrap();
        }
    }
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

pub struct ChangeSet {
    command: String,
    is_most_recent_run: bool,
}

impl ChangeSet {
    pub fn inputs(&self) -> Vec<(&Path, &ContentHash)> {
        todo!();
    }
}

pub trait StatusBackend {
    fn get_last_write_event(&self, p: &Path) -> anyhow::Result<Option<Event>>;
    fn get_event_group(&self, group_id: u64) -> anyhow::Result<Option<EventGroup>>;
}

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

pub fn get_single_file_status<B: StatusBackend>(
    backend: &B,
    p: &Path,
) -> anyhow::Result<SingleFileStatusResult> {
    // To get the file status we need to know the last write to it - which should return a hash
    // and the change-set-id that it was last changed in.
    // We can then compare the hash of the file with that in the change-set to determine if it has changed,
    // and compare the hash of all the inputs to determine if it is stale.

    let Some(event) = backend.get_last_write_event(p)? else {
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

    let Some(event_group) = backend.get_event_group(event.group_id)? else {
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
