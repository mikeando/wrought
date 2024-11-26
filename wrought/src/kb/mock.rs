#[cfg(test)]
pub mod internal {
    use mockall::mock;

    use crate::kb::kb_impl::KnowledgeBaseImpl;
    use crate::kb::document::Document;
    use crate::kb::document::DocumentId;
    use crate::kb::knowledgebase::KnowledgeBaseError;


    mock! {
        pub KnowledgeBaseImpl {}

        impl KnowledgeBaseImpl for KnowledgeBaseImpl {
            fn all_docs(&self) -> Vec<Document>;

            fn add_document(&self, doc: Document) -> Result<(), KnowledgeBaseError>;
            fn update_document(&self, doc: Document) -> Result<(), KnowledgeBaseError>;
            fn delete_document(&self, doc_id: &DocumentId) -> Result<(), KnowledgeBaseError>;
            fn get_document(&self, doc_id: &DocumentId) -> Option<Document>;
        }
    }
}