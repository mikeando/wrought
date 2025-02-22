use std::collections::BTreeMap;

use crate::binary16::ContentHash;
pub use crate::binary16::DocumentId;

use super::{graph_index::Connection, vector_index::ChunkMetadata};

#[derive(PartialEq, Debug, Clone)]
pub struct DocumentBase {
    pub doc_id: DocumentId,
    pub tags: Vec<String>,
    pub content_hash: Option<ContentHash>,
}

#[derive(PartialEq, Debug, Clone)]
pub struct DocumentInfo {
    pub metadata: BTreeMap<String, String>,
    pub tags: Vec<String>,
    pub content: Option<Vec<u8>>,
    pub vector_keys: Vec<(Vec<f32>, ChunkMetadata)>,
    pub connections: Vec<Connection>,
}
