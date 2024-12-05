use crate::kb::knowledgebase::KnowledgeBaseError;

use super::document::DocumentId;

pub trait KnowledgeBaseImpl {
    fn all_docs(&self) -> Vec<DocumentId>;
    fn new_document(&mut self) -> Result<DocumentId, KnowledgeBaseError>;
    fn delete_document(&mut self, doc_id: &DocumentId) -> Result<(), KnowledgeBaseError>;
}
