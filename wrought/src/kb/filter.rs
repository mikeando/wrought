use std::{collections::HashSet, sync::Arc};

use super::{document::DocumentId, knowledgebase::KnowledgeBase};

#[derive(Clone)]
pub enum Filter {
    And(Box<Filter>, Box<Filter>),
    Or(Box<Filter>, Box<Filter>),
    Not(Box<Filter>),
    FilterFn(Arc<dyn Fn(&DocumentId) -> bool>),
    MetadataEquals(String, String),
    MetadataContains(String, String),
    MetadataRegex(String, String),
    IdInSet(HashSet<DocumentId>),
}

impl Filter {
    pub fn and(f1: Filter, f2: Filter) -> Filter {
        Filter::And(Box::new(f1), Box::new(f2))
    }

    pub fn or(f1: Filter, f2: Filter) -> Filter {
        Filter::Or(Box::new(f1), Box::new(f2))
    }

    pub fn not(f: Filter) -> Filter {
        Filter::Not(Box::new(f))
    }

    pub fn metadata_equals(key: String, value: String) -> Filter {
        Filter::MetadataEquals(key, value)
    }

    pub fn metadata_contains(key: String, value: String) -> Filter {
        Filter::MetadataContains(key, value)
    }

    pub fn metadata_regex(key: String, regex: String) -> Filter {
        Filter::MetadataRegex(key, regex)
    }

    // TODO: Use a more general type here?
    pub fn id_in_set(doc_ids: Vec<DocumentId>) -> Filter {
        Filter::IdInSet(doc_ids.into_iter().collect())
    }

    pub fn should_keep(&self, kb: &KnowledgeBase, v: &DocumentId) -> bool {
        match self {
            Filter::And(f1, f2) => f1.should_keep(kb, v) && f2.should_keep(kb, v),
            Filter::Or(f1, f2) => f1.should_keep(kb, v) || f2.should_keep(kb, v),
            Filter::Not(f) => !f.should_keep(kb, v),
            Filter::FilterFn(f) => f(v),
            Filter::MetadataEquals(key, value) => {
                kb.get_metadata(v, key).map_or(false, |v| v == *value)
            }
            Filter::MetadataContains(key, value) => {
                kb.get_metadata(v, key).map_or(false, |v| v.contains(value))
            }
            Filter::MetadataRegex(key, regex) => {
                use regex::Regex;
                kb.get_metadata(v, key)
                    .and_then(|v| Regex::new(regex).ok().map(|r| r.is_match(&v)))
                    .unwrap_or(false)
            }
            Filter::IdInSet(ids) => ids.contains(v),
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::kb::{document::DocumentId, knowledgebase::test::create_test_kb};

    #[test]
    fn test_metadata_equals_filter() {
        let (kb, mut components) = create_test_kb();

        let doc_id = DocumentId::from_u64s(100, 200);
        components.set_document_metadata(&doc_id, &[("author", "Alice")]);

        let filter = Filter::metadata_equals("author".to_string(), "Alice".to_string());

        assert!(filter.should_keep(&kb, &doc_id));

        let wrong_filter = Filter::metadata_equals("author".to_string(), "Bob".to_string());

        assert!(!wrong_filter.should_keep(&kb, &doc_id));
    }

    #[test]
    fn test_metadata_contains_filter() {
        let doc_id = DocumentId::from_u64s(100, 200);

        let (kb, mut components) = create_test_kb();
        components.set_document_metadata(&doc_id, &[("tags", "rust dancing")]);

        let filter = Filter::metadata_contains("tags".to_string(), "rust".to_string());

        assert!(filter.should_keep(&kb, &doc_id));

        let wrong_filter = Filter::metadata_contains("tags".to_string(), "python".to_string());

        assert!(!wrong_filter.should_keep(&kb, &doc_id));
    }

    #[test]
    fn test_metadata_regex_filter() {
        let (kb, mut components) = create_test_kb();
        let doc_id = DocumentId::from_u64s(100, 200);

        components.set_document_metadata(&doc_id, &[("email", "alice@somewhere.com")]);

        let filter = Filter::metadata_regex("email".to_string(), r"^alice@.*\.com$".to_string());

        assert!(filter.should_keep(&kb, &doc_id));

        let wrong_filter =
            Filter::metadata_regex("email".to_string(), r"^bob@.*\.org$".to_string());

        assert!(!wrong_filter.should_keep(&kb, &doc_id));
    }

    #[test]
    fn test_complex_filter_combinations() {
        let (kb, mut components) = create_test_kb();
        let doc_id1 = DocumentId::from_u64s(100, 200);
        let doc_id2 = DocumentId::from_u64s(234, 567);

        components
            .set_document_metadata(&doc_id1, &[("tags", "rust dancing"), ("author", "Alice")]);
        components.set_document_metadata(&doc_id2, &[("tags", "other stuff"), ("author", "Alice")]);

        // Filter for Rust documents by Alice
        let filter = Filter::and(
            Filter::metadata_equals("author".to_string(), "Alice".to_string()),
            Filter::metadata_contains("tags".to_string(), "rust".to_string()),
        );

        assert!(filter.should_keep(&kb, &doc_id1));
        assert!(!filter.should_keep(&kb, &doc_id2));

        // Filter for non-Rust documents
        let not_rust_filter = Filter::not(Filter::metadata_contains(
            "tags".to_string(),
            "rust".to_string(),
        ));

        assert!(!not_rust_filter.should_keep(&kb, &doc_id1));
        assert!(not_rust_filter.should_keep(&kb, &doc_id2));
    }
}
