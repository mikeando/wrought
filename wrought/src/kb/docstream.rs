use std::{collections::VecDeque, sync::Arc};

use super::{document::Document, knowledgebase::KnowledgeBase};

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
        self.kb.create_iter(&self.info)
    }
}

#[derive(Clone)]
pub enum DocumentStreamFilter {
    And(Box<DocumentStreamFilter>, Box<DocumentStreamFilter>),
    FilterFn(Arc<dyn Fn(&Document) -> bool>),
}

impl DocumentStreamFilter {
    pub fn and(f1: DocumentStreamFilter, f2: DocumentStreamFilter) -> DocumentStreamFilter {
        DocumentStreamFilter::And(Box::new(f1), Box::new(f2))
    }

    fn should_keep(&self, v: &Document) -> bool {
        match self {
            DocumentStreamFilter::And(f1, f2) => f1.should_keep(v) && f2.should_keep(v),
            DocumentStreamFilter::FilterFn(f) => f(v),
        }
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
    Delayed(DocumentStreamIteratorDelayed)
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
                return e;
            },
            DocumentStreamIteratorDelayed::Evaluated(it) => {
                it.next()
            }
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
                while let Some(e) = it.next() {
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