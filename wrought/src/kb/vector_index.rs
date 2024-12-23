use std::collections::BTreeMap;

use super::document::DocumentId;

#[derive(Debug)]
pub enum VectorIndexError {
    VectorSizeMismatch { expected: usize, actual: usize },
    DocumentTooLong,
}

pub type Result<T> = std::result::Result<T, VectorIndexError>;

pub trait VectorIndex {
    fn key_length(&self) -> usize;
    fn index_document(&self, document_id: &DocumentId, content: &str) -> Result<()>;
    fn key_from_content(&self, content: &str) -> Result<Vec<f32>>;
    fn put(&mut self, document_id: &DocumentId, key: Vec<f32>) -> Result<()>;
    fn search(
        &self,
        key: &[f32],
        limit: Option<usize>,
        min_similarity: Option<f32>,
    ) -> Result<Vec<(f32, DocumentId)>>;

    fn keys_for_doc_id(&self, document_id: &DocumentId) -> Result<Vec<Vec<f32>>>;
    fn add_multiple_raw_for_doc(
        &mut self,
        document_id: &DocumentId,
        keys: Vec<Vec<f32>>,
    ) -> Result<()>;
}

#[derive(Default)]
pub struct InMemoryVectorIndex {
    doc_to_keys: BTreeMap<DocumentId, Vec<Vec<f32>>>,
}

impl VectorIndex for InMemoryVectorIndex {
    fn key_length(&self) -> usize {
        todo!()
    }

    fn index_document(&self, document_id: &DocumentId, content: &str) -> Result<()> {
        todo!()
    }

    fn key_from_content(&self, content: &str) -> Result<Vec<f32>> {
        todo!()
    }

    fn put(&mut self, document_id: &DocumentId, key: Vec<f32>) -> Result<()> {
        todo!()
    }

    fn search(
        &self,
        key: &[f32],
        limit: Option<usize>,
        min_similarity: Option<f32>,
    ) -> Result<Vec<(f32, DocumentId)>> {
        todo!()
    }

    fn keys_for_doc_id(&self, document_id: &DocumentId) -> Result<Vec<Vec<f32>>> {
        match self.doc_to_keys.get(document_id) {
            Some(keys) => Ok(keys.clone()),
            None => Ok(vec![]),
        }
    }

    fn add_multiple_raw_for_doc(
        &mut self,
        document_id: &DocumentId,
        keys: Vec<Vec<f32>>,
    ) -> Result<()> {
        let mut keys = keys;
        self.doc_to_keys
            .entry(document_id.clone())
            .or_default()
            .append(&mut keys);
        Ok(())
    }
}
