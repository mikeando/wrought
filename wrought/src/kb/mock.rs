#[cfg(test)]
pub mod internal {
    use mockall::mock;

    use crate::binary16::ContentHash;
    use crate::kb::document::DocumentBase;
    use crate::kb::document::DocumentId;
    use crate::kb::full_text_index::FullTextIndex;
    use crate::kb::graph_index::Connection;
    use crate::kb::graph_index::GraphIndex;
    use crate::kb::kb_impl::KnowledgeBaseImpl;
    use crate::kb::knowledgebase::KnowledgeBaseError;
    use crate::kb::metadata_store::MetadataStore;
    use crate::kb::vector_index;
    use crate::kb::vector_index::VectorIndex;

    mock! {
        pub KnowledgeBaseImpl {}

        impl KnowledgeBaseImpl for KnowledgeBaseImpl {
            fn all_docs(&self) -> Vec<DocumentId>;
            fn new_document(&mut self) -> Result<DocumentId, KnowledgeBaseError>;
            fn delete_document(&mut self, doc_id: &DocumentId) -> Result<(), KnowledgeBaseError>;
            fn get_doc_base(&self, doc_id: &DocumentId) -> Result<Option<DocumentBase>, KnowledgeBaseError>;
            fn add_tag(&mut self, doc_id: &DocumentId, tag: &str) -> Result<(), KnowledgeBaseError>;
            fn set_content(&mut self, doc_id: &DocumentId, content_hash: &ContentHash) -> Result<(), KnowledgeBaseError>;
        }
    }

    mock! {
        pub FullTextIndex {}

        impl FullTextIndex for FullTextIndex {
            fn index_document(&self, document_id: &DocumentId, content: &str) -> anyhow::Result<()>;
            fn search(&self, query_str: &str, limit: usize) -> anyhow::Result<Vec<(f32, DocumentId)>>;
        }
    }

    mock! {
        pub VectorIndex {}

        impl VectorIndex for VectorIndex {
            fn key_length(&self) -> usize;
            fn index_document(&self, document_id: &DocumentId, content: &str) -> vector_index::Result<()>;
            fn key_from_content(&self, content: &str) -> vector_index::Result<Vec<f32>>;
            fn search(&self, key: &[f32], limit: Option<usize>, min_similarity: Option<f32>) -> vector_index::Result<Vec<(f32, DocumentId)>>;
            fn put(&mut self, document_id: &DocumentId, key: Vec<f32>) -> vector_index::Result<()>;
            fn keys_for_doc_id(&self, document_id: &DocumentId) -> vector_index::Result<Vec<Vec<f32>>>;
            fn add_multiple_raw_for_doc(&mut self, document_id: &DocumentId, keys: Vec<Vec<f32>>) -> vector_index::Result<()>;
        }
    }

    mock! {
        pub MetadataStore {}

        impl MetadataStore for MetadataStore {
            fn get_metadata(&self, v: &DocumentId, key: &str) -> Option<String>;
            fn get_all_metadata(&self, doc_id: &DocumentId) -> std::collections::BTreeMap<String,String> ;
            fn set_all_metadata(&mut self, doc_id: &DocumentId, metadata: &std::collections::BTreeMap<String,String> ) -> anyhow::Result<()>;
        }
    }

    mock! {
        pub GraphIndex {}
        impl GraphIndex for GraphIndex {
            fn connections_for_doc_id(&self, doc_id: &DocumentId) -> anyhow::Result<Vec<Connection>>;
            fn add_connections(&mut self, doc_id: &DocumentId, connections:&[Connection]) -> anyhow::Result<()>;
        }
    }
}
