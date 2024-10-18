use std::path::{Path, PathBuf};

use anyhow::bail;

use crate::{
    binary16::ContentHash,
    events::{Event, EventGroup, EventType, ReadFileEvent, WriteFileEvent},
};

pub trait EventLog {
    fn get_last_write_event(&self, p: &Path) -> anyhow::Result<Option<Event>>;
    fn get_file_history(&self, p: &Path) -> anyhow::Result<Vec<Event>>;
    fn get_event_group(&self, group_id: u64) -> anyhow::Result<Option<EventGroup>>;

    /// Input must have group_id and ids all set to zero.
    /// Returns the full group with id's correctly set.
    fn add_event_group(&mut self, group: &EventGroup) -> anyhow::Result<EventGroup>;
}

// --------

/// This is really the start of a SQLite event log
///
/// TODO: Maybe it belongs in it's own file?

pub struct SQLiteEventLog {
    conn: rusqlite::Connection,
}

impl SQLiteEventLog {
    pub fn open<P: AsRef<Path>>(path: P) -> anyhow::Result<SQLiteEventLog> {
        use rusqlite::OpenFlags;
        //TODO: Move the conn into the instance.
        let conn = rusqlite::Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_URI
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        Ok(SQLiteEventLog { conn })
    }
}

impl EventLog for SQLiteEventLog {
    fn get_last_write_event(&self, p: &Path) -> anyhow::Result<Option<Event>> {
        let mut stmt = self.conn.prepare("SELECT * FROM Events WHERE action_type='write' AND file_path=?1 ORDER BY id DESC LIMIT 1")?;
        let mut events = stmt.query([format!("{}", p.display())])?;
        let Some(event_row) = events.next()? else {
            return Ok(None);
        };
        let event = self.event_from_event_row(event_row)?;
        Ok(Some(event))
    }

    fn get_file_history(&self, p: &Path) -> anyhow::Result<Vec<Event>> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM Events WHERE file_path=?1 ORDER BY id DESC LIMIT 1")?;
        let mut events = stmt.query([format!("{}", p.display())])?;
        let mut result = vec![];
        while let Some(event_row) = events.next()? {
            let event = self.event_from_event_row(event_row)?;
            result.push(event);
        }
        Ok(result)
    }

    fn get_event_group(&self, group_id: u64) -> anyhow::Result<Option<EventGroup>> {
        // Read the group data
        let mut stmt = self.conn.prepare("SELECT * FROM Groups where id=?1")?;
        let mut groups = stmt.query([group_id])?;
        let Some(group_row) = groups.next()? else {
            return Ok(None);
        };
        let mut group = self.group_from_group_row(group_row)?;

        // Now actually read the events it contains
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM Events WHERE group_id=?1")?;
        let mut events = stmt.query([group_id])?;
        while let Some(event_row) = events.next()? {
            let event = self.event_from_event_row(event_row)?;
            group.events.push(event);
        }

        Ok(Some(group))
    }

    fn add_event_group(&mut self, group: &EventGroup) -> anyhow::Result<EventGroup> {
        // Create the group.
        let mut group = group.clone();

        self.conn.execute(
            "INSERT INTO Groups (command) VALUES (?1)",
            [group.command.clone()],
        )?;

        group.id = self.conn.last_insert_rowid() as u64;
        let mut stmt = self.conn.prepare("INSERT INTO Events (group_id, action_type, file_path, before_hash, after_hash) VALUES(?, ?, ?, ?, ?)")?;

        for event in &mut group.events {
            event.group_id = group.id;
            stmt.execute(self.row_from_event_no_id(event))?;
            event.id = self.conn.last_insert_rowid() as u64;
        }

        Ok(group)
    }
}

