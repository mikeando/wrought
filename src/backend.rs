use std::path::Path;

use crate::{
    binary16::ContentHash,
    metadata::{MetadataEntry, MetadataKey},
};

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

pub struct DummyBackend {}

impl Backend for DummyBackend {
    fn get_metadata(
        &self,
        path: &Path,
        key: &MetadataKey,
    ) -> anyhow::Result<Option<MetadataEntry>> {
        eprintln!("DummyBackend::get_metadata({:?}, {:?})", path, key);
        Ok(None)
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
        Ok(None)
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
        Ok((None, ContentHash::from_content(value)))
    }

    fn read_file(&self, path: &Path) -> anyhow::Result<Option<(ContentHash, Vec<u8>)>> {
        todo!()
    }
}

// ----------------
