use std::path::Path;

use anyhow::Result;
use tantivy::schema::{Value, STRING};
use tantivy::{
    collector::TopDocs,
    directory::MmapDirectory,
    index::Index,
    query::QueryParser,
    schema::{Field, Schema, STORED, TEXT},
};
use tantivy::{IndexSettings, TantivyDocument};

use super::document::DocumentId;

pub trait FullTextIndex {
    fn index_document(&self, document_id: &DocumentId, content: &str) -> Result<()>;
    fn search(&self, query_str: &str, limit: usize) -> Result<Vec<(f32, DocumentId)>>;
}

// TODO: We could add additional fields that will be indexed.
// NOTE: We should not use tantivy as a _store_ for info, just an index, as
//       we'll need to rebuild it when schemas change etc. Which may be fragile.
pub struct TantivyFullTextIndex {
    index: Index,
    id_field: Field,
    content_field: Field,
}

impl TantivyFullTextIndex {
    fn create_with_index<F: Fn(&Schema) -> Result<Index>>(f: F) -> Result<Self> {
        // Create schema
        let mut schema_builder = Schema::builder();

        // Add id field
        let id_field = schema_builder.add_text_field("id", STRING | STORED);

        // Add content field
        let content_field = schema_builder.add_text_field("content", TEXT);

        let schema = schema_builder.build();

        // Create or open index
        let index = f(&schema)?;

        Ok(TantivyFullTextIndex {
            index,
            id_field,
            content_field,
        })
    }

    fn create_or_open_index(index_path: &Path, schema: &Schema) -> Result<Index> {
        std::fs::create_dir_all(index_path)?;
        let directory = MmapDirectory::open(index_path)?;
        let index_settings = IndexSettings::default();
        let index = Index::create(directory, schema.clone(), index_settings)?;
        Ok(index)
    }

    fn in_memory_index(schema: &Schema) -> Result<Index> {
        Ok(Index::create_in_ram(schema.clone()))
    }

    pub fn new(index_path: &Path) -> Result<Self> {
        Self::create_with_index(|schema| Self::create_or_open_index(index_path, schema))
    }

    // Mostly for testing...
    pub fn new_in_memory() -> Result<Self> {
        Self::create_with_index(Self::in_memory_index)
    }
}

impl FullTextIndex for TantivyFullTextIndex {
    fn index_document(&self, document_id: &DocumentId, content: &str) -> Result<()> {
        let mut index_writer = self.index.writer(50_000_000)?;

        // Prepare document for indexing
        let mut doc = tantivy::TantivyDocument::new();
        doc.add_text(self.id_field, document_id.to_string());
        doc.add_text(self.content_field, content);

        // Commit the document
        index_writer.add_document(doc)?;
        index_writer.commit()?;

        Ok(())
    }

    fn search(&self, query_str: &str, limit: usize) -> Result<Vec<(f32, DocumentId)>> {
        let reader = self.index.reader()?;
        let searcher = reader.searcher();

        // Create query parser
        let query_parser = QueryParser::for_index(&self.index, vec![self.content_field]);

        // Parse query
        let query = query_parser.parse_query(query_str)?;

        // Search
        let top_docs = searcher.search(&query, &TopDocs::with_limit(limit))?;

        // Convert results
        let mut results = Vec::new();
        for (score, doc_address) in top_docs {
            let retrieved_doc: TantivyDocument = searcher.doc(doc_address)?;

            // Get document id
            let doc_id = retrieved_doc
                .get_first(self.id_field)
                .and_then(|v| v.as_str())
                .map(|s| DocumentId::from_string(s).unwrap())
                .unwrap();

            results.push((score, doc_id));
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_full_text_indexing_and_search() -> Result<()> {
        // Create full-text index
        let full_text_index = TantivyFullTextIndex::new_in_memory()?;

        // Create content store
        let doc_content = "This is a bunch of random content";
        let doc_id = DocumentId::from_u64s(10, 20);

        // Index the document
        full_text_index.index_document(&doc_id, doc_content)?;

        // Search
        let results = full_text_index.search("random", 10)?;

        assert_eq!(1, results.len());
        assert_eq!(doc_id, results[0].1);

        let results = full_text_index.search("banana", 10)?;
        assert_eq!(0, results.len());
        Ok(())
    }
}
