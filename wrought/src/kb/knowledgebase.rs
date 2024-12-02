use core::str;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crate::{binary16::ContentHash, content_store::ContentStore};

use super::{
    docstream::{
        DocumentStream, DocumentStreamFilter, DocumentStreamInfo, DocumentStreamIterator,
        DocumentStreamIteratorDelayed,
    },
    document::DocumentId,
    kb_impl::KnowledgeBaseImpl,
    vector_index::VectorIndex,
};
use super::{document::Document, full_text_index::FullTextIndex};

#[derive(Clone)]
pub struct KnowledgeBase {
    pub kb: Arc<Mutex<dyn KnowledgeBaseImpl>>,
    pub content_store: Arc<Mutex<dyn ContentStore + Send + Sync>>,
    // Or does this belong inside the kb itself?
    pub full_text_index: Arc<Mutex<dyn FullTextIndex + Send + Sync>>,
    pub vector_index: Arc<Mutex<dyn VectorIndex + Send + Sync>>,
}

impl KnowledgeBase {
    pub fn store_document(
        &mut self,
        content: &[u8],
        metadata: HashMap<String, String>,
    ) -> anyhow::Result<Document> {
        let hash = self.content_store.lock().unwrap().store(content)?;
        let doc_id = DocumentId { id: hash };
        let document = Document::new(doc_id.clone(), metadata, self.content_store.clone());
        let content = str::from_utf8(content)?;

        // Optionally index or persist metadata here
        self.full_text_index
            .lock()
            .unwrap()
            .index_document(&doc_id, content)?;
        Ok(document)
    }

    pub fn get_content(&self, hash: &ContentHash) -> anyhow::Result<Option<Vec<u8>>> {
        self.content_store.lock().unwrap().retrieve(hash.clone())
    }

    pub fn create_iter(&self, info: &DocumentStreamInfo) -> DocumentStreamIterator {
        match info {
            DocumentStreamInfo::AllDocs => {
                DocumentStreamIterator::list(self.kb.lock().unwrap().all_docs())
            }
            DocumentStreamInfo::EmptyStream => DocumentStreamIterator::empty(),
            DocumentStreamInfo::Filter(f, info) => {
                DocumentStreamIterator::filter(f.clone(), self.create_iter(info))
            }
            DocumentStreamInfo::Sequence(infos) => DocumentStreamIterator::sequence(
                infos
                    .iter()
                    .map(|info| {
                        DocumentStreamIterator::Delayed(DocumentStreamIteratorDelayed::Unevaluated(
                            self.clone(),
                            info.clone(),
                        ))
                    })
                    .collect(),
            ),
            DocumentStreamInfo::WithTextQuery(query, info) => {
                // First resolve the query into a document set.
                // then use that as a filter for the remaining iterator
                // TODO: Do something sensible with limit.
                let doc_ids = self
                    .full_text_index
                    .lock()
                    .unwrap()
                    .search(&query, 100)
                    .unwrap();
                let doc_ids: Vec<_> = doc_ids.into_iter().map(|(_, id)| id).collect();
                DocumentStreamIterator::filter(
                    DocumentStreamFilter::id_in_set(doc_ids),
                    self.create_iter(info),
                )
            }
            DocumentStreamInfo::WithVectorQuery(key, info) => {
                // First resolve the query into a document set.
                // then use that as a filter for the remaining iterator
                // TODO: Do something sensible with limit.
                let doc_ids = self
                    .vector_index
                    .lock()
                    .unwrap()
                    .search(&key, Some(100), Some(0.1))
                    .unwrap();
                let doc_ids: Vec<_> = doc_ids.into_iter().map(|(_, id)| id).collect();
                DocumentStreamIterator::filter(
                    DocumentStreamFilter::id_in_set(doc_ids),
                    self.create_iter(info),
                )
            }
            
        }
    }

