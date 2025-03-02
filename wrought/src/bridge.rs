use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use crate::{
    backend::Backend, events::{
        Event, EventGroup, GetMetadataEvent, ReadFileEvent, SetMetadataEvent, WriteFileEvent,
    }, kb::knowledgebase::KnowledgeBaseTrait, llm::{AiMessage, LLM}, metadata::{MetadataEntry, MetadataKey}
};

// TODO: Move this.
pub struct KBDoc {
    pub content: String,
    pub tags: Vec<String>,
}

impl std::fmt::Debug for KBDoc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {

        let content = if self.content.len() > 50 {
            // TODO: This doesn't work for non-single-byte utf-8 characters.
            format!("{}...{}", &self.content[..23], &self.content[(self.content.len()-23)..])
        } else {
            self.content.clone()
        };
        f.debug_struct("KBDoc").field("content", &content).field("tags", &self.tags).finish()
    }
}

pub trait Bridge {
    fn write_file(&mut self, path: &Path, value: &[u8]) -> anyhow::Result<()>;
    fn read_file(&mut self, path: &Path) -> anyhow::Result<Option<Vec<u8>>>;
    fn get_metadata(&mut self, path: &Path, key: &str) -> anyhow::Result<Option<String>>;
    fn set_metadata(&mut self, path: &Path, key: &str, value: &str) -> anyhow::Result<()>;
    fn get_event_group(&self) -> Option<EventGroup>;

    fn ai_query(&mut self, query: &str) -> anyhow::Result<String>;
    fn ai_message(&mut self) -> anyhow::Result<AiMessage>;
    fn send_ai_message(&mut self, message: &AiMessage) -> anyhow::Result<serde_json::Value>;

    fn kb_ingest(&mut self, doc: &KBDoc) -> anyhow::Result<()>;
}

pub struct SimpleBridge {
    pub backend: Arc<Mutex<dyn Backend + Send + 'static>>,
    // pub event_log: Arc<Mutex< dyn EventLog >>,
    pub llm: Arc<Mutex<dyn LLM + Send + 'static>>,
    pub kb: Arc<Mutex<dyn KnowledgeBaseTrait + Send + 'static>>,
    pub root: PathBuf,

    pub event_group: EventGroup,
}

impl Bridge for SimpleBridge {
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

    fn read_file(&mut self, path: &Path) -> anyhow::Result<Option<Vec<u8>>> {
        let v = self.backend.lock().unwrap().read_file(path)?;
        let (content_hash, content) = match v {
            Some((content_hash, content)) => (Some(content_hash), Some(content)),
            None => (None, None),
        };
        let event = ReadFileEvent {
            path: path.to_path_buf(),
            hash: content_hash,
        };
        self.add_event(event.into());
        Ok(content)
    }

    fn get_metadata(&mut self, path: &Path, key: &str) -> anyhow::Result<Option<String>> {
        let key = MetadataKey::from(key);
        let v = self.backend.lock().unwrap().get_metadata(path, &key)?;
        let event = GetMetadataEvent {
            path: path.to_path_buf(),
            key: key.clone(),
            value: v.clone(),
        };
        self.add_event(event.into());
        Ok(v.map(|v| v.as_string()))
    }

    fn set_metadata(&mut self, path: &Path, key: &str, value: &str) -> anyhow::Result<()> {
        let key = MetadataKey::from(key);
        let v = MetadataEntry::from(value);
        let v = Some(v);
        let before_value = self.backend.lock().unwrap().set_metadata(path, &key, &v)?;
        let event = SetMetadataEvent {
            path: path.to_path_buf(),
            key,
            before_value,
            after_value: v,
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

    fn ai_query(&mut self, query: &str) -> anyhow::Result<String> {
        self.llm.lock().unwrap().query(query)
    }

    fn ai_message(&mut self) -> anyhow::Result<AiMessage> {
        Ok(AiMessage::default())
    }

    fn send_ai_message(&mut self, message: &AiMessage) -> anyhow::Result<serde_json::Value> {
        self.llm.lock().unwrap().send_ai_message(message)
    }

    fn kb_ingest(&mut self, doc: &KBDoc) -> anyhow::Result<()> {
        eprintln!("kb_ingest: doc = {:?}", doc);
        self.kb.lock().unwrap().ingest(doc)?;
        Ok(())
    }
}

impl SimpleBridge {
    pub fn add_event(&mut self, event: Event) {
        self.event_group.events.push(event);
    }
}
