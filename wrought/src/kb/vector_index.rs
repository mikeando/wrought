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
}
