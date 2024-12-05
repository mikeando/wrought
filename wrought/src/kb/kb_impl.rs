use crate::kb::knowledgebase::KnowledgeBaseError;

use super::document::{Document, DocumentContent, DocumentId};

pub trait KnowledgeBaseImpl {
    fn all_docs(&self) -> Vec<DocumentId>;

    // Basic CRUD opts?
    fn add_document(&mut self, doc: DocumentContent) -> Result<DocumentId, KnowledgeBaseError>;
    fn update_document(&mut self, doc: Document) -> Result<(), KnowledgeBaseError>;
    fn delete_document(&mut self, doc_id: &DocumentId) -> Result<(), KnowledgeBaseError>;
    fn get_document(&self, doc_id: &DocumentId) -> Option<Document>;
}
