use crate::binary16::ContentHash;

pub trait ContentStore {
    fn store(&mut self, value: &[u8]) -> anyhow::Result<ContentHash>;
    fn retrieve(&self, hash: ContentHash) -> anyhow::Result<Option<Vec<u8>>>;
}

pub struct DummyContentStore {}

impl ContentStore for DummyContentStore {
    fn store(&mut self, value: &[u8]) -> anyhow::Result<ContentHash> {
        println!(
            "DummyContentStore: store('{:?}')",
            String::from_utf8_lossy(value)
        );
        Ok(ContentHash::from_content(value))
    }

    fn retrieve(&self, hash: ContentHash) -> anyhow::Result<Option<Vec<u8>>> {
        println!("DummyContentStore: retrieve({:?})", hash.to_string());
        Ok(None)
    }
}
