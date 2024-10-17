use std::{
    path::Path,
    sync::{Arc, Mutex},
};

use crate::{binary16::ContentHash, event_log::EventLog, events::EventType};

#[derive(Debug, PartialEq)]
pub struct EventLogCommand(pub String);

#[derive(Debug, PartialEq)]
pub enum FileHistoryEntry {
    Deleted,
    DeletedBy(EventLogCommand),
    UnknownHash(ContentHash),
    StoredHash(ContentHash, EventLogCommand),
    LocalChanges(ContentHash),
}

pub fn file_history(
    fs: Arc<Mutex<dyn xfs::Xfs>>,
    event_log: Arc<Mutex<dyn EventLog>>,
    project_root: &Path,
    file_path: &Path,
) -> anyhow::Result<Vec<FileHistoryEntry>> {
    let mut entries = vec![];
    let events = event_log.lock().unwrap().get_file_history(file_path)?;
    let mut last_write_hash = None;
    for e in events {
        match e.event_type {
            EventType::WriteFile(write_file_event) => {
                if write_file_event.before_hash != last_write_hash {
                    if let Some(hash) = write_file_event.before_hash {
                        entries.push(FileHistoryEntry::UnknownHash(hash));
                    } else {
                        entries.push(FileHistoryEntry::Deleted);
                    }
                }
                let group = event_log.lock().unwrap().get_event_group(e.group_id)?;
                if let Some(hash) = &write_file_event.after_hash {
                    entries.push(FileHistoryEntry::StoredHash(
                        hash.clone(),
                        EventLogCommand(group.unwrap().command),
                    ));
                } else {
                    entries.push(FileHistoryEntry::DeletedBy(EventLogCommand(
                        group.unwrap().command,
                    )));
                }
                last_write_hash = write_file_event.after_hash;
            }
            EventType::ReadFile(_read_file_event) => {}
            EventType::GetMetadata(_get_metadata_event) => {}
            EventType::SetMetadata(set_metadata_event) => eprint!("{:?}", set_metadata_event),
        }
    }
    // Now check the actual file
    let cur_hash = if let Some(mut reader) = fs
        .lock()
        .unwrap()
        .reader_if_exists(&project_root.join(file_path))?
    {
        let mut buf = vec![];
        reader.read_to_end(&mut buf)?;
        Some(ContentHash::from_content(&buf))
    } else {
        None
    };
    if cur_hash != last_write_hash {
        if let Some(hash) = cur_hash {
            entries.push(FileHistoryEntry::LocalChanges(hash));
        } else {
            entries.push(FileHistoryEntry::Deleted)
        }
    }
    Ok(entries)
}

#[cfg(test)]
pub mod test {
    use std::{
        io::Cursor,
        path::PathBuf,
        sync::{Arc, Mutex},
    };

    use mockall::{mock, predicate};

    use crate::{
        binary16::ContentHash,
        event_log::test_utils::MockEventLog,
        events::{Event, EventGroup, WriteFileEvent},
        file_history::{EventLogCommand, FileHistoryEntry},
    };

    use super::file_history;

    //TODO: Move this somewhere we can reuse it.
    mock! {

        pub Fs {}

        impl xfs::Xfs for Fs {
            fn on_each_entry(
                &self,
                p: &std::path::Path,
                f: &mut dyn FnMut(&dyn xfs::Xfs, &dyn xfs::XfsDirEntry) -> anyhow::Result<()>,
            ) -> anyhow::Result<()>;

            fn on_each_entry_mut(
                &mut self,
                p: &std::path::Path,
                f: &mut dyn FnMut(&mut dyn xfs::Xfs, &dyn xfs::XfsDirEntry) -> anyhow::Result<()>,
            ) -> anyhow::Result<()>;

            fn reader(&self, p: &std::path::Path) -> xfs::Result<Box<dyn std::io::Read>>;
            fn reader_if_exists(&self, p: &std::path::Path) -> xfs::Result<Option<Box<dyn std::io::Read>>>;
            fn writer(&mut self, p: &std::path::Path) -> xfs::Result<Box<dyn std::io::Write>>;
            fn create_dir(&mut self, p: &std::path::Path) -> xfs::Result<()>;
            fn create_dir_all(&mut self, p: &std::path::Path) -> xfs::Result<()>;
            fn read_all_lines(&self, p: &std::path::Path) -> xfs::Result<Vec<String>>;
            fn metadata(&self, p: &std::path::Path) -> xfs::Result<Box<dyn xfs::XfsMetadata>>;
            fn tree(&self) -> String;
            fn canonicalize(&self, p: &std::path::Path) -> xfs::Result<std::path::PathBuf>;
            fn copy(&mut self, src_path: &std::path::Path, dst_path: &std::path::Path) -> xfs::Result<()>;
            fn is_dir(&self, p: &std::path::Path) -> bool;
            fn is_file(&self, p: &std::path::Path) -> bool;
            fn exists(&self, p: &std::path::Path) -> bool;
        }

    }

