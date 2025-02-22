use core::str;
use std::{
    collections::HashSet,
    sync::{Arc, Mutex}, vec,
};

use crate::{binary16::ContentHash, content_store::ContentStore};

use super::{
    document::DocumentId, kb_impl::KnowledgeBaseImpl, metadata_store::MetadataStore, query::Query,
    vector_index::VectorIndex,
};
use super::{document::DocumentInfo, full_text_index::FullTextIndex, graph_index::GraphIndex};

#[derive(Clone)]
pub struct KnowledgeBase {
    pub kb: Arc<Mutex<dyn KnowledgeBaseImpl>>,

    pub content_store: Arc<Mutex<dyn ContentStore + Send + Sync>>,
    pub full_text_index: Arc<Mutex<dyn FullTextIndex + Send + Sync>>,
    pub vector_index: Arc<Mutex<dyn VectorIndex + Send + Sync>>,
    pub metadata_store: Arc<Mutex<dyn MetadataStore + Send + Sync>>,
    pub graph_index: Arc<Mutex<dyn GraphIndex + Send + Sync>>,
}

impl KnowledgeBase {
    pub fn get_content(&self, hash: &ContentHash) -> anyhow::Result<Option<Vec<u8>>> {
        self.content_store.lock().unwrap().retrieve(hash.clone())
    }

    fn query(&self, query: Query) -> Vec<DocumentId> {
        match query {
            Query::AllDocs => self.kb.lock().unwrap().all_docs(),
            Query::EmptyStream => vec![],
            Query::Filter(f, info) => {
                let docs = self.query(*info);
                docs.into_iter()
                    .filter(|d| f.should_keep(self, d))
                    .collect()
            }
            Query::Sequence(queries) => {
                let mut docs = vec![];
                for query in queries {
                    docs.append(&mut self.query(query));
                }
                docs
            }
            Query::WithTextQuery(query, info) => {
                // First resolve the query into a document set.
                // then use that as a filter for the remaining iterator
                // TODO: Do something sensible with limit.
                let doc_ids = self
                    .full_text_index
                    .lock()
                    .unwrap()
                    .search(&query, 100)
                    .unwrap();
                let doc_ids: HashSet<_> = doc_ids.into_iter().map(|(_, id)| id).collect();
                let other_doc_ids = self.query(*info);
                other_doc_ids
                    .into_iter()
                    .filter(|d: &DocumentId| doc_ids.contains(d))
                    .collect()
            }
            Query::WithVectorQuery(key, info) => {
                // First resolve the query into a document set.
                // then use that as a filter for the remaining iterator
                // TODO: Do something sensible with limit.
                let doc_ids = self
                    .vector_index
                    .lock()
                    .unwrap()
                    .search(&key, Some(100), Some(0.1))
                    .unwrap();
                let doc_ids: HashSet<_> = doc_ids.into_iter().map(|(_, id)| id).collect();
                let other_doc_ids = self.query(*info);
                other_doc_ids
                    .into_iter()
                    .filter(|d: &DocumentId| doc_ids.contains(d))
                    .collect()
            }
        }
    }

    pub(crate) fn get_metadata(&self, v: &DocumentId, key: &str) -> Option<String> {
        self.metadata_store.lock().unwrap().get_metadata(v, key)
    }

    fn add_document(&mut self, doc_info: DocumentInfo) -> anyhow::Result<DocumentId> {
        let doc_id = self
            .kb
            .lock()
            .unwrap()
            .new_document()
            .map_err(|e| anyhow::anyhow!("Unable to create document"))?;
        for tag in doc_info.tags {
            self.kb
                .lock()
                .unwrap()
                .add_tag(&doc_id, &tag)
                .map_err(|e| anyhow::anyhow!("Unable to add tag"))?;
        }

        // Store the metadata
        self.metadata_store
            .lock()
            .unwrap()
            .set_all_metadata(&doc_id, &doc_info.metadata)
            .map_err(|e| anyhow::anyhow!("Error storing metadata"))?;

        // Store the content
        if let Some(content) = doc_info.content {
            let content_hash = self.content_store.lock().unwrap().store(&content)?;
            self.kb
                .lock()
                .unwrap()
                .set_content(&doc_id, &content_hash)
                .map_err(|e| anyhow::anyhow!("Unable to set content"))?;

            // Optionally index or persist metadata here
            if let Ok(content) = String::from_utf8(content) {
                self.full_text_index
                    .lock()
                    .unwrap()
                    .index_document(&doc_id, &content)?;
            }
        }

        // Store the vector keys
        if !doc_info.vector_keys.is_empty() {
            self.vector_index
                .lock()
                .unwrap()
                .add_multiple_raw_for_doc(&doc_id, doc_info.vector_keys.clone())
                .map_err(|e| anyhow::anyhow!("Unable to add vector keys"))?;
        }

        // Store the graph connections
        if !doc_info.connections.is_empty() {
            self.graph_index
                .lock()
                .unwrap()
                .add_connections(&doc_id, &doc_info.connections)?;
        }

        Ok(doc_id)
    }

