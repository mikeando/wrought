use crate::{binary16::ContentHash, kb::knowledgebase::KnowledgeBaseError};

use super::document::{DocumentBase, DocumentId};

pub trait KnowledgeBaseImpl {
    fn all_docs(&self) -> Vec<DocumentId>;
    fn new_document(&mut self) -> Result<DocumentId, KnowledgeBaseError>;
    fn delete_document(&mut self, doc_id: &DocumentId) -> Result<(), KnowledgeBaseError>;
    fn get_doc_base(&self, doc_id: &DocumentId)
        -> Result<Option<DocumentBase>, KnowledgeBaseError>;
    fn add_tag(&mut self, doc_id: &DocumentId, tag: &str) -> Result<(), KnowledgeBaseError>;
    fn set_content(
        &mut self,
        doc_id: &DocumentId,
        content_hash: &ContentHash,
    ) -> Result<(), KnowledgeBaseError>;
}
