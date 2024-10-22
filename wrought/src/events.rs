use std::path::PathBuf;

use crate::binary16::ContentHash;
use crate::metadata::MetadataEntry;
use crate::metadata::MetadataKey;

#[derive(Debug, Clone, PartialEq)]
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

#[derive(Debug, Clone, PartialEq)]
pub enum EventType {
    WriteFile(WriteFileEvent),
    ReadFile(ReadFileEvent),
    GetMetadata(GetMetadataEvent),
    SetMetadata(SetMetadataEvent),
}

// Can actually represent create/modify/delete
#[derive(Debug, Clone, PartialEq)]
pub struct WriteFileEvent {
    pub path: PathBuf,
    pub before_hash: Option<ContentHash>,
    pub after_hash: Option<ContentHash>,
}

// When called on a missing file, hash=None
#[derive(Debug, Clone, PartialEq)]
pub struct ReadFileEvent {
    pub path: PathBuf,
    pub hash: Option<ContentHash>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GetMetadataEvent {
    pub path: PathBuf,
    pub key: MetadataKey,
    pub value: Option<MetadataEntry>,
}

#[derive(Debug, Clone, PartialEq)]
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

impl From<ReadFileEvent> for EventType {
    fn from(value: ReadFileEvent) -> Self {
        EventType::ReadFile(value)
    }
}

impl From<ReadFileEvent> for Event {
    fn from(value: ReadFileEvent) -> Self {
        let event_type = value.into();
        Event {
            id: 0,
            group_id: 0,
            event_type,
        }
    }
}

impl From<GetMetadataEvent> for EventType {
    fn from(value: GetMetadataEvent) -> Self {
        EventType::GetMetadata(value)
    }
}

impl From<GetMetadataEvent> for Event {
    fn from(value: GetMetadataEvent) -> Self {
        let event_type = value.into();
        Event {
            id: 0,
            group_id: 0,
            event_type,
        }
    }
}

impl From<SetMetadataEvent> for EventType {
    fn from(value: SetMetadataEvent) -> Self {
        EventType::SetMetadata(value)
    }
}

impl From<SetMetadataEvent> for Event {
    fn from(value: SetMetadataEvent) -> Self {
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
    pub id: u64,
    pub command: String,
    pub events: Vec<Event>,
    pub is_most_recent_run: bool,
}
impl EventGroup {
    pub(crate) fn empty() -> EventGroup {
        EventGroup {
            id: 0,
            command: "unknown".to_string(),
            events: vec![],
            is_most_recent_run: true,
        }
    }
}
