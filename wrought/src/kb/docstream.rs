use std::{
    collections::{HashSet, VecDeque},
    sync::Arc,
};

use super::{
    document::{Document, DocumentId},
    knowledgebase::KnowledgeBase,
};

pub struct DocumentStream {
    pub kb: KnowledgeBase,
    pub info: DocumentStreamInfo,
}

#[derive(Clone)]
pub enum DocumentStreamInfo {
    AllDocs,
    EmptyStream,
    Filter(DocumentStreamFilter, Box<DocumentStreamInfo>),
    Sequence(Vec<DocumentStreamInfo>),
    WithTextQuery(String, Box<DocumentStreamInfo>),
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
                let infos: Vec<_> = infos
                    .iter()
                    .map(|v| DocumentStreamInfo::Filter(f.clone(), Box::new(v.clone())))
                    .collect();
                if infos.is_empty() {
                    DocumentStreamInfo::EmptyStream
                } else {
                    DocumentStreamInfo::Sequence(infos)
                }
            }
            DocumentStreamInfo::WithTextQuery(s, info) => DocumentStreamInfo::Filter(
                f,
                Box::new(DocumentStreamInfo::WithTextQuery(s.clone(), info.clone())),
            ),
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

    pub fn with_text_query(&self, query: &str) -> DocumentStream {
        let info = self.info.clone();

        let info = DocumentStreamInfo::WithTextQuery(query.to_string(), Box::new(info));

        DocumentStream {
            kb: self.kb.clone(),
            info,
        }
    }

    pub fn query(&self) -> Vec<Document> {
        self.iter().collect()
    }

    pub fn iter(&self) -> DocumentStreamIterator {
        // TODO: Probably just move this into KnowledgeDb
        self.kb.create_iter(&self.info)
    }
}

#[derive(Clone)]
pub enum DocumentStreamFilter {
    And(Box<DocumentStreamFilter>, Box<DocumentStreamFilter>),
    Or(Box<DocumentStreamFilter>, Box<DocumentStreamFilter>),
    Not(Box<DocumentStreamFilter>),
    FilterFn(Arc<dyn Fn(&Document) -> bool>),
    MetadataEquals(String, String),
    MetadataContains(String, String),
    MetadataRegex(String, String),
    IdInSet(HashSet<DocumentId>),
}

impl DocumentStreamFilter {
    pub fn and(f1: DocumentStreamFilter, f2: DocumentStreamFilter) -> DocumentStreamFilter {
        DocumentStreamFilter::And(Box::new(f1), Box::new(f2))
    }

    pub fn or(f1: DocumentStreamFilter, f2: DocumentStreamFilter) -> DocumentStreamFilter {
        DocumentStreamFilter::Or(Box::new(f1), Box::new(f2))
    }

    pub fn not(f: DocumentStreamFilter) -> DocumentStreamFilter {
        DocumentStreamFilter::Not(Box::new(f))
    }

    pub fn metadata_equals(key: String, value: String) -> DocumentStreamFilter {
        DocumentStreamFilter::MetadataEquals(key, value)
    }

    pub fn metadata_contains(key: String, value: String) -> DocumentStreamFilter {
        DocumentStreamFilter::MetadataContains(key, value)
    }

    pub fn metadata_regex(key: String, regex: String) -> DocumentStreamFilter {
        DocumentStreamFilter::MetadataRegex(key, regex)
    }

    pub fn should_keep(&self, v: &Document) -> bool {
        match self {
            DocumentStreamFilter::And(f1, f2) => f1.should_keep(v) && f2.should_keep(v),
            DocumentStreamFilter::Or(f1, f2) => f1.should_keep(v) || f2.should_keep(v),
            DocumentStreamFilter::Not(f) => !f.should_keep(v),
            DocumentStreamFilter::FilterFn(f) => f(v),
            DocumentStreamFilter::MetadataEquals(key, value) => {
                v.get_metadata(key).map_or(false, |v| v == value)
            }
            DocumentStreamFilter::MetadataContains(key, value) => {
                v.get_metadata(key).map_or(false, |v| v.contains(value))
            }
            DocumentStreamFilter::MetadataRegex(key, regex) => {
                use regex::Regex;
                v.get_metadata(key)
                    .and_then(|v| Regex::new(regex).ok().map(|r| r.is_match(v)))
                    .unwrap_or(false)
            }
            DocumentStreamFilter::IdInSet(ids) => ids.contains(v.id()),
        }
    }

    // TODO: Use a more general type here?
    pub(crate) fn id_in_set(doc_ids: Vec<DocumentId>) -> DocumentStreamFilter {
        DocumentStreamFilter::IdInSet(doc_ids.into_iter().collect())
    }
}

pub struct DocumentStreamIteratorList {
    pending: VecDeque<Document>,
}

pub enum DocumentStreamIteratorDelayed {
    Unevaluated(KnowledgeBase, DocumentStreamInfo),
    Evaluated(Box<DocumentStreamIterator>),
}

pub enum DocumentStreamIterator {
    ListOfDocs(DocumentStreamIteratorList),
    Empty,
    Filter(DocumentStreamFilter, Box<DocumentStreamIterator>),
    Sequence(VecDeque<DocumentStreamIterator>),
    Delayed(DocumentStreamIteratorDelayed),
}

impl DocumentStreamIterator {
    pub fn list(pending: Vec<Document>) -> DocumentStreamIterator {
        DocumentStreamIterator::ListOfDocs(DocumentStreamIteratorList {
            pending: VecDeque::from(pending),
        })
    }

