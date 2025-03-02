
#[derive(Debug)]
pub enum EmbeddingError {
    TooManyTokens(usize),
    Other(anyhow::Error),
}

pub trait Embedding {
    fn estimeated_token_count(&self, data: &[u8]) -> usize;
    fn max_token_count(&self) -> usize;
    fn calculate_embedding(&self, data: &[u8]) -> Result<Vec<f32>, EmbeddingError>;
    fn embedding_length(&self) -> usize;
}

pub struct RandomEmbedding {}

impl Embedding for RandomEmbedding {
    fn estimeated_token_count(&self, data: &[u8]) -> usize {
        todo!()
    }

    fn max_token_count(&self) -> usize {
        todo!()
    }

    fn calculate_embedding(&self, data: &[u8]) -> Result<Vec<f32>, EmbeddingError> {
        todo!()
    }

    fn embedding_length(&self) -> usize {
        todo!()
    }
}
