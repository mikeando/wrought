#[derive(Debug, Clone)]
pub enum MetadataKey {
    StringKey(String),
}

impl MetadataKey {
    pub fn as_string(&self) -> String {
        match self {
            MetadataKey::StringKey(k) => k.clone(),
        }
    }
}

impl From<&str> for MetadataKey {
    fn from(value: &str) -> Self {
        MetadataKey::StringKey(value.to_string())
    }
}

#[derive(Debug, Clone)]
pub struct MetadataEntry {
    value: String,
}

impl MetadataEntry {
    pub fn as_string(&self) -> String {
        self.value.clone()
    }
}

impl From<&str> for MetadataEntry {
    fn from(value: &str) -> Self {
        MetadataEntry {
            value: value.to_string(),
        }
    }
}
