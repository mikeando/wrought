use std::collections::BTreeMap;

use super::document::DocumentId;

pub trait MetadataStore {
    fn get_metadata(&self, v: &DocumentId, key: &str) -> Option<String>;
    fn get_all_metadata(&self, doc_id: &DocumentId) -> BTreeMap<String, String>;
    fn set_all_metadata(
        &mut self,
        doc_id: &DocumentId,
        metadata: &BTreeMap<String, String>,
    ) -> anyhow::Result<()>;
}

#[derive(Default)]
pub struct InMemoryMetadataStore {
    metadata: BTreeMap<DocumentId, BTreeMap<String, String>>,
}

impl MetadataStore for InMemoryMetadataStore {
    fn get_metadata(&self, v: &DocumentId, key: &str) -> Option<String> {
        self.metadata.get(v).and_then(|md| md.get(key)).cloned()
    }

    fn set_all_metadata(
        &mut self,
        doc_id: &DocumentId,
        metadata: &BTreeMap<String, String>,
    ) -> anyhow::Result<()> {
        self.metadata.insert(doc_id.clone(), metadata.clone());
        Ok(())
    }

    fn get_all_metadata(&self, doc_id: &DocumentId) -> BTreeMap<String, String> {
        match self.metadata.get(doc_id) {
            Some(v) => v.clone(),
            None => BTreeMap::new(),
        }
    }
}
