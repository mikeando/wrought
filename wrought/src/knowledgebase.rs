use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex},
};

use crate::{binary16::ContentHash, content_store::ContentStore};

#[derive(Clone)]
pub struct KnowledgeBase {
    kb: Arc<Mutex<dyn KnowledgeBaseImpl>>,
    content_store: Arc<Mutex<dyn ContentStore + Send + Sync>>,
}

impl KnowledgeBase {
    pub fn store_document(
        &mut self,
        content: &[u8],
        metadata: HashMap<String, String>,
    ) -> anyhow::Result<Document> {
        let hash = self.content_store.lock().unwrap().store(content)?;
        let document = Document::new(hash, metadata, self.content_store.clone());
        // Optionally index or persist metadata here
        Ok(document)
    }

    pub fn get_content(&self, hash: &ContentHash) -> anyhow::Result<Option<Vec<u8>>> {
        self.content_store.lock().unwrap().retrieve(hash.clone())
    }
}

pub trait KnowledgeBaseImpl {
    fn create_iter(&self, kb: &KnowledgeBase, info: &DocumentStreamInfo) -> DocumentStreamIterator;
}

pub struct DocumentStream {
    kb: KnowledgeBase,
    info: DocumentStreamInfo,
}

#[derive(Clone)]
pub enum DocumentStreamInfo {
    AllDocs,
    EmptyStream,
    Filter(DocumentStreamFilter, Box<DocumentStreamInfo>),
    Sequence(Vec<DocumentStreamInfo>),
}

impl KnowledgeBase {
    pub fn all_docs(&self) -> DocumentStream {
        DocumentStream {
            kb: self.clone(),
            info: DocumentStreamInfo::AllDocs,
        }
    }
}

impl DocumentStream {
    pub fn filter(&self, f: DocumentStreamFilter) -> DocumentStream {
        let info = match &self.info {
            DocumentStreamInfo::AllDocs => {
                DocumentStreamInfo::Filter(f, Box::new(DocumentStreamInfo::AllDocs))
            }
            DocumentStreamInfo::EmptyStream => DocumentStreamInfo::EmptyStream,
            DocumentStreamInfo::Filter(f2, info) => {
                DocumentStreamInfo::Filter(DocumentStreamFilter::and(f2.clone(), f), info.clone())
            }
            DocumentStreamInfo::Sequence(infos) => {
                // f(seq(a,b,c)) = seq(f(a), f(b), f(c))
                let infos: Vec<_> = infos.iter().filter(|v| f.should_keep(v)).cloned().collect();
                if infos.is_empty() {
                    DocumentStreamInfo::EmptyStream
                } else {
                    DocumentStreamInfo::Sequence(infos)
                }
            }
        };
        DocumentStream {
            kb: self.kb.clone(),
            info,
        }
    }

    pub fn sequence(kb: KnowledgeBase, streams: &[DocumentStream]) -> DocumentStream {
        let mut infos = vec![];
        for s in streams {
            // TODO: Should we report an error instead?
            assert!(Arc::ptr_eq(&kb.kb, &s.kb.kb));

            match s.info {
                DocumentStreamInfo::EmptyStream => {}
                _ => infos.push(s.info.clone()),
            }
        }

        if infos.is_empty() {
            return DocumentStream {
                kb,
                info: DocumentStreamInfo::EmptyStream,
            };
        }
        if infos.len() == 1 {
            return DocumentStream {
                kb,
                info: infos[0].clone(),
            };
        }
        DocumentStream {
            kb,
            info: DocumentStreamInfo::Sequence(infos),
        }
    }

    pub fn query(&self) -> Vec<Document> {
        self.iter().collect()
    }

    pub fn iter(&self) -> DocumentStreamIterator {
        // TODO: Probably just move this into KnowledgeDb
        self.kb.kb.lock().unwrap().create_iter(&self.kb, &self.info)
    }
}

#[derive(Clone)]
pub enum DocumentStreamFilter {
    And(Box<DocumentStreamFilter>, Box<DocumentStreamFilter>),
    FilterFn(Arc<dyn Fn(&DocumentStreamInfo) -> bool>),
}

impl DocumentStreamFilter {
    pub fn and(f1: DocumentStreamFilter, f2: DocumentStreamFilter) -> DocumentStreamFilter {
        DocumentStreamFilter::And(Box::new(f1), Box::new(f2))
    }

    fn should_keep(&self, v: &DocumentStreamInfo) -> bool {
        match self {
            DocumentStreamFilter::And(f1, f2) => f1.should_keep(v) && f2.should_keep(v),
            DocumentStreamFilter::FilterFn(f) => f(v),
        }
    }
}

pub struct Document {
    id: ContentHash,
    metadata: HashMap<String, String>,
    content_store: Arc<Mutex<dyn ContentStore + Send + Sync>>,
}

impl Document {
    /// Create a new document with the given ID and metadata.
    pub fn new(
        id: ContentHash,
        metadata: HashMap<String, String>,
        content_store: Arc<Mutex<dyn ContentStore + Send + Sync>>,
    ) -> Self {
        Document {
            id,
            metadata,
            content_store,
        }
    }

    /// Retrieve the document ID.
    pub fn id(&self) -> &ContentHash {
        &self.id
    }

    pub fn content(&self) -> anyhow::Result<Option<Vec<u8>>> {
        self.content_store.lock().unwrap().retrieve(self.id.clone())
    }