    pub fn all_docs(&self) -> DocumentStream {
        DocumentStream {
            kb: self.clone(),
            info: DocumentStreamInfo::AllDocs,
        }
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
            docstream::DocumentStreamFilter,
            mock::internal::{MockFullTextIndex, MockKnowledgeBaseImpl, MockVectorIndex},
            vector_index,
        },
    };

    use super::*;

    fn dummy_doc(content_store: Arc<Mutex<dyn ContentStore + Send + Sync>>) -> Document {
        doc(
            &DocumentId {
                id: ContentHash::zero(),
            },
            content_store,
        )
    }

    fn doc(id: &DocumentId, content_store: Arc<Mutex<dyn ContentStore + Send + Sync>>) -> Document {
        Document::new(id.clone(), HashMap::new(), content_store)
    }

    fn doc_id(content: &str) -> DocumentId {
        DocumentId {
            id: ContentHash::from_content(content.as_bytes()),
        }
    }

    #[test]
    pub fn basic_stream() {
        let cs = Arc::new(Mutex::new(MockContentStore::new()));
        let cs2 = cs.clone();
        let full_text_index = Arc::new(Mutex::new(MockFullTextIndex::default()));
        let kb_impl = Arc::new(Mutex::new(MockKnowledgeBaseImpl::new()));
        let vector_index = Arc::new(Mutex::new(MockVectorIndex::default()));
        kb_impl
            .lock()
            .unwrap()
            .expect_all_docs()
            .returning(move || {
                vec![
                    dummy_doc(cs.clone()),
                    dummy_doc(cs.clone()),
                    dummy_doc(cs.clone()),
                    dummy_doc(cs.clone()),
                ]
            });
        let kb = KnowledgeBase {
            kb: kb_impl,
            content_store: cs2,
            full_text_index,
            vector_index,
        };

        let docs = kb.all_docs().query();

        assert_eq!(docs.len(), 4);
    }

    #[test]
    pub fn document_retrieves_content() {
        let mut cs = MockContentStore::new();
        let content = "Hello, world!";
        let docid = DocumentId {
            id: ContentHash::from_content("some content".as_bytes()),
        };

        cs.expect_retrieve()
            .with(mockall::predicate::eq(docid.id.clone()))
            .returning(move |_| Ok(Some(content.as_bytes().to_vec())));

        let document = Document::new(docid.clone(), HashMap::new(), Arc::new(Mutex::new(cs)));

        let retrieved_content = document.content().unwrap();
        assert_eq!(retrieved_content, Some(content.as_bytes().to_vec()));
    }

    #[test]
    pub fn document_stream_filter() {
        let content_store = Arc::new(Mutex::new(MockContentStore::new()));
        let full_text_index = Arc::new(Mutex::new(MockFullTextIndex::default()));
        let cs = content_store.clone();
        let kb_impl = Arc::new(Mutex::new(MockKnowledgeBaseImpl::new()));
        let vector_index = Arc::new(Mutex::new(MockVectorIndex::default()));

        kb_impl
            .lock()
            .unwrap()
            .expect_all_docs()
            .returning(move || vec![dummy_doc(cs.clone()), dummy_doc(cs.clone())]);

        let kb = KnowledgeBase {
            kb: kb_impl,
            content_store,
            full_text_index,
            vector_index,
        };

        let stream = kb
            .all_docs()
            .filter(DocumentStreamFilter::FilterFn(Arc::new(|_| false)));
        let docs: Vec<Document> = stream.query();

        assert!(docs.is_empty());

        let stream = kb
            .all_docs()
            .filter(DocumentStreamFilter::FilterFn(Arc::new(|_| true)));
        let docs: Vec<Document> = stream.query();
        assert_eq!(docs.len(), 2);
    }

    #[test]
    pub fn missing_content_in_store() {
        let mut cs = MockContentStore::new();
        let docid = DocumentId {
            id: ContentHash::from_content("some content".as_bytes()),
        };

        cs.expect_retrieve()
            .with(mockall::predicate::eq(docid.id.clone()))
            .returning(|_| Ok(None));

        let document = Document::new(docid.clone(), HashMap::new(), Arc::new(Mutex::new(cs)));

        let retrieved_content = document.content().unwrap();
        assert!(retrieved_content.is_none());
    }

    #[test]
    pub fn sequence_streams() {
        let content_store = Arc::new(Mutex::new(MockContentStore::new()));
        let cs = content_store.clone();
        let full_text_index = Arc::new(Mutex::new(MockFullTextIndex::default()));
        let kb_impl = Arc::new(Mutex::new(MockKnowledgeBaseImpl::new()));
        let vector_index = Arc::new(Mutex::new(MockVectorIndex::default()));

        kb_impl
            .lock()
            .unwrap()
            .expect_all_docs()
            .returning(move || vec![dummy_doc(cs.clone())]);
        let kb = KnowledgeBase {
            kb: kb_impl,
            content_store,
            full_text_index,
            vector_index,
        };

        let stream1 = kb.all_docs();
        let stream2 = kb.all_docs();
        let combined = DocumentStream::sequence(kb, &[stream1, stream2]);

        let docs: Vec<Document> = combined.query();
        assert_eq!(docs.len(), 2);
    }

    struct TestComponents {
        content_store: Arc<Mutex<MockContentStore>>,
        full_text_index: Arc<Mutex<MockFullTextIndex>>,
        vector_index: Arc<Mutex<MockVectorIndex>>,
        kb_impl: Arc<Mutex<MockKnowledgeBaseImpl>>,
    }

    impl TestComponents {
        pub fn new() -> TestComponents {
            let content_store = Arc::new(Mutex::new(MockContentStore::new()));
            let full_text_index = Arc::new(Mutex::new(MockFullTextIndex::default()));
            let vector_index = Arc::new(Mutex::new(MockVectorIndex::default()));
            let kb_impl = Arc::new(Mutex::new(MockKnowledgeBaseImpl::new()));

            TestComponents {
                content_store,
                full_text_index,
                vector_index,
                kb_impl,
            }
        }

        pub fn build_kb(&self) -> KnowledgeBase {
            KnowledgeBase {
                kb: self.kb_impl.clone(),
                content_store: self.content_store.clone(),
                full_text_index: self.full_text_index.clone(),
                vector_index: self.vector_index.clone(),
            }
        }

        fn set_text_search_results(&mut self, key: &str, score_and_ids: Vec<(f32, DocumentId)>) {
            self.full_text_index
                .lock()
                .unwrap()
                .expect_search()
                .once()
                .with(predicate::eq(key.to_string()), predicate::eq(100))
                .returning(move |_, _| Ok(score_and_ids.clone()));
        }

        fn set_vector_search_results(&mut self, key: Vec<f32>, score_and_ids: Vec<(f32, DocumentId)>) {
            self.vector_index
                .lock()
                .unwrap()
                .expect_search()
                .once()
                .with(predicate::eq(key), predicate::always(), predicate::always())
                .returning(move |_,_,_| Ok(score_and_ids.clone()));
        }

        fn set_all_doc_ids(&mut self, all_doc_ids: &[DocumentId]) {
            let documents: Vec<_> = all_doc_ids
                .iter()
                .map(|d| doc(d, self.content_store.clone()))
                .collect();
            self.kb_impl
                .lock()
                .unwrap()
                .expect_all_docs()
                .once()
                .returning(move || documents.clone());
        }
    }

    fn create_test_kb() -> (KnowledgeBase, TestComponents) {
        let components = TestComponents::new();
        (components.build_kb(), components)
    }

    #[test]
    pub fn extract_by_full_text_search() {
        // Here we have a kb containing 3 docs,(id1, id3, id4)
        // and a full-text-index where 3 docs are associated with the query "tofu" (id1, id2, id3)
        // If we search the kb for "tofu" we should just get (id1, id3)

        let (kb, mut components) = create_test_kb();

        let id1 = doc_id("Some doc about tofu");
        let id2 = doc_id("Another doc about tofu");
        let id3 = doc_id("I love docs about tofu");
        let id4 = doc_id("A doc about beans");

        let docs_for_tofu = vec![
            (1.0, (&id1).clone()),
            (0.7, id2.clone()),
            (0.5, id3.clone()),
        ];

        components.set_text_search_results("tofu", docs_for_tofu);
        components.set_all_doc_ids(&[id1.clone(), id3.clone(), id4.clone()]);

        let docs = kb.all_docs().with_text_query("tofu").query();

        assert_eq!(docs.len(), 2);
        assert_eq!(
            docs.into_iter().map(|d| d.id().clone()).collect::<Vec<_>>(),
            vec![id1.clone(), id3.clone()]
        );
    }

    #[test]
    pub fn extract_by_vector_search() {
        // Here we have a kb containing 3 docs,(id1, id3, id4)
        // and a vector-index where 3 docs are associated with the query [1.0, 2.0] (id1, id2, id3)
        // If we search the kb for [1.0, 2.0] we should just get (id1, id3)

        let (kb, mut components) = create_test_kb();

        let id1 = doc_id("Some doc about tofu");
        let id2 = doc_id("Another doc about tofu");
        let id3 = doc_id("I love docs about tofu");
        let id4 = doc_id("A doc about beans");

        let key = vec![1.0, 2.0];
        let docs_for_key = vec![
            (1.0, (&id1).clone()),
            (0.7, id2.clone()),
            (0.5, id3.clone()),
        ];

        components.set_vector_search_results(key.clone(), docs_for_key);
        components.set_all_doc_ids(&[id1.clone(), id3.clone(), id4.clone()]);

        let docs = kb.all_docs().with_vector_query(key.clone()).query();

        assert_eq!(docs.len(), 2);
        assert_eq!(
            docs.into_iter().map(|d| d.id().clone()).collect::<Vec<_>>(),
            vec![id1.clone(), id3.clone()]
        );
    }

    #[test]
    pub fn document_metadata() {
        let cs = Arc::new(Mutex::new(MockContentStore::new()));
        let mut metadata = HashMap::new();
        metadata.insert("author".to_string(), "Alice".to_string());

        let docid = DocumentId {
            id: ContentHash::from_content("some content".as_bytes()),
        };
        let mut document = Document::new(docid, metadata.clone(), cs);

        assert_eq!(document.get_metadata("author"), Some(&"Alice".to_string()));

        document.set_metadata("author".to_string(), "Bob".to_string());
        assert_eq!(document.get_metadata("author"), Some(&"Bob".to_string()));
    }

    #[test]
    pub fn content_store_error_propagation() {
        let cs = Arc::new(Mutex::new(MockContentStore::new()));
        let docid = DocumentId {
            id: ContentHash::from_content("some hash".as_bytes()),
        };

        cs.lock()
            .unwrap()
            .expect_retrieve()
            .with(mockall::predicate::eq(docid.id.clone()))
            .returning(|_| Err(anyhow::anyhow!("Store error")));

        let document = Document::new(docid.clone(), HashMap::new(), cs);

        let err = document.content().unwrap_err();
        assert_eq!(err.to_string(), "Store error");
    }
}