impl SQLiteEventLog {
    pub fn init<P: AsRef<Path>>(path: P) -> anyhow::Result<()> {
        // For now we swallow errors if we cant remove the file.
        if path.as_ref().exists() {
            bail!("event db file '{}' already exists", path.as_ref().display());
        }
        let conn = rusqlite::Connection::open(path)?;

        conn.execute(
            "create table Events (
                 id integer primary key,
                 group_id integer NOT NULL REFERENCES Groups(id),
                 action_type text NOT NULL,
                 file_path text,
                 before_hash text,
                 after_hash text
             )",
            (),
        )?;
        conn.execute(
            "create table Groups (
                 id integer primary key,
                 command text NOT NULL
             )",
            (),
        )?;
        Ok(())
    }

    fn event_from_event_row(&self, row: &rusqlite::Row) -> anyhow::Result<Event> {
        // Unpack the row
        let id: u64 = row.get("id")?;
        let group_id: u64 = row.get("group_id")?;
        let action_type: String = row.get("action_type")?;
        let event_type = match action_type.as_str() {
            "write" => {
                let file_path: String = row.get("file_path")?;
                let file_path = PathBuf::from(file_path);

                let before_hash: Option<String> = row.get("before_hash")?;
                let before_hash = match before_hash {
                    Some(s) => Some(ContentHash::from_string(&s)?),
                    None => None,
                };

                let after_hash: Option<String> = row.get("after_hash")?;
                let after_hash = match after_hash {
                    Some(s) => Some(ContentHash::from_string(&s)?),
                    None => None,
                };

                let write_file_event = WriteFileEvent {
                    path: file_path,
                    before_hash,
                    after_hash,
                };
                EventType::WriteFile(write_file_event)
            }
            "read" => {
                let file_path: String = row.get("file_path")?;
                let file_path = PathBuf::from(file_path);

                let before_hash: Option<String> = row.get("before_hash")?;
                let before_hash = match before_hash {
                    Some(s) => Some(ContentHash::from_string(&s)?),
                    None => None,
                };

                let read_file_event = ReadFileEvent {
                    path: file_path,
                    hash: before_hash,
                };
                EventType::ReadFile(read_file_event)
            }
            _ => {
                unreachable!("Invalid action_type='{}' encountered", action_type);
            }
        };

        Ok(Event {
            id,
            group_id,
            event_type,
        })
    }

    fn group_from_group_row(&self, row: &rusqlite::Row) -> anyhow::Result<EventGroup> {
        let command = row.get("command")?;
        // TODO: Fill in is_most_recent_run somehow?
        Ok(EventGroup {
            id: row.get("id")?,
            command,
            events: vec![],
            is_most_recent_run: true,
        })
    }

    // Order is group_id, action_type, file_path, before_hash, after_hash
    fn row_from_event_no_id(
        &self,
        event: &Event,
    ) -> (String, String, String, Option<String>, Option<String>) {
        // TODO: Make this work for more types
        match &event.event_type {
            // TODO: Fix the "???" values to use e.before_hash and e.after_hash
            EventType::WriteFile(e) => (
                event.group_id.to_string(),
                "write".to_string(),
                e.path.display().to_string(),
                e.before_hash.as_ref().map(|h| h.to_string()),
                e.after_hash.as_ref().map(|h| h.to_string()),
            ),
            EventType::ReadFile(e) => (
                event.group_id.to_string(),
                "read".to_string(),
                e.path.display().to_string(),
                e.hash.as_ref().map(|h| h.to_string()),
                None,
            ),
            EventType::GetMetadata(e) => (
                event.group_id.to_string(),
                "get_md".to_string(),
                e.path.display().to_string(),
                None,
                None,
            ),
            EventType::SetMetadata(e) => (
                event.group_id.to_string(),
                "set_md".to_string(),
                e.path.display().to_string(),
                None,
                None,
            ),
        }
    }
}

#[cfg(test)]
pub mod test_utils {
    use super::*;
    use mockall::mock;

    mock! {
        pub EventLog {}

        impl EventLog for EventLog {
            fn get_last_write_event(&self, p: &Path) -> anyhow::Result<Option<Event>>;
            fn get_file_history(&self, p: &Path) -> anyhow::Result<Vec<Event>>;
            fn get_event_group(&self, group_id: u64) -> anyhow::Result<Option<EventGroup>>;
            fn add_event_group(&mut self, group: &EventGroup) -> anyhow::Result<EventGroup>;
        }
    }
}

#[cfg(test)]
pub mod test {
    use std::path::PathBuf;

    use super::{test_utils::MockEventLog, EventLog};

    pub fn check_mocking_works() {
        let mut event_log = MockEventLog::default();
        event_log
            .expect_get_last_write_event()
            .returning(|_| Ok(None));

        assert_eq!(
            event_log
                .get_last_write_event(&PathBuf::from("hello"))
                .unwrap(),
            None
        );
    }
}
