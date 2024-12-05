use core::str;
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
};

use crate::{binary16::ContentHash, content_store::ContentStore};

use super::full_text_index::FullTextIndex;
use super::{
    document::DocumentId, kb_impl::KnowledgeBaseImpl, metadata_store::MetadataStore, query::Query,
    vector_index::VectorIndex,
};

#[derive(Clone)]
pub struct KnowledgeBase {
    pub kb: Arc<Mutex<dyn KnowledgeBaseImpl>>,

    pub content_store: Arc<Mutex<dyn ContentStore + Send + Sync>>,
    pub full_text_index: Arc<Mutex<dyn FullTextIndex + Send + Sync>>,
    pub vector_index: Arc<Mutex<dyn VectorIndex + Send + Sync>>,
    pub metadata_store: Arc<Mutex<dyn MetadataStore + Send + Sync>>,
}

impl KnowledgeBase {
    pub fn store_document(
        &mut self,
        doc_id: DocumentId,
        content: &[u8],
        metadata: HashMap<String, String>,
    ) -> anyhow::Result<DocumentId> {
        let content = str::from_utf8(content)?;

        // Optionally index or persist metadata here
        self.full_text_index
            .lock()
            .unwrap()
            .index_document(&doc_id, content)?;
        Ok(doc_id)
    }

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
}

#[derive(Debug)]
pub enum KnowledgeBaseError {
    Unknown,
    DocumentAlreadyExists,
    DocumentNotFound,
}

#[cfg(test)]
pub mod test {

    use mockall::predicate;

    use crate::{
        content_store::mocks::MockContentStore,
        kb::{
            filter::Filter,
            mock::internal::{
                MockFullTextIndex, MockKnowledgeBaseImpl, MockMetadataStore, MockVectorIndex,
            },
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

    pub struct TestComponents {
        pub content_store: Arc<Mutex<MockContentStore>>,
        pub full_text_index: Arc<Mutex<MockFullTextIndex>>,
        pub vector_index: Arc<Mutex<MockVectorIndex>>,
        pub kb_impl: Arc<Mutex<MockKnowledgeBaseImpl>>,
        pub metadata_store: Arc<Mutex<MockMetadataStore>>,
    }

    impl TestComponents {
        pub fn new() -> TestComponents {
            let content_store = Arc::new(Mutex::new(MockContentStore::new()));
            let full_text_index = Arc::new(Mutex::new(MockFullTextIndex::default()));
            let vector_index = Arc::new(Mutex::new(MockVectorIndex::default()));
            let kb_impl = Arc::new(Mutex::new(MockKnowledgeBaseImpl::new()));
            let metadata_store = Arc::new(Mutex::new(MockMetadataStore::new()));

            TestComponents {
                content_store,
                full_text_index,
                vector_index,
                kb_impl,
                metadata_store,
            }
        }

        pub fn build_kb(&self) -> KnowledgeBase {
            KnowledgeBase {
                kb: self.kb_impl.clone(),
                content_store: self.content_store.clone(),
                full_text_index: self.full_text_index.clone(),
                vector_index: self.vector_index.clone(),
                metadata_store: self.metadata_store.clone(),
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
        todo!();
    }
}
