use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

use crate::metadata;

use super::{
    document::DocumentId,
    embedding::{Embedding, EmbeddingError, RandomEmbedding},
};

#[derive(Debug)]
pub enum VectorIndexError {
    VectorSizeMismatch { expected: usize, actual: usize },
    DocumentTooLong,
    EmbeddingError(EmbeddingError),
}

pub type Result<T> = std::result::Result<T, VectorIndexError>;

pub trait VectorIndex {
    fn key_length(&self) -> usize;
    fn index_document(&mut self, document_id: &DocumentId, content: &str) -> Result<()>;
    fn key_from_content(&self, content: &str) -> Result<(Vec<f32>, ChunkMetadata)>;
    fn put(
        &mut self,
        document_id: &DocumentId,
        key: Vec<f32>,
        chunk_metadata: ChunkMetadata,
    ) -> Result<()>;
    fn search(
        &self,
        key: &[f32],
        limit: Option<usize>,
        min_similarity: Option<f32>,
    ) -> Result<Vec<(f32, DocumentId)>>;

    fn keys_for_doc_id(&self, document_id: &DocumentId) -> Result<Vec<(Vec<f32>, ChunkMetadata)>>;
    fn add_multiple_raw_for_doc(
        &mut self,
        document_id: &DocumentId,
        keys: Vec<(Vec<f32>, ChunkMetadata)>,
    ) -> Result<()>;
}

#[derive(Debug, Clone)]
pub struct Chunk<'a> {
    pub data: &'a [u8],
    pub offset: usize,
}

#[derive(PartialEq, Debug, Clone)]
pub struct ChunkMetadata {
    pub offset: usize,
    pub length: usize,
}

pub trait Chunker {
    fn chunk<'a>(&self, data: &'a [u8]) -> Vec<Chunk<'a>>;
}

struct SimpleByteChunker {}

impl Chunker for SimpleByteChunker {
    fn chunk<'a>(&self, data: &'a [u8]) -> Vec<Chunk<'a>> {
        // If there are too many tokens to do in one hit, we
        // break into overlapping regions.
        // Imagine we have 1000 tokens and we're using
        // 300 token chunks with 100 overlap
        // then the "stride" is 200
        // 000-300 page 0
        // 200-500 page 1
        // 400-700 page 2
        // 600-900 page 3
        // 800-1000 page 4 (short)

        // total length = (n-1) * (page length - page overlap) + page length
        //              = n * page length - (n-1) * page overlap
        //
        // (total length - overlap) / ( page length - overlao) = n
        // (1000 - 100) / ( 300 - 100) = (900 / 200) = 4.5

        // if 5 pages, and fixed page length = 300
        // 1000 =  5 * 300 - 4 * overlap
        // 500 = 4 * overlap
        // overlap = 125
        // [0.300], [175, 475], [350,650], [525,825], [700,1000]
        //
        // If 6 pages and fixed page length = 300
        // 1000 = 6 * 300 - 5 overlap
        // 800 = 5 overlap
        // 160 = overlap
        // [0, 300], [140, 440], [280, 580], [420, 720], [560, 860], [700,1000]
        //
        // if 4 pages and fixed page length = 300
        // 1000 = 4 * 300 - 3 * overlap
        // 200 = 3 overlap
        // 66 = overlap
        // [0, 300], [233, 533], [466, 766], [700, 1000]

        let overlap = 100_i32;
        let chunk_tokens = 300_i32;

        let n_chunks = ((data.len() as i32 - overlap) as f32) / ((chunk_tokens - overlap) as f32);
        let n_chunks = n_chunks.ceil() as i32;

        let actual_overlap = (n_chunks * chunk_tokens - data.len() as i32) / (n_chunks - 1);
        let stride = chunk_tokens - actual_overlap;

        let mut chunks = vec![];
        for i in 0..n_chunks {
            let chunk_start = i * stride;
            let chunk_end = chunk_start + chunk_tokens;
            chunks.push((chunk_start, chunk_end));
        }
        // Fix the last chunk to ensure we end at the end.
        todo!()
    }
}

pub struct InMemoryVectorIndex {
    embedding: Arc<Mutex<dyn Embedding + Send>>,
    chunker: Arc<Mutex<dyn Chunker + Send>>,
    doc_to_keys: BTreeMap<DocumentId, Vec<(Vec<f32>, ChunkMetadata)>>,
}

impl Default for InMemoryVectorIndex {
    fn default() -> Self {
        Self {
            embedding: Arc::new(Mutex::new(RandomEmbedding {})),
            chunker: Arc::new(Mutex::new(SimpleByteChunker {})),
            doc_to_keys: Default::default(),
        }
    }
}

fn similarity(key_a: &[f32], key_b: &[f32]) -> f32 {
    assert_eq!(key_a.len(), key_b.len());
    let mut sum = 0.0;
    for i in 0..key_a.len() {
        sum += key_a[i] * key_b[i];
    }
    sum
}

impl VectorIndex for InMemoryVectorIndex {
    fn key_length(&self) -> usize {
        self.embedding.lock().unwrap().embedding_length()
    }