    fn get_document(&self, doc_id: &DocumentId) -> anyhow::Result<DocumentInfo> {
        // First check its a document
        let Some(document_base) = self
            .kb
            .lock()
            .unwrap()
            .get_doc_base(doc_id)
            .map_err(|e| anyhow::anyhow!("Error loading document base:{:?}", e))?
        else {
            return Err(anyhow::anyhow!("Invalid document id {}", doc_id));
        };

        // Get the tags
        let tags = document_base.tags;

        // Get the metadata
        let metadata = self.metadata_store.lock().unwrap().get_all_metadata(doc_id);

        // Get the content
        let content = if let Some(ch) = document_base.content_hash {
            self.content_store.lock().unwrap().retrieve(ch)?
        } else {
            None
        };

        // Get the vector keys
        let vector_keys = self
            .vector_index
            .lock()
            .unwrap()
            .keys_for_doc_id(doc_id)
            .map_err(|e| anyhow::anyhow!("Error getting vector keys: {:?}", e))?;

        // Get the connections
        let connections = self
            .graph_index
            .lock()
            .unwrap()
            .connections_for_doc_id(doc_id)?;

        Ok(DocumentInfo {
            metadata,
            tags,
            content,
            vector_keys,
            connections,
        })
    }
}

#[derive(Debug)]
pub enum KnowledgeBaseError {
    Unknown,
    DocumentAlreadyExists,
    DocumentNotFound,
}

#[cfg(test)]
pub mod test {

    use std::collections::BTreeMap;

    use mockall::predicate;

    use crate::{
        content_store::{mocks::MockContentStore, InMemoryContentStore},
        kb::{
            filter::Filter,
            full_text_index::{self, TantivyFullTextIndex},
            graph_index::{Connection, InMemoryGraphIndex},
            kb_impl,
            memorykb::InMemoryKnowledgeBaseImpl,
            metadata_store::{self, InMemoryMetadataStore},
            mock::internal::{
                MockFullTextIndex, MockGraphIndex, MockKnowledgeBaseImpl, MockMetadataStore,
                MockVectorIndex,
            },
            vector_index::{ChunkMetadata, InMemoryVectorIndex},
        },
    };

    use super::*;

    #[test]
    pub fn basic_stream() {
        let (kb, components) = create_test_kb();

        components
            .kb_impl
            .lock()
            .unwrap()
            .expect_all_docs()
            .returning(move || {
                vec![
                    DocumentId::from_u64s(100, 200),
                    DocumentId::from_u64s(100, 201),
                    DocumentId::from_u64s(100, 202),
                    DocumentId::from_u64s(100, 203),
                ]
            });

        let query = Query::AllDocs;
        let docs = kb.query(query);

        assert_eq!(docs.len(), 4);
    }

    #[test]
    pub fn document_stream_filter() {
        let (kb, components) = create_test_kb();

        components
            .kb_impl
            .lock()
            .unwrap()
            .expect_all_docs()
            .returning(move || {
                vec![
                    DocumentId::from_u64s(100, 200),
                    DocumentId::from_u64s(100, 201),
                ]
            });

        let query = Query::AllDocs.filter(Filter::FilterFn(Arc::new(|_| false)));

        let docs: Vec<DocumentId> = kb.query(query);

        assert!(docs.is_empty());

        let query = Query::AllDocs.filter(Filter::FilterFn(Arc::new(|_| true)));

        let docs: Vec<DocumentId> = kb.query(query);
        assert_eq!(docs.len(), 2);
    }

    #[test]
    pub fn sequence_streams() {
        let (kb, components) = create_test_kb();

        components
            .kb_impl
            .lock()
            .unwrap()
            .expect_all_docs()
            .returning(move || vec![DocumentId::from_u64s(234, 567)]);

        let stream1 = Query::AllDocs;
        let stream2 = Query::AllDocs;
        let combined = Query::sequence(&[stream1, stream2]);

        let docs: Vec<DocumentId> = kb.query(combined);
        assert_eq!(docs.len(), 2);
    }