    /// Get metadata by key.
    pub fn get_metadata(&self, key: &str) -> Option<&String> {
        self.metadata.get(key)
    }

    /// Add or update a metadata field.
    pub fn set_metadata(&mut self, key: String, value: String) {
        self.metadata.insert(key, value);
    }

    /// Remove a metadata field.
    pub fn remove_metadata(&mut self, key: &str) -> Option<String> {
        self.metadata.remove(key)
    }
}

pub struct DocumentStreamIteratorList {
    pending: VecDeque<Document>,
}

pub enum DocumentStreamIterator {
    ListOfDocs(DocumentStreamIteratorList),
}

impl DocumentStreamIterator {
    fn list(pending: Vec<Document>) -> DocumentStreamIterator {
        DocumentStreamIterator::ListOfDocs(DocumentStreamIteratorList {
            pending: VecDeque::from(pending),
        })
    }
}

impl Iterator for DocumentStreamIteratorList {
    type Item = Document;

    fn next(&mut self) -> Option<Self::Item> {
        self.pending.pop_front()
    }
}

impl Iterator for DocumentStreamIterator {
    type Item = Document;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            DocumentStreamIterator::ListOfDocs(v) => v.next(),
        }
    }
}

#[cfg(test)]
pub mod test {
    use mockall::mock;

    use crate::content_store::mocks::MockContentStore;

    use super::*;

    mock! {
        pub KnowledgeBaseImpl {}

        impl KnowledgeBaseImpl for KnowledgeBaseImpl {
            fn create_iter(&self, kb: &KnowledgeBase, info: &DocumentStreamInfo) -> DocumentStreamIterator;
        }
    }

    fn dummy_doc(kb: &KnowledgeBase) -> Document {
        Document {
            id: ContentHash::zero(),
            metadata: HashMap::new(),
            content_store: kb.content_store.clone(),
        }
    }

    #[test]
    pub fn basic_stream() {
        let kb_impl = Arc::new(Mutex::new(MockKnowledgeBaseImpl::new()));
        kb_impl
            .lock()
            .unwrap()
            .expect_create_iter()
            .returning(move |kb, doc_stream| {
                DocumentStreamIterator::list(vec![
                    dummy_doc(kb),
                    dummy_doc(kb),
                    dummy_doc(kb),
                    dummy_doc(kb),
                ])
            });
        let cs = Arc::new(Mutex::new(MockContentStore::new()));
        let kb = KnowledgeBase {
            kb: kb_impl,
            content_store: cs,
        };

        let docs = kb.all_docs().query();

        assert_eq!(docs.len(), 4);
    }

    #[test]
    pub fn document_retrieves_content() {
        let mut cs = MockContentStore::new();
        let content = "Hello, world!";
        let hash = ContentHash::from_content("some content".as_bytes());

        cs.expect_retrieve()
            .with(mockall::predicate::eq(hash.clone()))
            .returning(move |_| Ok(Some(content.as_bytes().to_vec())));

        let document = Document::new(hash.clone(), HashMap::new(), Arc::new(Mutex::new(cs)));

        let retrieved_content = document.content().unwrap();
        assert_eq!(retrieved_content, Some(content.as_bytes().to_vec()));
    }

    #[test]
    pub fn document_stream_filter() {
        let kb_impl = Arc::new(Mutex::new(MockKnowledgeBaseImpl::new()));
        kb_impl
            .lock()
            .unwrap()
            .expect_create_iter()
            .returning(move |kb, _| {
                DocumentStreamIterator::list(vec![dummy_doc(kb), dummy_doc(kb)])
            });

        let kb = KnowledgeBase {
            kb: kb_impl,
            content_store: Arc::new(Mutex::new(MockContentStore::new())),
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
        let hash = ContentHash::from_content("some content".as_bytes());

        cs.expect_retrieve()
            .with(mockall::predicate::eq(hash.clone()))
            .returning(|_| Ok(None));

        let document = Document::new(hash.clone(), HashMap::new(), Arc::new(Mutex::new(cs)));

        let retrieved_content = document.content().unwrap();
        assert!(retrieved_content.is_none());
    }

    #[test]
    pub fn sequence_streams() {
        let kb_impl = Arc::new(Mutex::new(MockKnowledgeBaseImpl::new()));
        kb_impl
            .lock()
            .unwrap()
            .expect_create_iter()
            .returning(move |kb, ds| DocumentStreamIterator::list(vec![dummy_doc(&kb)]));
        let content_store = Arc::new(Mutex::new(MockContentStore::new()));
        let kb = KnowledgeBase {
            kb: kb_impl,
            content_store,
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

        let mut document = Document::new(
            ContentHash::from_content("some content".as_bytes()),
            metadata.clone(),
            cs,
        );

        assert_eq!(document.get_metadata("author"), Some(&"Alice".to_string()));

        document.set_metadata("author".to_string(), "Bob".to_string());
        assert_eq!(document.get_metadata("author"), Some(&"Bob".to_string()));
    }

    #[test]
    pub fn content_store_error_propagation() {
        let cs = Arc::new(Mutex::new(MockContentStore::new()));
        let hash = ContentHash::from_content("some hash".as_bytes());

        cs.lock()
            .unwrap()
            .expect_retrieve()
            .with(mockall::predicate::eq(hash.clone()))
            .returning(|_| Err(anyhow::anyhow!("Store error")));

        let document = Document::new(hash.clone(), HashMap::new(), cs);

        let err = document.content().unwrap_err();
        assert_eq!(err.to_string(), "Store error");
    }
}
