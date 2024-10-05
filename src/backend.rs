use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use crate::{
    binary16::ContentHash,
    content_store::ContentStore,
    metadata::{MetadataEntry, MetadataKey},
};

use anyhow::anyhow;

/// The backend is purely to access the data,
/// it does not provide loging of the events, nor
/// infrastructure. It is the lowest level.
///
/// It is very similar to what scripts get access to,
/// but they are handed a wrapper that performs event logging
/// and other validation.

pub trait Backend {
    fn get_metadata(&self, path: &Path, key: &MetadataKey)
        -> anyhow::Result<Option<MetadataEntry>>;
    /// Returns the old value - if any.
    fn set_metadata(
        &self,
        path: &Path,
        key: &MetadataKey,
        value: &Option<MetadataEntry>,
    ) -> anyhow::Result<Option<MetadataEntry>>;
    fn write_file(
        &self,
        path: &Path,
        value: &[u8],
    ) -> anyhow::Result<(Option<ContentHash>, ContentHash)>;
    fn read_file(&self, path: &Path) -> anyhow::Result<Option<(ContentHash, Vec<u8>)>>;
}

// -----------------

pub struct DummyBackend {
    pub fs: Arc<Mutex<dyn xfs::Xfs>>,
    pub root: PathBuf,
    pub content_store: Arc<Mutex<dyn ContentStore>>,
}

impl Backend for DummyBackend {
    fn get_metadata(
        &self,
        path: &Path,
        key: &MetadataKey,
    ) -> anyhow::Result<Option<MetadataEntry>> {
        eprintln!("DummyBackend::get_metadata({:?}, {:?})", path, key);
        let md_path = self.root.join(".wrought").join("metadata.json");
        let md_store: BTreeMap<String, BTreeMap<String, String>> = match self.fs.lock().unwrap().reader_if_exists(&md_path)? {
            Some(reader) => {
                serde_json::from_reader(reader)?
            }
            None => BTreeMap::new(),
        };
        let v = md_store
            .get(&path.display().to_string())
            .and_then(|c| c.get(&key.as_string()));

        Ok(v.map(|s| MetadataEntry::from(s.as_str())))
    }

    fn set_metadata(
        &self,
        path: &Path,
        key: &MetadataKey,
        value: &Option<MetadataEntry>,
    ) -> anyhow::Result<Option<MetadataEntry>> {
        eprintln!(
            "DummyBackend::set_metadata({:?}, {:?}, {:?})",
            path, key, value
        );
        let md_path = self.root.join(".wrought").join("metadata.json");
        let mut md_store: BTreeMap<String, BTreeMap<String, String>> = match self.fs.lock().unwrap().reader_if_exists(&md_path)? {
            Some(reader) => {
                serde_json::from_reader(reader)?
            }
            None => BTreeMap::new(),
        };
        let original = md_store
            .get(&path.display().to_string())
            .and_then(|m| m.get(&key.as_string()));
        let original = original.map(|v| MetadataEntry::from(v.as_str()));
        if let Some(v) = value {
            md_store
                .entry(path.display().to_string())
                .or_default()
                .insert(key.as_string(), v.as_string());
        } else {
            let clean = if let Some(x) = md_store.get_mut(&path.display().to_string()) {
                x.remove(&key.as_string());
                x.is_empty()
            } else {
                false
            };
            if clean {
                md_store.remove(&path.display().to_string());
            }
        }
        
        let writer = self.fs.lock().unwrap().writer(&md_path)?;
        serde_json::to_writer_pretty(writer, &md_store)?;
        eprintln!(
            "DONE DummyBackend::set_metadata({:?}, {:?}, {:?}) -> {:?}",
            path, key, value, original
        );
        Ok(original)
    }

    fn write_file(
        &self,
        path: &Path,
        value: &[u8],
    ) -> anyhow::Result<(Option<ContentHash>, ContentHash)> {
        eprintln!(
            "DummyBackend::write_file({:?}, {:?})",
            path,
            String::from_utf8_lossy(value).to_string()
        );

        let p = self.root.join(path);

        // Check if the file exists
        let original_hash =match self.fs.lock().unwrap().reader_if_exists(&p)? {
            Some(mut reader) => {
                let mut content = vec![];
                reader.read_to_end(&mut content)?;
                Some(ContentHash::from_content(&content))
            }
            None => None,
        };

        // TODO: This should check p and parent are within the root.
        let parent = p
            .parent()
            .ok_or_else(|| anyhow!("Unable to find parent for {}", p.display()))?;
        self.fs.lock().unwrap().create_dir_all(parent)?;
        self.fs.lock().unwrap().writer(&p)?.write_all(value)?;

        self.content_store.lock().unwrap().store(value)?;

        // TODO: Need to read the previous content if it exists.
        Ok((original_hash, ContentHash::from_content(value)))
    }

    fn read_file(&self, path: &Path) -> anyhow::Result<Option<(ContentHash, Vec<u8>)>> {
        eprintln!("DummyBackend::read_file({:?})", path);
        let p = self.root.join(path);
        // Check if the file exists
        let original_and_hash = match self.fs.lock().unwrap().reader_if_exists(&p)? {
            Some(mut reader) => {
                let mut content = vec![];
                reader.read_to_end(&mut content)?;
                Some((
                    ContentHash::from_content(&content),
                    content
                ))
            },
            None => None,
        };

        Ok(original_and_hash)
    }
}

// ----------------
