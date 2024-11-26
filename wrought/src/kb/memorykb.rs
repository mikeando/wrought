use std::sync::Mutex;

use super::{
    document::{Document, DocumentId},
    kb_impl::KnowledgeBaseImpl,
    knowledgebase::KnowledgeBaseError,
};

pub struct InMemoryKnowledgeBase {
    documents: Mutex<Vec<Document>>,
}

impl InMemoryKnowledgeBase {
    pub fn new() -> Self {
        InMemoryKnowledgeBase {
            documents: Mutex::new(vec![]),
        }
    }
}

impl KnowledgeBaseImpl for InMemoryKnowledgeBase {
    fn all_docs(&self) -> Vec<Document> {
        self.documents.lock().unwrap().clone()
    }

    fn add_document(&self, doc: Document) -> Result<(), KnowledgeBaseError> {
        let mut docs = self.documents.lock().unwrap();

        // Check if a document with the same ID already exists
        if docs.iter().any(|existing| existing.id() == doc.id()) {
            return Err(KnowledgeBaseError::DocumentAlreadyExists);
        }

        docs.push(doc);
        Ok(())
    }

    fn update_document(&self, doc: Document) -> Result<(), KnowledgeBaseError> {
        let mut docs = self.documents.lock().unwrap();

        // Find and replace the document with matching ID
        if let Some(index) = docs.iter().position(|existing| existing.id() == doc.id()) {
            docs[index] = doc;
            Ok(())
        } else {
            Err(KnowledgeBaseError::DocumentNotFound)
        }
    }

    fn delete_document(&self, doc_id: &DocumentId) -> Result<(), KnowledgeBaseError> {
        let mut docs = self.documents.lock().unwrap();

        // Find and remove the document with matching ID
        if let Some(index) = docs.iter().position(|doc| doc.id() == doc_id) {
            docs.remove(index);
            Ok(())
        } else {
            Err(KnowledgeBaseError::DocumentNotFound)
        }
    }

    fn get_document(&self, doc_id: &DocumentId) -> Option<Document> {
        let docs = self.documents.lock().unwrap();
        docs.iter().find(|doc| doc.id() == doc_id).cloned()
    }
}

// Tests for the InMemoryKnowledgeBase
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{binary16::ContentHash, content_store::mocks::MockContentStore};
    use std::{collections::HashMap, sync::Arc};

    fn create_test_document(
        content: &str,
        content_store: Arc<Mutex<MockContentStore>>,
    ) -> Document {
        let hash = ContentHash::from_content(content.as_bytes());
        let doc_id = DocumentId { id: hash };

        let mut metadata = HashMap::new();
        metadata.insert("test_key".to_string(), "test_value".to_string());

        Document::new(doc_id, metadata, content_store)
    }

    #[test]
    fn test_add_and_get_document() {
        let content_store = Arc::new(Mutex::new(MockContentStore::new()));
        let kb = InMemoryKnowledgeBase::new();

        let doc = create_test_document("Hello, world!", content_store.clone());
        let doc_id = doc.id().clone();

        // Add document
        kb.add_document(doc.clone())
            .expect("Should add document successfully");

        // Retrieve document
        let retrieved_doc = kb.get_document(&doc_id).expect("Should retrieve document");
        assert_eq!(retrieved_doc.id(), &doc_id);
        assert_eq!(
            retrieved_doc.get_metadata("test_key"),
            Some(&"test_value".to_string())
        );
    }

    #[test]
    fn test_duplicate_document_add() {
        let content_store = Arc::new(Mutex::new(MockContentStore::new()));
        let kb = InMemoryKnowledgeBase::new();

        let doc = create_test_document("Hello, world!", content_store.clone());

        // First add should succeed
        kb.add_document(doc.clone())
            .expect("First add should succeed");

        // Second add with same ID should fail
        let result = kb.add_document(doc.clone());
        assert!(matches!(
            result,
            Err(KnowledgeBaseError::DocumentAlreadyExists)
        ));
    }

    #[test]
    fn test_update_document() {
        let content_store = Arc::new(Mutex::new(MockContentStore::new()));
        let kb = InMemoryKnowledgeBase::new();

        let mut doc = create_test_document("Hello, world!", content_store.clone());
        let doc_id = doc.id().clone();

        // Add initial document
        kb.add_document(doc.clone()).expect("Should add document");

        // Update metadata
        doc.set_metadata("updated_key".to_string(), "updated_value".to_string());
        kb.update_document(doc.clone())
            .expect("Should update document");

        // Retrieve and verify
        let updated_doc = kb
            .get_document(&doc_id)
            .expect("Should retrieve updated document");
        assert_eq!(
            updated_doc.get_metadata("updated_key"),
            Some(&"updated_value".to_string())
        );
    }

    #[test]
    fn test_delete_document() {
        let content_store = Arc::new(Mutex::new(MockContentStore::new()));
        let kb = InMemoryKnowledgeBase::new();

        let doc = create_test_document("Hello, world!", content_store.clone());
        let doc_id = doc.id().clone();

        // Add document
        kb.add_document(doc).expect("Should add document");

        // Delete document
        kb.delete_document(&doc_id).expect("Should delete document");

        // Verify document is gone
        assert!(kb.get_document(&doc_id).is_none());
    }

    #[test]
    fn test_delete_nonexistent_document() {
        let kb = InMemoryKnowledgeBase::new();

        let doc_id = DocumentId {
            id: ContentHash::from_content(b"test"),
        };

        // Delete should fail
        let result = kb.delete_document(&doc_id);
        assert!(matches!(result, Err(KnowledgeBaseError::DocumentNotFound)));
    }

    #[test]
    fn test_update_nonexistent_document() {
        let content_store = Arc::new(Mutex::new(MockContentStore::new()));
        let kb = InMemoryKnowledgeBase::new();

        let doc = create_test_document("Hello, world!", content_store.clone());

        // Update should fail
        let result = kb.update_document(doc);
        assert!(matches!(result, Err(KnowledgeBaseError::DocumentNotFound)));
    }
}
