use super::{
    document::{Document, DocumentId},
    kb_impl::KnowledgeBaseImpl,
    knowledgebase::KnowledgeBaseError,
};

pub struct InMemoryKnowledgeBase {
    documents: Vec<Document>,
}

impl InMemoryKnowledgeBase {
    pub fn new() -> Self {
        InMemoryKnowledgeBase { documents: vec![] }
    }
}

impl KnowledgeBaseImpl for InMemoryKnowledgeBase {
    fn all_docs(&self) -> Vec<Document> {
        self.documents.clone()
    }

    fn add_document(&self, doc: Document) -> Result<(), KnowledgeBaseError> {
        todo!();
    }
    fn update_document(&self, doc: Document) -> Result<(), KnowledgeBaseError> {
        todo!();
    }
    fn delete_document(&self, doc_id: &DocumentId) -> Result<(), KnowledgeBaseError> {
        todo!();
    }
    fn get_document(&self, doc_id: &DocumentId) -> Option<Document> {
        todo!();
    }
}
