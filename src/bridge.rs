use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use crate::{
    backend::Backend,
    events::{Event, EventGroup, WriteFileEvent},
};

pub trait Bridge {
    fn write_file(&mut self, path: &Path, value: &[u8]) -> anyhow::Result<()>;
    fn get_event_group(&self) -> Option<EventGroup>;
}

pub struct DummyBridge {
    pub backend: Arc<Mutex<dyn Backend>>,
    // pub event_log: Arc<Mutex< dyn EventLog >>,
    pub root: PathBuf,

    pub event_group: EventGroup,
}

impl Bridge for DummyBridge {
    fn write_file(&mut self, path: &Path, value: &[u8]) -> anyhow::Result<()> {
        let (before_hash, hash) = self.backend.lock().unwrap().write_file(path, value)?;
        let after_hash = Some(hash);
        let event = WriteFileEvent {
            path: path.to_path_buf(),
            before_hash,
            after_hash,
        };
        self.add_event(event.into());
        Ok(())
    }
    fn get_event_group(&self) -> Option<EventGroup> {
        if self.event_group.events.is_empty() {
            return None;
        }
        Some(self.event_group.clone())
    }
}

impl DummyBridge {
    pub fn add_event(&mut self, event: Event) {
        self.event_group.events.push(event);
    }
}