    pub struct InMemoryComponents {
        pub content_store: Arc<Mutex<InMemoryContentStore>>,
        pub full_text_index: Arc<Mutex<TantivyFullTextIndex>>,
        pub vector_index: Arc<Mutex<InMemoryVectorIndex>>,
        pub kb_impl: Arc<Mutex<InMemoryKnowledgeBaseImpl>>,
        pub metadata_store: Arc<Mutex<InMemoryMetadataStore>>,
        pub graph_index: Arc<Mutex<InMemoryGraphIndex>>,
    }

    impl InMemoryComponents {
        pub fn new() -> InMemoryComponents {
            let content_store = Arc::new(Mutex::new(InMemoryContentStore::default()));
            let full_text_index =
                Arc::new(Mutex::new(TantivyFullTextIndex::new_in_memory().unwrap()));

            let vector_index = Arc::new(Mutex::new(InMemoryVectorIndex::default()));
            let kb_impl = Arc::new(Mutex::new(InMemoryKnowledgeBaseImpl::default()));
            let metadata_store = Arc::new(Mutex::new(InMemoryMetadataStore::default()));
            let graph_index = Arc::new(Mutex::new(InMemoryGraphIndex::default()));

            InMemoryComponents {
                content_store,
                full_text_index,
                vector_index,
                kb_impl,
                metadata_store,
                graph_index,
            }
        }

        pub fn build_kb(&self) -> KnowledgeBase {
            KnowledgeBase {
                kb: self.kb_impl.clone(),
                content_store: self.content_store.clone(),
                full_text_index: self.full_text_index.clone(),
                vector_index: self.vector_index.clone(),
                metadata_store: self.metadata_store.clone(),
                graph_index: self.graph_index.clone(),
            }
        }
    }

    pub fn create_in_memory_kb() -> (KnowledgeBase, InMemoryComponents) {
        let components = InMemoryComponents::new();
        (components.build_kb(), components)
    }

    pub struct TestComponents {
        pub content_store: Arc<Mutex<MockContentStore>>,
        pub full_text_index: Arc<Mutex<MockFullTextIndex>>,
        pub vector_index: Arc<Mutex<MockVectorIndex>>,
        pub kb_impl: Arc<Mutex<MockKnowledgeBaseImpl>>,
        pub metadata_store: Arc<Mutex<MockMetadataStore>>,
        pub graph_index: Arc<Mutex<MockGraphIndex>>,
    }

    impl TestComponents {
        pub fn new() -> TestComponents {
            let content_store = Arc::new(Mutex::new(MockContentStore::new()));
            let full_text_index = Arc::new(Mutex::new(MockFullTextIndex::default()));
            let vector_index = Arc::new(Mutex::new(MockVectorIndex::default()));
            let kb_impl = Arc::new(Mutex::new(MockKnowledgeBaseImpl::new()));
            let metadata_store = Arc::new(Mutex::new(MockMetadataStore::new()));
            let graph_index = Arc::new(Mutex::new(MockGraphIndex::new()));

            TestComponents {
                content_store,
                full_text_index,
                vector_index,
                kb_impl,
                metadata_store,
                graph_index,
            }
        }

        pub fn build_kb(&self) -> KnowledgeBase {
            KnowledgeBase {
                kb: self.kb_impl.clone(),
                content_store: self.content_store.clone(),
                full_text_index: self.full_text_index.clone(),
                vector_index: self.vector_index.clone(),
                metadata_store: self.metadata_store.clone(),
                graph_index: self.graph_index.clone(),
            }
        }

        pub fn set_text_search_results(
            &mut self,
            key: &str,
            score_and_ids: Vec<(f32, DocumentId)>,
        ) {
            self.full_text_index
                .lock()
                .unwrap()
                .expect_search()
                .once()
                .with(predicate::eq(key.to_string()), predicate::eq(100))
                .returning(move |_, _| Ok(score_and_ids.clone()));
        }

        pub fn set_vector_search_results(
            &mut self,
            key: Vec<f32>,
            score_and_ids: Vec<(f32, DocumentId)>,
        ) {
            self.vector_index
                .lock()
                .unwrap()
                .expect_search()
                .once()
                .with(predicate::eq(key), predicate::always(), predicate::always())
                .returning(move |_, _, _| Ok(score_and_ids.clone()));
        }

        pub fn set_all_doc_ids(&mut self, all_doc_ids: &[DocumentId]) {
            let documents: Vec<DocumentId> = all_doc_ids.to_vec();
            self.kb_impl
                .lock()
                .unwrap()
                .expect_all_docs()
                .once()
                .returning(move || documents.clone());
        }

