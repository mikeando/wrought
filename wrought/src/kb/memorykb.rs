use std::sync::Mutex;

use super::{document::DocumentId, kb_impl::KnowledgeBaseImpl, knowledgebase::KnowledgeBaseError};

pub struct InMemoryKnowledgeBase {
    last_id_seed: usize,
    documents: Mutex<Vec<DocumentId>>,
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
            .map(|doc_id| doc_id.clone())
            .collect()
    }

    fn new_document(&mut self) -> Result<DocumentId, KnowledgeBaseError> {
        use sha2::Digest;

        self.last_id_seed += 1;
        let mut seed_content = b"crazy_seed".to_vec();
        seed_content.append(&mut self.last_id_seed.to_le_bytes().to_vec());
        let digest = sha2::Sha256::digest(seed_content);
        let doc_id = DocumentId::from_raw(digest.as_slice()[0..16].try_into().unwrap());

        let mut docs = self.documents.lock().unwrap();
        docs.push(doc_id.clone());

        Ok(doc_id)
    }

    fn delete_document(&mut self, doc_id: &DocumentId) -> Result<(), KnowledgeBaseError> {
        let mut docs = self.documents.lock().unwrap();

        // Find and remove the document with matching ID
        if let Some(index) = docs.iter().position(|doc| doc == doc_id) {
            docs.remove(index);
            Ok(())
        } else {
            Err(KnowledgeBaseError::DocumentNotFound)
        }
    }
}

// Tests for the InMemoryKnowledgeBase
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_and_get_document() {
        let mut kb = InMemoryKnowledgeBase::new();
        let doc_id = kb.new_document().expect("Should add document successfully");
        assert_eq!(vec![doc_id], kb.all_docs());
    }

    #[test]
    fn test_delete_document() {
        let mut kb = InMemoryKnowledgeBase::new();
        let doc_id = kb.new_document().expect("Should add document");
        kb.delete_document(&doc_id).expect("Should delete document");
        assert_eq!(Vec::<DocumentId>::new(), kb.all_docs());
    }

    #[test]
    fn test_delete_nonexistent_document() {
        let mut kb = InMemoryKnowledgeBase::new();
        let doc_id = DocumentId::from_u64s(100, 200);
        let result = kb.delete_document(&doc_id);
        assert!(matches!(result, Err(KnowledgeBaseError::DocumentNotFound)));
    }
}
