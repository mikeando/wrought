use core::str;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crate::{binary16::ContentHash, content_store::ContentStore};

use super::{
    docstream::{
        DocumentStream, DocumentStreamInfo, DocumentStreamIterator, DocumentStreamIteratorDelayed,
    },
    document::DocumentId,
    kb_impl::KnowledgeBaseImpl,
};
use super::{document::Document, fulltextindex::FullTextIndex};

#[derive(Clone)]
pub struct KnowledgeBase {
    pub kb: Arc<Mutex<dyn KnowledgeBaseImpl>>,
    pub content_store: Arc<Mutex<dyn ContentStore + Send + Sync>>,
    // Or does this belong inside the kb itself?
    pub full_text_index: Arc<Mutex<dyn FullTextIndex + Send + Sync>>,
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

    use crate::{
        content_store::mocks::MockContentStore,
        kb::{
            docstream::DocumentStreamFilter,
            mock::internal::{MockFullTextIndex, MockKnowledgeBaseImpl},
        },
    };

    use super::*;

    fn dummy_doc(content_store: Arc<Mutex<dyn ContentStore + Send + Sync>>) -> Document {
        Document::new(
            DocumentId {
                id: ContentHash::zero(),
            },
            HashMap::new(),
            content_store,
        )
    }

    #[test]
    pub fn basic_stream() {
        let cs = Arc::new(Mutex::new(MockContentStore::new()));
        let cs2 = cs.clone();
        let full_text_index = Arc::new(Mutex::new(MockFullTextIndex::default()));
        let kb_impl = Arc::new(Mutex::new(MockKnowledgeBaseImpl::new()));
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
        kb_impl
            .lock()
            .unwrap()
            .expect_all_docs()
            .returning(move || vec![dummy_doc(cs.clone()), dummy_doc(cs.clone())]);

        let kb = KnowledgeBase {
            kb: kb_impl,
            content_store,
            full_text_index,
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
        kb_impl
            .lock()
            .unwrap()
            .expect_all_docs()
            .returning(move || vec![dummy_doc(cs.clone())]);
        let kb = KnowledgeBase {
            kb: kb_impl,
            content_store,
            full_text_index,
        };

        let stream1 = kb.all_docs();
        let stream2 = kb.all_docs();
        let combined = DocumentStream::sequence(kb, &[stream1, stream2]);

        let docs: Vec<Document> = combined.query();
        assert_eq!(docs.len(), 2);
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
