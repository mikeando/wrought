#[derive(Debug, Clone)]
pub enum MetadataKey {
    StringKey(String),
}

impl From<&str> for MetadataKey {
    fn from(value: &str) -> Self {
        MetadataKey::StringKey(value.to_string())
    }
}

#[derive(Debug, Clone)]
pub struct MetadataEntry {}

impl MetadataEntry {
    pub fn as_string(&self) -> String {
        todo!();
    }
}

impl From<&str> for MetadataEntry {
    fn from(value: &str) -> Self {
        MetadataEntry {}
    }
}