        pub fn set_document_metadata(&mut self, doc_id: &DocumentId, kvs: &[(&str, &str)]) {
            for (k, v) in kvs {
                let v = v.to_string();
                self.metadata_store
                    .lock()
                    .unwrap()
                    .expect_get_metadata()
                    .with(predicate::eq(doc_id.clone()), predicate::eq(k.to_string()))
                    .returning(move |_, _| Some(v.clone()));
            }
        }
    }

    pub fn create_test_kb() -> (KnowledgeBase, TestComponents) {
        let components = TestComponents::new();
        (components.build_kb(), components)
    }

    #[test]
    pub fn extract_by_full_text_search() {
        // Here we have a kb containing 3 docs,(id1, id3, id4)
        // and a full-text-index where 3 docs are associated with the query "tofu" (id1, id2, id3)
        // If we search the kb for "tofu" we should just get (id1, id3)

        let (kb, mut components) = create_test_kb();

        let id1 = DocumentId::from_u64s(123, 1);
        let id2 = DocumentId::from_u64s(123, 2);
        let id3 = DocumentId::from_u64s(123, 3);
        let id4 = DocumentId::from_u64s(123, 4);

        let docs_for_tofu = vec![
            (1.0, (&id1).clone()),
            (0.7, id2.clone()),
            (0.5, id3.clone()),
        ];

        components.set_text_search_results("tofu", docs_for_tofu);
        components.set_all_doc_ids(&[id1.clone(), id3.clone(), id4.clone()]);

        let docs = kb.query(Query::AllDocs.with_text_query("tofu"));

        assert_eq!(docs.len(), 2);
        assert_eq!(docs, vec![id1.clone(), id3.clone()]);
    }

    #[test]
    pub fn extract_by_vector_search() {
        // Here we have a kb containing 3 docs,(id1, id3, id4)
        // and a vector-index where 3 docs are associated with the query [1.0, 2.0] (id1, id2, id3)
        // If we search the kb for [1.0, 2.0] we should just get (id1, id3)

        let (kb, mut components) = create_test_kb();

        let id1 = DocumentId::from_u64s(123, 1);
        let id2 = DocumentId::from_u64s(123, 2);
        let id3 = DocumentId::from_u64s(123, 3);
        let id4 = DocumentId::from_u64s(123, 4);

        let key = vec![1.0, 2.0];
        let docs_for_key = vec![
            (1.0, (&id1).clone()),
            (0.7, id2.clone()),
            (0.5, id3.clone()),
        ];

        components.set_vector_search_results(key.clone(), docs_for_key);
        components.set_all_doc_ids(&[id1.clone(), id3.clone(), id4.clone()]);

        let docs = kb.query(Query::AllDocs.with_vector_query(key.clone()));

        assert_eq!(docs.len(), 2);
        assert_eq!(docs, vec![id1.clone(), id3.clone()]);
    }

    #[test]
    pub fn can_add_and_retrieve_a_document() {
        let (mut kb, mut _components) = create_in_memory_kb();
        let doc_info = DocumentInfo {
            metadata: build_metadata(&[
                ("some_key", "some_value"),
                ("another_key", "a_different_value"),
            ]),
            tags: build_tags(&["a_tag", "another_tag"]),
            content: build_content("This is some interesting content"),
            vector_keys: build_vector_keys(&[&[1.0, 2.0, 3.0], &[3.0, 2.0, 1.0]]),
            connections: build_connections(&[
                (DocumentId::from_u64s(123, 456), "some connection", true), // Is not reverse direction.
            ]),
        };
        let doc_id = kb.add_document(doc_info.clone()).unwrap();
        let copy = kb.get_document(&doc_id).unwrap();
        assert_eq!(doc_info, copy);
    }

    fn build_metadata(entries: &[(&str, &str)]) -> BTreeMap<String, String> {
        entries
            .iter()
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .collect()
    }

    fn build_tags(tags: &[&str]) -> Vec<String> {
        tags.iter().map(|a| a.to_string()).collect()
    }

    fn build_content(content: &str) -> Option<Vec<u8>> {
        Some(content.as_bytes().to_vec())
    }

    fn build_vector_key(vk: &[f32]) -> (Vec<f32>, ChunkMetadata) {
        (vk.to_vec(), ChunkMetadata{offset: 0,length: 0})
    }

    fn build_vector_keys(vks: &[&[f32]]) -> Vec<(Vec<f32>, ChunkMetadata)> {
        vks.iter().map(|k| build_vector_key(k)).collect()
    }

    fn build_connections(connections: &[(DocumentId, &str, bool); 1]) -> Vec<Connection> {
        connections.iter().map(|c| Connection {}).collect()
    }
}