    pub fn empty() -> DocumentStreamIterator {
        DocumentStreamIterator::Empty
    }

    pub fn filter(f: DocumentStreamFilter, it: DocumentStreamIterator) -> DocumentStreamIterator {
        DocumentStreamIterator::Filter(f, Box::new(it))
    }

    pub fn sequence(its: Vec<DocumentStreamIterator>) -> DocumentStreamIterator {
        DocumentStreamIterator::Sequence(VecDeque::from(its))
    }
}

impl Iterator for DocumentStreamIteratorList {
    type Item = Document;

    fn next(&mut self) -> Option<Self::Item> {
        self.pending.pop_front()
    }
}

impl Iterator for DocumentStreamIteratorDelayed {
    type Item = Document;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            DocumentStreamIteratorDelayed::Unevaluated(kb, info) => {
                let mut it = kb.create_iter(info);
                let e = it.next();
                *self = DocumentStreamIteratorDelayed::Evaluated(Box::new(it));
                e
            }
            DocumentStreamIteratorDelayed::Evaluated(it) => it.next(),
        }
    }
}

impl Iterator for DocumentStreamIterator {
    type Item = Document;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            DocumentStreamIterator::ListOfDocs(v) => v.next(),
            DocumentStreamIterator::Empty => None,
            DocumentStreamIterator::Filter(f, it) => {
                for e in it.by_ref() {
                    if f.should_keep(&e) {
                        return Some(e);
                    }
                }
                None
            }
            DocumentStreamIterator::Sequence(its) => loop {
                if its.is_empty() {
                    return None;
                }
                let e = its[0].next();
                if let Some(e) = e {
                    return Some(e);
                }
                its.pop_front();
            },
            DocumentStreamIterator::Delayed(delayed) => delayed.next(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        binary16::ContentHash,
        content_store::mocks::MockContentStore,
        kb::document::{Document, DocumentId},
    };
    use std::sync::Arc;
    use std::{collections::HashMap, sync::Mutex};

    fn create_test_document(
        metadata: HashMap<String, String>,
        content_store: Arc<Mutex<MockContentStore>>,
    ) -> Document {
        let hash = ContentHash::from_content(b"test content");
        let doc_id = DocumentId { id: hash };

        Document::new(doc_id, metadata, content_store)
    }

    #[test]
    fn test_metadata_equals_filter() {
        let content_store = Arc::new(Mutex::new(MockContentStore::new()));

        let mut metadata = HashMap::new();
        metadata.insert("author".to_string(), "Alice".to_string());

        let doc = create_test_document(metadata, content_store);

        let filter =
            DocumentStreamFilter::metadata_equals("author".to_string(), "Alice".to_string());

        assert!(filter.should_keep(&doc));

        let wrong_filter =
            DocumentStreamFilter::metadata_equals("author".to_string(), "Bob".to_string());

        assert!(!wrong_filter.should_keep(&doc));
    }

    #[test]
    fn test_metadata_contains_filter() {
        let content_store = Arc::new(Mutex::new(MockContentStore::new()));

        let mut metadata = HashMap::new();
        metadata.insert("tags".to_string(), "rust programming language".to_string());

        let doc = create_test_document(metadata, content_store);

        let filter =
            DocumentStreamFilter::metadata_contains("tags".to_string(), "rust".to_string());

        assert!(filter.should_keep(&doc));

        let wrong_filter =
            DocumentStreamFilter::metadata_contains("tags".to_string(), "python".to_string());

        assert!(!wrong_filter.should_keep(&doc));
    }

    #[test]
    fn test_metadata_regex_filter() {
        let content_store = Arc::new(Mutex::new(MockContentStore::new()));

        let mut metadata = HashMap::new();
        metadata.insert("email".to_string(), "alice@example.com".to_string());

        let doc = create_test_document(metadata, content_store);

        let filter = DocumentStreamFilter::metadata_regex(
            "email".to_string(),
            r"^alice@.*\.com$".to_string(),
        );

        assert!(filter.should_keep(&doc));

        let wrong_filter =
            DocumentStreamFilter::metadata_regex("email".to_string(), r"^bob@.*\.org$".to_string());

        assert!(!wrong_filter.should_keep(&doc));
    }

    #[test]
    fn test_complex_filter_combinations() {
        let content_store = Arc::new(Mutex::new(MockContentStore::new()));

        let mut metadata1 = HashMap::new();
        metadata1.insert("author".to_string(), "Alice".to_string());
        metadata1.insert("tags".to_string(), "rust programming".to_string());

        let mut metadata2 = HashMap::new();
        metadata2.insert("author".to_string(), "Bob".to_string());
        metadata2.insert("tags".to_string(), "python programming".to_string());

        let doc1 = create_test_document(metadata1, content_store.clone());
        let doc2 = create_test_document(metadata2, content_store);

        // Filter for Rust documents by Alice
        let filter = DocumentStreamFilter::and(
            DocumentStreamFilter::metadata_equals("author".to_string(), "Alice".to_string()),
            DocumentStreamFilter::metadata_contains("tags".to_string(), "rust".to_string()),
        );

        assert!(filter.should_keep(&doc1));
        assert!(!filter.should_keep(&doc2));

        // Filter for non-Rust documents
        let not_rust_filter = DocumentStreamFilter::not(DocumentStreamFilter::metadata_contains(
            "tags".to_string(),
            "rust".to_string(),
        ));

        assert!(!not_rust_filter.should_keep(&doc1));
        assert!(not_rust_filter.should_keep(&doc2));
    }
}
