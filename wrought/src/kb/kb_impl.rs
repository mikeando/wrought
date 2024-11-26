use crate::kb::knowledgebase::KnowledgeBaseError;

use super::document::{Document, DocumentId};

pub trait KnowledgeBaseImpl {
    fn all_docs(&self) -> Vec<Document>;

    // Basic CRUD opts?
    fn add_document(&self, doc: Document) -> Result<(), KnowledgeBaseError>;
    fn update_document(&self, doc: Document) -> Result<(), KnowledgeBaseError>;
    fn delete_document(&self, doc_id: &DocumentId) -> Result<(), KnowledgeBaseError>;
    fn get_document(&self, doc_id: &DocumentId) -> Option<Document>;
}
