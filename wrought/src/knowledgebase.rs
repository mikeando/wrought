use std::{collections::{HashMap, VecDeque}, sync::{Arc, Mutex}};

use crate::binary16::ContentHash;


#[derive(Clone)]
pub struct KnowledgeBase {
    kb: Arc<Mutex<dyn KnowledgeBaseImpl>>
}

pub trait KnowledgeBaseImpl {
    fn create_iter(&self, info: &DocumentStreamInfo) -> DocumentStreamIterator;
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
        DocumentStream{ kb: self.clone(), info: DocumentStreamInfo::AllDocs }
    }
}

impl DocumentStream {
    pub fn filter(&self, f: DocumentStreamFilter ) -> DocumentStream {
        let info = match &self.info {
            DocumentStreamInfo::AllDocs => DocumentStreamInfo::Filter(f, Box::new(DocumentStreamInfo::AllDocs)),
            DocumentStreamInfo::EmptyStream => DocumentStreamInfo::EmptyStream,
            DocumentStreamInfo::Filter(f2, info) =>  {
                DocumentStreamInfo::Filter(DocumentStreamFilter::and(f2.clone(), f), info.clone())
            },
            DocumentStreamInfo::Sequence(infos) => {
                // f(seq(a,b,c)) = seq(f(a), f(b), f(c))
                let infos: Vec<_> = infos.iter().filter(|v| f.should_keep(v) ).cloned().collect();
                if infos.is_empty() {
                    DocumentStreamInfo::EmptyStream
                } else {
                    DocumentStreamInfo::Sequence(infos)
                }
            }
        };
        DocumentStream{ kb: self.kb.clone(), info }
    }

    pub fn sequence(kb: KnowledgeBase, streams: &[DocumentStream]) -> DocumentStream {
        let mut infos = vec![];
        for s in streams {
            // TODO: Should we report an error instead?
            assert!(Arc::ptr_eq(&kb.kb, &s.kb.kb));

            match s.info {
                DocumentStreamInfo::EmptyStream => { },
                _ => {
                    infos.push(s.info.clone())
                }
            }
        }

        if infos.len() == 0 {
            return DocumentStream{ kb, info: DocumentStreamInfo::EmptyStream};
        }
        if infos.len() == 1 {
            return DocumentStream{ kb, info: infos[0].clone() }
        }
        DocumentStream{ kb: kb, info: DocumentStreamInfo::Sequence(infos)}

    }

    pub fn query(&self) -> Vec<Document> {
        self.iter().collect()
    }

    pub fn iter(&self) -> DocumentStreamIterator {
        self.kb.kb.lock().unwrap().create_iter(&self.info)
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
    metadata: HashMap<String,String>,
}

impl Document {
    /// Create a new document with the given ID and metadata.
    pub fn new(id: ContentHash, metadata: HashMap<String, String>) -> Self {
        Document { id, metadata }
    }

    /// Retrieve the document ID.
    pub fn id(&self) -> &ContentHash {
        &self.id
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
    pending: VecDeque<Document>
}

pub enum DocumentStreamIterator {
    ListOfDocs(DocumentStreamIteratorList)
}

impl DocumentStreamIterator {
    fn list(pending: Vec<Document>) -> DocumentStreamIterator {
        DocumentStreamIterator::ListOfDocs(DocumentStreamIteratorList { pending: VecDeque::from(pending) })
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
            DocumentStreamIterator::ListOfDocs(v) => {
                v.next()
            },
        }
    }
}

#[cfg(test)]
pub mod test {
    use super::*;

    pub struct DummyKnowledgeBaseImpl {

    }

    impl KnowledgeBaseImpl for DummyKnowledgeBaseImpl {
        fn create_iter(&self, info: &DocumentStreamInfo) -> DocumentStreamIterator {
            DocumentStreamIterator::list(
                vec![Document{}, Document{}, Document{}, Document{}]
            )
        }
    }

    pub fn dummy_kb() -> Arc<Mutex<DummyKnowledgeBaseImpl>> {
        Arc::new(Mutex::new(DummyKnowledgeBaseImpl{}))
    }

    #[test]
    pub fn basic_stream() {
        let kb_impl = dummy_kb();
        let kb = KnowledgeBase{ kb: kb_impl };

        let docs = kb.all_docs().query();

        assert_eq!(docs.len(), 4);
    }
}