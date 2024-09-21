use std::path::PathBuf;

use crate::binary16::ContentHash;
use crate::metadata::MetadataEntry;
use crate::metadata::MetadataKey;

#[derive(Debug, Clone)]
pub struct Event {
    pub id: u64,
    pub group_id: u64,
    pub event_type: EventType,
}
impl Event {
    pub(crate) fn with_group_id(&self, group_id: u64) -> Event {
        let mut result = self.clone();
        result.group_id = group_id;
        result
    }
}

#[derive(Debug, Clone)]
pub enum EventType {
    WriteFile(WriteFileEvent),
    ReadFile(ReadFileEvent),
    GetMetadata(GetMetadataEvent),
    SetMetadata(SetMetadataEvent),
}

// Can actually represent create/modify/delete
#[derive(Debug, Clone)]
pub struct WriteFileEvent {
    pub path: PathBuf,
    pub before_hash: Option<ContentHash>,
    pub after_hash: Option<ContentHash>,
}

// When called on a missing file, hash=None
#[derive(Debug, Clone)]
pub struct ReadFileEvent {
    pub path: PathBuf,
    pub hash: Option<ContentHash>,
}

#[derive(Debug, Clone)]
pub struct GetMetadataEvent {
    pub path: PathBuf,
    pub key: MetadataKey,
    pub value: Option<MetadataEntry>,
}

#[derive(Debug, Clone)]
pub struct SetMetadataEvent {
    pub path: PathBuf,
    pub key: MetadataKey,
    pub before_value: Option<MetadataEntry>,
    pub after_value: Option<MetadataEntry>,
}

impl From<WriteFileEvent> for EventType {
    fn from(value: WriteFileEvent) -> Self {
        EventType::WriteFile(value)
    }
}

impl From<WriteFileEvent> for Event {
    fn from(value: WriteFileEvent) -> Self {
        let event_type = value.into();
        Event {
            id: 0,
            group_id: 0,
            event_type,
        }
    }
}

#[derive(Debug, Clone)]
pub struct EventGroup {
    pub command: String,
    pub events: Vec<Event>,
    pub is_most_recent_run: bool,
}
impl EventGroup {
    pub(crate) fn empty() -> EventGroup {
        EventGroup {
            command: "unknown".to_string(),
            events: vec![],
            is_most_recent_run: true,
        }
    }
}