    fn index_document(&mut self, document_id: &DocumentId, content: &str) -> Result<()> {
        // We use a super naive splitter.
        let data: Vec<u8> = content.as_bytes().to_vec();
        let chunker = self.chunker.lock().unwrap();
        let chunks = chunker.chunk(&data);
        let embedding = self.embedding.lock().unwrap();

        let mut vs: Vec<(Vec<f32>, ChunkMetadata)> = vec![];
        for chunk in chunks {
            let metadata = ChunkMetadata {
                offset: chunk.offset,
                length: chunk.data.len(),
            };
            let v = embedding
                .calculate_embedding(chunk.data)
                .map_err(|e| VectorIndexError::EmbeddingError(e))?;
            vs.push((v, metadata));
        }

        self.doc_to_keys
            .entry(document_id.clone())
            .or_default()
            .append(&mut vs);
        Ok(())
    }

    fn key_from_content(&self, content: &str) -> Result<(Vec<f32>, ChunkMetadata)> {
        todo!();
    }

    fn put(
        &mut self,
        document_id: &DocumentId,
        key: Vec<f32>,
        chunk_metadata: ChunkMetadata,
    ) -> Result<()> {
        assert_eq!(key.len(), self.embedding.lock().unwrap().embedding_length());
        self.doc_to_keys
            .entry(document_id.clone())
            .or_default()
            .push((key, chunk_metadata));
        Ok(())
    }

    fn search(
        &self,
        key: &[f32],
        limit: Option<usize>,
        min_similarity: Option<f32>,
    ) -> Result<Vec<(f32, DocumentId)>> {
        let mut acceptable_entries: Vec<_> = self
            .doc_to_keys
            .iter()
            .filter_map(|(doc, keys)| {
                let max_similarity: f32 = keys
                    .iter()
                    .map(|doc_key| similarity(key, &doc_key.0))
                    .reduce(f32::max)
                    .unwrap_or(-1.0);
                if min_similarity.map(|ms| max_similarity > ms).unwrap_or(true) {
                    Some((max_similarity, doc.clone()))
                } else {
                    None
                }
            })
            .collect();
        acceptable_entries.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        if let Some(limit) = limit {
            Ok(acceptable_entries[0..limit].to_vec())
        } else {
            Ok(acceptable_entries)
        }
    }

    fn keys_for_doc_id(&self, document_id: &DocumentId) -> Result<Vec<(Vec<f32>, ChunkMetadata)>> {
        match self.doc_to_keys.get(document_id) {
            Some(keys) => Ok(keys.clone()),
            None => Ok(vec![]),
        }
    }

    fn add_multiple_raw_for_doc(
        &mut self,
        document_id: &DocumentId,
        keys: Vec<(Vec<f32>, ChunkMetadata)>,
    ) -> Result<()> {
        let mut keys = keys;
        self.doc_to_keys
            .entry(document_id.clone())
            .or_default()
            .append(&mut keys);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        sync::{Arc, Mutex},
    };

    use mockall::predicate;

    use crate::{
        binary16::DocumentId,
        content_store::mocks,
        kb::{mock, vector_index::Chunk},
    };

    use super::{InMemoryVectorIndex, VectorIndex};

    #[test]
    pub fn can_store_document() {
        let content = "Lorem Ipsum something something";
        let L = content.len();
        let first_slice = &content.as_bytes()[0..2 * L / 3];
        let second_slice = &content.as_bytes()[L / 3..L];

        let mut mock_embedding = mock::internal::MockEmbedding::default();
        mock_embedding
            .expect_calculate_embedding()
            .with(predicate::eq(first_slice))
            .times(1)
            .returning(|_| Ok(vec![1.0, 2.0, 3.0]));
        mock_embedding
            .expect_calculate_embedding()
            .with(predicate::eq(second_slice))
            .times(1)
            .returning(|_| Ok(vec![3.0, 2.0, 1.0]));

        let mock_embedding = Arc::new(Mutex::new(mock_embedding));

        let mut mock_chunker = mock::internal::MockChunker {
            expectations: vec![],
        };
        let chunks = vec![
            Chunk {
                data: first_slice,
                offset: 0,
            },
            Chunk {
                data: second_slice,
                offset: L / 3,
            },
        ];
        mock_chunker
            .expectations
            .push((content.as_bytes(), chunks.clone()));
        let mock_chunker = Arc::new(Mutex::new(mock_chunker));

        let mut vector_db = InMemoryVectorIndex {
            embedding: mock_embedding.clone(),
            chunker: mock_chunker.clone(),
            doc_to_keys: BTreeMap::new(),
        };
        let document_id = DocumentId::from_u64s(1234, 4321);
        vector_db.index_document(&document_id, content).unwrap();

        let keys = vector_db.keys_for_doc_id(&document_id).unwrap();
        let expected_keys = vec![
            (
                vec![1.0, 2.0, 3.0],
                super::ChunkMetadata {
                    offset: 0,
                    length: 2 * L / 3,
                },
            ),
            (
                vec![3.0, 2.0, 1.0],
                super::ChunkMetadata {
                    offset: L / 3,
                    length: L - L / 3,
                },
            ),
        ];
        assert_eq!(keys, expected_keys);
    }
}
