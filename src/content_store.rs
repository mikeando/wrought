use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use crate::binary16::ContentHash;

pub trait ContentStore {
    fn store(&mut self, value: &[u8]) -> anyhow::Result<ContentHash>;
    fn retrieve(&self, hash: ContentHash) -> anyhow::Result<Option<Vec<u8>>>;
}

pub struct DummyContentStore {
    fs: Arc<Mutex<dyn xfs::Xfs>>,
    storage_path: PathBuf,
}

impl DummyContentStore {
    pub fn new(
        fs: Arc<Mutex<dyn xfs::Xfs>>,
        storage_path: std::path::PathBuf,
    ) -> DummyContentStore {
        Self { fs, storage_path }
    }
}

impl ContentStore for DummyContentStore {
    fn store(&mut self, value: &[u8]) -> anyhow::Result<ContentHash> {
        println!(
            "DummyContentStore: store('{:?}')",
            String::from_utf8_lossy(value)
        );
        let hash = ContentHash::from_content(value);
        let path = self.storage_path.join(hash.to_string());
        self.fs.lock().unwrap().writer(&path)?.write_all(value)?;
        Ok(hash)
    }

    fn retrieve(&self, hash: ContentHash) -> anyhow::Result<Option<Vec<u8>>> {
        println!("DummyContentStore: retrieve({:?})", hash.to_string());
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

#[cfg(test)]
pub mod tests {
    use std::{
        path::PathBuf,
        sync::{Arc, Mutex},
    };

    use crate::binary16::ContentHash;

    use super::{ContentStore, DummyContentStore};

    fn simple_test_case() -> (Arc<Mutex<xfs::mockfs::MockFS>>, DummyContentStore) {
        use xfs::Xfs;

        let mut fs = xfs::mockfs::MockFS::new();
        let storage_path = PathBuf::from("some/random/dir");
        fs.create_dir_all(&storage_path).unwrap();

        let fs = Arc::new(Mutex::new(fs));
        let store = DummyContentStore::new(fs.clone(), storage_path);
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
        let (fs, mut store) = simple_test_case();
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
}
