use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use backend::{Backend, DummyBackend};
use clap::{Parser, Subcommand};

pub mod backend;
pub mod binary16;
pub mod event_log;
pub mod events;
pub mod metadata;

use binary16::ContentHash;
use event_log::{DummyEventLog, EventLog};
use events::Event;
use events::{EventType, GetMetadataEvent, SetMetadataEvent, WriteFileEvent};

use metadata::MetadataEntry;
use metadata::MetadataKey;

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
        let value = self.backend.lock().unwrap().get_metadata(path.as_ref(), &key)?;

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
        let before_value = self
            .backend
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
        let (before_hash, after_hash) = self.backend.lock().unwrap().write_file(path.as_ref(), value)?;
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

fn main() {
    let args = Cli::parse();
    match args.command {
        Command::FileStatus(cmd) => {
            let backend = DummyEventLog::open("wrought.db").unwrap();
            let status = get_single_file_status(&backend, &cmd.path).unwrap();
            print_single_file_status(&status);
        }
        Command::HelloWorld => {
            let mut w = Wrought::new(Arc::new(Mutex::new(DummyBackend {})));
            hello_world(&mut w);
        }
        Command::Init => {
            DummyEventLog::init("wrought.db").unwrap();
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
