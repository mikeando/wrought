use std::sync::Mutex;

use super::{
    document::{Document, DocumentContent, DocumentId},
    kb_impl::KnowledgeBaseImpl,
    knowledgebase::KnowledgeBaseError,
};

pub struct InMemoryKnowledgeBase {
    last_id_seed: usize,
    documents: Mutex<Vec<Document>>,
}

impl InMemoryKnowledgeBase {
    pub fn new() -> Self {
        InMemoryKnowledgeBase {
            last_id_seed: 0,
            documents: Mutex::new(vec![]),
        }
    }
}

impl KnowledgeBaseImpl for InMemoryKnowledgeBase {
    fn all_docs(&self) -> Vec<DocumentId> {
        self.documents
            .lock()
            .unwrap()
            .iter()
            .map(|doc| doc.id.clone())
            .collect()
    }

    fn add_document(&mut self, content: DocumentContent) -> Result<DocumentId, KnowledgeBaseError> {
        use sha2::Digest;

        self.last_id_seed += 1;
        let mut seed_content = b"crazy_seed".to_vec();
        seed_content.append(&mut self.last_id_seed.to_le_bytes().to_vec());
        let digest = sha2::Sha256::digest(seed_content);
        let doc_id = DocumentId::from_raw(digest.as_slice()[0..16].try_into().unwrap());

        let mut docs = self.documents.lock().unwrap();

        docs.push(Document {
            id: doc_id.clone(),
            content,
        });
        Ok(doc_id)
    }

    fn update_document(&mut self, doc: Document) -> Result<(), KnowledgeBaseError> {
        let mut docs = self.documents.lock().unwrap();

        // Find and replace the document with matching ID
        if let Some(index) = docs.iter().position(|existing| existing.id == doc.id) {
            docs[index] = doc;
            Ok(())
        } else {
            Err(KnowledgeBaseError::DocumentNotFound)
        }
    }

    fn delete_document(&mut self, doc_id: &DocumentId) -> Result<(), KnowledgeBaseError> {
        let mut docs = self.documents.lock().unwrap();

        // Find and remove the document with matching ID
        if let Some(index) = docs.iter().position(|doc| doc.id == *doc_id) {
            docs.remove(index);
            Ok(())
        } else {
            Err(KnowledgeBaseError::DocumentNotFound)
        }
    }

    fn get_document(&self, doc_id: &DocumentId) -> Option<Document> {
        self.documents
            .lock()
            .unwrap()
            .iter()
            .find(|e| e.id == *doc_id)
            .cloned()
    }

    // fn get_document(&self, doc_id: &DocumentId) -> Option<Document> {
    //     let docs = self.documents.lock().unwrap();
    //     docs.iter().find(|doc| doc.id() == doc_id).cloned()
    // }
}

// Tests for the InMemoryKnowledgeBase
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn create_test_document(doc_id: DocumentId, content: &str) -> Document {
        let mut metadata = HashMap::new();
        metadata.insert("test_key".to_string(), "test_value".to_string());

        Document {
            id: doc_id,
            content: DocumentContent {
                content: content.to_string(),
            },
        }
    }

    #[test]
    fn test_add_and_get_document() {
        let mut kb = InMemoryKnowledgeBase::new();

        let doc = create_test_document(DocumentId::from_u64s(100, 200), "Hello, world!");
        let doc_id = doc.id.clone();

        // Add document
        kb.add_document(doc.content)
            .expect("Should add document successfully");

        // Retrieve document
        let retrieved_doc = kb.get_document(&doc_id).expect("Should retrieve document");
        assert_eq!(retrieved_doc.id, doc_id);
    }

    #[test]
    fn test_duplicate_document_add() {
        let mut kb = InMemoryKnowledgeBase::new();

        let doc = create_test_document(DocumentId::from_u64s(100, 200), "Hello, world!");

        // First add should succeed
        kb.add_document(doc.content.clone())
            .expect("First add should succeed");

        // Second add with same ID should fail
        let result = kb.add_document(doc.content);
        assert!(matches!(
            result,
            Err(KnowledgeBaseError::DocumentAlreadyExists)
        ));
    }

    #[test]
    fn test_update_document() {
        todo!();
    }

    #[test]
    fn test_delete_document() {
        let mut kb = InMemoryKnowledgeBase::new();

        let doc = create_test_document(DocumentId::from_u64s(100, 200), "Hello, world!");
        let doc_id = doc.id.clone();

        // Add document
        kb.add_document(doc.content).expect("Should add document");

        // Delete document
        kb.delete_document(&doc_id).expect("Should delete document");

        // Verify document is gone
        assert!(kb.get_document(&doc_id).is_none());
    }

    #[test]
    fn test_delete_nonexistent_document() {
        let mut kb = InMemoryKnowledgeBase::new();

        let doc_id = DocumentId::from_u64s(100, 200);

        // Delete should fail
        let result = kb.delete_document(&doc_id);
        assert!(matches!(result, Err(KnowledgeBaseError::DocumentNotFound)));
    }

    #[test]
    fn test_update_nonexistent_document() {
        let mut kb = InMemoryKnowledgeBase::new();

        let doc = create_test_document(DocumentId::from_u64s(100, 200), "Hello, world!");

        // Update should fail
        let result = kb.update_document(doc);
        assert!(matches!(result, Err(KnowledgeBaseError::DocumentNotFound)));
    }
}
