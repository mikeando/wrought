use std::collections::BTreeMap;

use super::{
    document::{DocumentBase, DocumentId},
    kb_impl::KnowledgeBaseImpl,
    knowledgebase::KnowledgeBaseError,
};

#[derive(Default)]
pub struct InMemoryKnowledgeBaseImpl {
    last_id_seed: u64,
    docs: BTreeMap<DocumentId, DocumentBase>,
}

impl KnowledgeBaseImpl for InMemoryKnowledgeBaseImpl {
    fn all_docs(&self) -> Vec<DocumentId> {
        self.docs.keys().cloned().collect()
    }

    fn new_document(&mut self) -> Result<DocumentId, KnowledgeBaseError> {
        use sha2::Digest;

        self.last_id_seed += 1;
        let mut seed_content = b"crazy_seed".to_vec();
        seed_content.append(&mut self.last_id_seed.to_le_bytes().to_vec());
        let digest = sha2::Sha256::digest(seed_content);
        let doc_id = DocumentId::from_raw(digest.as_slice()[0..16].try_into().unwrap());
        let base = DocumentBase {
            doc_id: doc_id.clone(),
            tags: vec![],
            content_hash: None,
        };
        self.docs.insert(doc_id.clone(), base);
        Ok(doc_id)
    }

    fn delete_document(&mut self, doc_id: &DocumentId) -> Result<(), KnowledgeBaseError> {
        match self.docs.remove(doc_id) {
            Some(_v) => Ok(()),
            None => Err(KnowledgeBaseError::DocumentNotFound),
        }
    }

    fn get_doc_base(
        &self,
        doc_id: &DocumentId,
    ) -> Result<Option<DocumentBase>, KnowledgeBaseError> {
        let base = self.docs.get(doc_id);
        let base = base.cloned();
        Ok(base)
    }

    fn add_tag(&mut self, doc_id: &DocumentId, tag: &str) -> Result<(), KnowledgeBaseError> {
        let Some(doc_base) = self.docs.get_mut(&doc_id) else {
            return Err(KnowledgeBaseError::DocumentNotFound);
        };
        doc_base.tags.push(tag.to_string());
        Ok(())
    }

    fn set_content(
        &mut self,
        doc_id: &DocumentId,
        content_hash: &crate::binary16::ContentHash,
    ) -> Result<(), KnowledgeBaseError> {
        let Some(doc_base) = self.docs.get_mut(&doc_id) else {
            return Err(KnowledgeBaseError::DocumentNotFound);
        };
        doc_base.content_hash = Some(content_hash.clone());
        Ok(())
    }
}

// Tests for the InMemoryKnowledgeBase
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_and_get_document() {
        let mut kb = InMemoryKnowledgeBaseImpl::default();
        let doc_id = kb.new_document().expect("Should add document successfully");
        assert_eq!(vec![doc_id], kb.all_docs());
    }

    #[test]
    fn test_delete_document() {
        let mut kb = InMemoryKnowledgeBaseImpl::default();
        let doc_id = kb.new_document().expect("Should add document");
        kb.delete_document(&doc_id).expect("Should delete document");
        assert_eq!(Vec::<DocumentId>::new(), kb.all_docs());
    }

    #[test]
    fn test_delete_nonexistent_document() {
        let mut kb = InMemoryKnowledgeBaseImpl::default();
        let doc_id = DocumentId::from_u64s(100, 200);
        let result = kb.delete_document(&doc_id);
        assert!(matches!(result, Err(KnowledgeBaseError::DocumentNotFound)));
    }
}
