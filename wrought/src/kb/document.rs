use std::{collections::HashMap, sync::{Arc, Mutex}};

use crate::{binary16::ContentHash, content_store::ContentStore};

#[derive(Clone)]
pub struct DocumentId {
    // TODO: unpub this
    pub id: ContentHash
}

#[derive(Clone)]
pub struct Document {
    id: DocumentId,
    metadata: HashMap<String, String>,
    content_store: Arc<Mutex<dyn ContentStore + Send + Sync>>,
}

impl Document {
    /// Create a new document with the given ID and metadata.
    pub fn new(
        id: DocumentId,
        metadata: HashMap<String, String>,
        content_store: Arc<Mutex<dyn ContentStore + Send + Sync>>,
    ) -> Self {
        Document {
            id,
            metadata,
            content_store,
        }
    }

    /// Retrieve the document ID.
    pub fn id(&self) -> &DocumentId {
        &self.id
    }

    pub fn content(&self) -> anyhow::Result<Option<Vec<u8>>> {
        self.content_store.lock().unwrap().retrieve(self.id.id.clone())
    }

    /// Get metadata by key.
    pub fn get_metadata(&self, key: &str) -> Option<&String> {
        self.metadata.get(key)
    }

    /// Add or update a metadata field.
    pub fn set_metadata(&mut self, key: String, value: String) {
        self.metadata.insert(key, value);
    }

    /// Remove a metadata field.
    pub fn remove_metadata(&mut self, key: &str) -> Option<String> {
        self.metadata.remove(key)
    }
}