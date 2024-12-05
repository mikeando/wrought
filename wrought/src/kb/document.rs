pub use crate::binary16::DocumentId;

#[deprecated]
#[derive(Clone)]
pub struct Document {
    pub id: DocumentId,
    pub content: DocumentContent,
}

#[deprecated]
#[derive(Clone)]
pub struct DocumentContent {
    pub content: String,
}
