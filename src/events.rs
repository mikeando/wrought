use std::path::PathBuf;

use crate::binary16::ContentHash;
use crate::metadata::MetadataEntry;
use crate::metadata::MetadataKey;

#[derive(Debug)]
pub struct Event {
    pub id: u64,
    pub group_id: u64,
    pub event_type: EventType,
}

#[derive(Debug)]
pub enum EventType {
    WriteFile(WriteFileEvent),
    ReadFile(ReadFileEvent),
    GetMetadata(GetMetadataEvent),
    SetMetadata(SetMetadataEvent),
}

// Can actually represent create/modify/delete
#[derive(Debug)]
pub struct WriteFileEvent {
    pub path: PathBuf,
    pub before_hash: Option<ContentHash>,
    pub after_hash: Option<ContentHash>,
}

// When called on a missing file, hash=None
#[derive(Debug)]
pub struct ReadFileEvent {
    pub path: PathBuf,
    pub hash: Option<ContentHash>,
}

#[derive(Debug)]
pub struct GetMetadataEvent {
    pub path: PathBuf,
    pub key: MetadataKey,
    pub value: Option<MetadataEntry>,
}

#[derive(Debug)]
pub struct SetMetadataEvent {
    pub path: PathBuf,
    pub key: MetadataKey,
    pub before_value: Option<MetadataEntry>,
    pub after_value: Option<MetadataEntry>,
}

#[derive(Debug)]
pub struct EventGroup {
    pub command: String,
    pub events: Vec<Event>,
    pub is_most_recent_run: bool,
}