    impl MockFs {
        pub fn with_read<P: Into<PathBuf>, B: Into<Vec<u8>>>(&mut self, path: P, content: B) {
            let content = Box::new(Cursor::new(content.into()));
            self.expect_reader_if_exists()
                .with(predicate::eq(path.into()))
                .returning(move |_| Ok(Some(content.clone())));
        }

        pub fn with_missing_read<P: Into<PathBuf>>(&mut self, path: P) {
            self.expect_reader_if_exists()
                .with(predicate::eq(path.into()))
                .returning(move |_| Ok(None));
        }
    }

    #[test]
    pub fn untracked_nonexistant_file() {
        todo!()
    }

    #[test]
    pub fn normal_file() {
        let mut fs = MockFs::default();
        let mut event_log = MockEventLog::default();

        let project_root = PathBuf::from("project_root");
        let file_path = PathBuf::from("tofu.txt");

        let file_original_content = b"This is a test";
        let file_local_chages_content = b"Hello World";
        fs.with_read(project_root.join(&file_path), file_local_chages_content);

        let mock_events: Vec<Event> = vec![Event::from(WriteFileEvent {
            path: project_root.join(&file_path),
            before_hash: None,
            after_hash: Some(ContentHash::from_content(file_original_content)),
        })
        .with_group_id(12)];

        let event_group = EventGroup {
            id: 12,
            command: "dancing".to_string(),
            events: vec![],
            is_most_recent_run: false,
        };

        event_log
            .expect_get_file_history()
            .with(predicate::eq(file_path.clone()))
            .returning(move |_| Ok(mock_events.clone()));
        event_log
            .expect_get_event_group()
            .with(predicate::eq(12u64))
            .returning(move |_| Ok(Some(event_group.clone())));

        let fs = Arc::new(Mutex::new(fs));
        let event_log = Arc::new(Mutex::new(event_log));
        let history =
            file_history(fs.clone(), event_log.clone(), &project_root, &file_path).unwrap();

        assert_eq!(
            history,
            vec![
                FileHistoryEntry::StoredHash(
                    ContentHash::from_content(file_original_content),
                    EventLogCommand("dancing".to_string())
                ),
                FileHistoryEntry::LocalChanges(ContentHash::from_content(
                    file_local_chages_content
                ))
            ]
        );

        fs.lock().unwrap().checkpoint();
        event_log.lock().unwrap().checkpoint();
    }

    #[test]
    pub fn removed_file() {
        let mut fs = MockFs::default();
        let mut event_log = MockEventLog::default();

        let project_root = PathBuf::from("project_root");
        let file_path = PathBuf::from("tofu.txt");

        let file_original_content = b"This is a test";
        fs.with_missing_read(project_root.join(&file_path));

        let mock_events: Vec<Event> = vec![Event::from(WriteFileEvent {
            path: project_root.join(&file_path),
            before_hash: None,
            after_hash: Some(ContentHash::from_content(file_original_content)),
        })
        .with_group_id(12)];

        let event_group = EventGroup {
            id: 12,
            command: "dancing".to_string(),
            events: vec![],
            is_most_recent_run: false,
        };

        event_log
            .expect_get_file_history()
            .with(predicate::eq(file_path.clone()))
            .returning(move |_| Ok(mock_events.clone()));
        event_log
            .expect_get_event_group()
            .with(predicate::eq(12u64))
            .returning(move |_| Ok(Some(event_group.clone())));

        let fs = Arc::new(Mutex::new(fs));
        let event_log = Arc::new(Mutex::new(event_log));
        let history =
            file_history(fs.clone(), event_log.clone(), &project_root, &file_path).unwrap();

        assert_eq!(
            history,
            vec![
                FileHistoryEntry::StoredHash(
                    ContentHash::from_content(file_original_content),
                    EventLogCommand("dancing".to_string())
                ),
                FileHistoryEntry::Deleted,
            ]
        );

        fs.lock().unwrap().checkpoint();
        event_log.lock().unwrap().checkpoint();
    }

    #[test]
    pub fn called_on_directory() {
        todo!();
    }

    #[test]
    pub fn file_became_directory() {
        todo!();
    }

    #[test]
    pub fn handles_filesystem_error() {
        todo!();
    }

    #[test]
    pub fn handles_event_log_errors() {
        todo!();
    }
}
