use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use crate::binary16::ContentHash;

pub trait ContentStore {
    fn store(&mut self, value: &[u8]) -> anyhow::Result<ContentHash>;
    fn retrieve(&self, hash: ContentHash) -> anyhow::Result<Option<Vec<u8>>>;
}

pub struct FileSystemContentStore {
    fs: Arc<Mutex<dyn xfs::Xfs + Send + 'static>>,
    storage_path: PathBuf,
}

impl FileSystemContentStore {
    pub fn new(
        fs: Arc<Mutex<dyn xfs::Xfs + Send + 'static>>,
        storage_path: std::path::PathBuf,
    ) -> FileSystemContentStore {
        Self { fs, storage_path }
    }
}

impl ContentStore for FileSystemContentStore {
    fn store(&mut self, value: &[u8]) -> anyhow::Result<ContentHash> {
        let hash = ContentHash::from_content(value);
        let path = self.storage_path.join(hash.to_string());
        self.fs.lock().unwrap().writer(&path)?.write_all(value)?;
        Ok(hash)
    }

    fn retrieve(&self, hash: ContentHash) -> anyhow::Result<Option<Vec<u8>>> {
        let path = self.storage_path.join(hash.to_string());
        match self.fs.lock().unwrap().reader_if_exists(&path)? {
            Some(mut reader) => {
                let mut buf = vec![];
                reader.read_to_end(&mut buf)?;
                Ok(Some(buf))
            }
            None => Ok(None),
        }
    }
}

#[derive(Default)]
pub struct InMemoryContentStore {
    id_to_content: BTreeMap<ContentHash, Vec<u8>>,
}

impl ContentStore for InMemoryContentStore {
    fn store(&mut self, content: &[u8]) -> anyhow::Result<ContentHash> {
        let content_hash = ContentHash::from_content(content);
        self.id_to_content
            .insert(content_hash.clone(), content.to_vec());
        Ok(content_hash)
    }

    fn retrieve(&self, hash: ContentHash) -> anyhow::Result<Option<Vec<u8>>> {
        let result = self.id_to_content.get(&hash).map(|e| e.clone());
        Ok(result)
    }
}

#[cfg(test)]
pub mod tests {
    use std::{
        collections::BTreeMap,
        path::PathBuf,
        sync::{Arc, Mutex},
    };

    use crate::binary16::ContentHash;

    use super::{ContentStore, FileSystemContentStore, InMemoryContentStore};

    fn simple_test_case() -> (Arc<Mutex<xfs::mockfs::MockFS>>, FileSystemContentStore) {
        use xfs::Xfs;

        let mut fs = xfs::mockfs::MockFS::new();
        let storage_path = PathBuf::from("some/random/dir");
        fs.create_dir_all(&storage_path).unwrap();

        let fs = Arc::new(Mutex::new(fs));
        let store = FileSystemContentStore::new(fs.clone(), storage_path);
        (fs, store)
    }

    #[test]
    pub fn store_and_retrieve_pair_work() {
        let (_fs, mut store) = simple_test_case();
        let hash = store.store("This is a test".as_bytes()).unwrap();
        let content = store.retrieve(hash).unwrap();
        let content = content.unwrap();
        assert_eq!("This is a test", std::str::from_utf8(&content).unwrap());
    }

    #[test]
    pub fn store_writes_to_correct_path() {
        let (fs, mut store) = simple_test_case();
        let content = "dummy content".as_bytes();
        let expected_hash = ContentHash::from_content(content);
        let hash = store.store(content).unwrap();
        assert_eq!(hash, expected_hash);
        let expected_path = PathBuf::from(format!("some/random/dir/{}", hash.to_string()));
        let actual_content = fs.lock().unwrap().get(&expected_path).unwrap();
        assert_eq!(actual_content, content);
    }

    #[test]
    pub fn retrieve_reads_from_correct_path() {
        let (fs, store) = simple_test_case();
        let content = "some content".as_bytes();
        let hash = ContentHash::from_content(content);
        let expected_path = PathBuf::from(format!("some/random/dir/{}", hash.to_string()));

        fs.lock()
            .unwrap()
            .add_r(&expected_path, content.to_vec())
            .unwrap();

        let result = store.retrieve(hash).unwrap();
        let result = result.unwrap();

        assert_eq!(result, content);
    }

    // Helper function to create a new InMemoryContentStore
    fn create_store() -> InMemoryContentStore {
        InMemoryContentStore {
            id_to_content: BTreeMap::new(),
        }
    }

    #[test]
    fn test_store_and_retrieve_content() {
        let mut store = create_store();

        // Test storing and retrieving content
        let test_content = b"Hello, world!";
        let hash = store.store(test_content).expect("Failed to store content");

        // Verify the hash matches expected hash
        let retrieved_content = store
            .retrieve(hash.clone())
            .expect("Failed to retrieve content")
            .expect("Content not found");

        assert_eq!(
            test_content.to_vec(),
            retrieved_content,
            "Retrieved content does not match stored content"
        );
    }

    #[test]
    fn test_inmemory_store_multiple_contents() {
        let mut store = create_store();

        // Store multiple pieces of content
        let content1 = b"First content";
        let content2 = b"Second content";

        let hash1 = store
            .store(content1)
            .expect("Failed to store first content");
        let hash2 = store
            .store(content2)
            .expect("Failed to store second content");

        // Verify both contents can be retrieved
        let retrieved1 = store
            .retrieve(hash1.clone())
            .expect("Failed to retrieve first content")
            .expect("First content not found");
        let retrieved2 = store
            .retrieve(hash2.clone())
            .expect("Failed to retrieve second content")
            .expect("Second content not found");

        assert_eq!(
            content1.to_vec(),
            retrieved1,
            "First retrieved content does not match"
        );
        assert_eq!(
            content2.to_vec(),
            retrieved2,
            "Second retrieved content does not match"
        );
    }

    #[test]
    fn test_inmemory_retrieve_nonexistent_content() {
        let store = create_store();

        // Create a dummy content hash
        let dummy_hash = ContentHash::from_content(b"Some nonexistent content");

        // Attempt to retrieve nonexistent content
        let result = store.retrieve(dummy_hash).expect("Failed to call retrieve");

        assert!(
            result.is_none(),
            "Should return None for nonexistent content"
        );
    }

    #[test]
    fn test_inmemory_idempotent_storage() {
        let mut store = create_store();

        let content = b"Immutable content";

        // Store content first time
        let hash1 = store
            .store(content)
            .expect("Failed to store content first time");

        // Store same content again
        let hash2 = store
            .store(content)
            .expect("Failed to store content second time");

        // Hashes should be identical for identical content
        assert_eq!(
            hash1, hash2,
            "Identical content should produce the same hash"
        );

        // Verify content can be retrieved
        let retrieved_content = store
            .retrieve(hash1)
            .expect("Failed to retrieve content")
            .expect("Content not found");

        assert_eq!(
            content.to_vec(),
            retrieved_content,
            "Retrieved content does not match"
        );
    }

    #[test]
    fn test_inmemory_content_cannot_be_overwritten() {
        let mut store = create_store();

        let initial_content = b"Initial content";
        let different_content = b"Different content";

        // Store initial content
        let hash = store
            .store(initial_content)
            .expect("Failed to store initial content");

        // Attempt to "overwrite" with different content should not change the stored content
        store
            .store(different_content)
            .expect("Failed to store different content");

        // Retrieve and verify the original content remains unchanged
        let retrieved_content = store
            .retrieve(hash)
            .expect("Failed to retrieve content")
            .expect("Content not found");

        assert_eq!(
            initial_content.to_vec(),
            retrieved_content,
            "Content should remain unchanged"
        );
    }

    #[test]
    fn test_inmemory_empty_content() {
        let mut store = create_store();

        // Test storing and retrieving empty content
        let empty_content: &[u8] = &[];
        let hash = store
            .store(empty_content)
            .expect("Failed to store empty content");

        let retrieved_content = store
            .retrieve(hash.clone())
            .expect("Failed to retrieve empty content")
            .expect("Empty content not found");

        assert!(
            retrieved_content.is_empty(),
            "Retrieved content should be empty"
        );
    }
}

#[cfg(test)]
pub mod mocks {
    use super::*;
    use mockall::mock;

    mock! {
        pub ContentStore {}

        impl ContentStore for ContentStore {
            fn store(&mut self, value: &[u8]) -> anyhow::Result<ContentHash>;
            fn retrieve(&self, hash: ContentHash) -> anyhow::Result<Option<Vec<u8>>>;
        }
    }
}
