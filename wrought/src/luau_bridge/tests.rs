#[cfg(test)]
mod tests {
    use crate::{
        bridge::Bridge,
        events::EventGroup,
        luau_bridge::{run_script, run_script_ex},
    };

    use anyhow::anyhow;
    use mlua::Lua;
    use mockall::{mock, predicate};
    use std::{
        path::{Path, PathBuf},
        sync::{Arc, Mutex},
    };

    mock! {
        pub Bridge {}

        impl Bridge for Bridge {
            fn write_file(&mut self, path: &Path, value: &[u8]) -> anyhow::Result<()>;
            fn read_file(&mut self, path: &Path) -> anyhow::Result<Option<Vec<u8>>>;
            fn get_metadata(&mut self, path: &Path, key: &str) -> anyhow::Result<Option<String>>;
            fn set_metadata(&mut self, path: &Path, key: &str, value: &str) -> anyhow::Result<()>;
            fn ai_query(&mut self, query: &str) -> anyhow::Result<String>;
            fn get_event_group(&self) -> Option<EventGroup>;
            fn ai_message(&mut self) -> anyhow::Result<crate::llm::AiMessage>;
            fn send_ai_message(&mut self, message:&crate::llm::AiMessage) -> anyhow::Result<serde_json::Value>;
        }
    }

    pub fn add_test_helpers(lua: &Lua, calls: Arc<Mutex<Vec<String>>>) -> anyhow::Result<()> {
        let globals = lua.globals();
        globals.set(
            "push_test_value",
            lua.create_function(move |_l, v: String| {
                calls.lock().unwrap().push(v);
                Ok(())
            })?,
        )?;
        Ok(())
    }

    #[test]
    pub fn can_report_lua_errors() {
        // i.e. do we get a sensible result back from a lua script calling error?
        // see https://www.lua.org/pil/8.3.html
        todo!();
    }

    #[test]
    pub fn can_lua_handle_bridge_errors() {
        // i.e. can a script use pcall style stuff to avoid crashing when an bridge function returns an error.
        //      though really they shouldn't in most cases, they instead return None - but maybe they will error
        //      in future if you try to access a path outside the project or a protected resourse or something like that?
        // see https://www.lua.org/pil/8.4.html
        todo!();
    }

    #[test]
    pub fn run_script_write_file() {
        let mut fs = xfs::mockfs::MockFS::new();

        fs.add_r(
            &PathBuf::from("somedir/script.luau"),
            br#"write_file("someplace/foo.txt", "some content")"#.to_vec(),
        )
        .unwrap();

        let mut mock_bridge = MockBridge::new();
        mock_bridge
            .expect_write_file()
            .with(
                predicate::eq(PathBuf::from("someplace/foo.txt")),
                predicate::eq(b"some content".to_vec()),
            )
            .returning(|_, _| Ok(()));

        let mock_bridge = Arc::new(Mutex::new(mock_bridge));
        let fs = Arc::new(Mutex::new(fs));

        run_script(
            mock_bridge.clone(),
            fs,
            &PathBuf::from("somedir/script.luau"),
        )
        .unwrap();

        mock_bridge.lock().unwrap().checkpoint();
    }

    #[test]
    pub fn run_script_write_file_invalid() {
        let mut fs = xfs::mockfs::MockFS::new();

        fs.add_r(
            &PathBuf::from("somedir/script.luau"),
            br#"write_file("someplace/foo.txt", "some content")"#.to_vec(),
        )
        .unwrap();

        let mut mock_bridge = MockBridge::new();
        mock_bridge
            .expect_write_file()
            .with(
                predicate::eq(PathBuf::from("someplace/foo.txt")),
                predicate::eq(b"some content".to_vec()),
            )
            .returning(|_, _| Err(anyhow!("Write Failure")));

        let mock_bridge = Arc::new(Mutex::new(mock_bridge));
        let fs = Arc::new(Mutex::new(fs));

        let result = run_script(
            mock_bridge.clone(),
            fs,
            &PathBuf::from("somedir/script.luau"),
        );
        assert!(result.is_err());

        mock_bridge.lock().unwrap().checkpoint();
    }

    #[test]
    pub fn run_script_read_file() {
        let mut fs = xfs::mockfs::MockFS::new();

        fs.add_r(
            &PathBuf::from("somedir/script.luau"),
            br#"content = read_file("someplace/foo.txt")"#.to_vec(),
        )
        .unwrap();

        let mut mock_bridge = MockBridge::new();
        mock_bridge
            .expect_read_file()
            .with(predicate::eq(PathBuf::from("someplace/foo.txt")))
            .returning(|_| Ok(Some(b"some content".to_vec())));

        let mock_bridge = Arc::new(Mutex::new(mock_bridge));
        let fs = Arc::new(Mutex::new(fs));

        run_script(
            mock_bridge.clone(),
            fs,
            &PathBuf::from("somedir/script.luau"),
        )
        .unwrap();

        // TODO: We should install a lua function so the content can be reported back to rust, so
        //       we can be sure that lua is seeing the same values as we expect.
        //       then the function would just get an extra line like `report_to_tests(content)`
        //       Tricky bit about this is working out how to hook it up to `run_script`
        //

        mock_bridge.lock().unwrap().checkpoint();
    }

    #[test]
    pub fn run_script_read_empty() {
        let mut fs = xfs::mockfs::MockFS::new();

        fs.add_r(
            &PathBuf::from("somedir/script.luau"),
            br#"content = read_file("someplace/foo.txt")"#.to_vec(),
        )
        .unwrap();

        let mut mock_bridge = MockBridge::new();
        mock_bridge
            .expect_read_file()
            .with(predicate::eq(PathBuf::from("someplace/foo.txt")))
            .returning(|_| Ok(None));

        let mock_bridge = Arc::new(Mutex::new(mock_bridge));
        let fs = Arc::new(Mutex::new(fs));

        run_script(
            mock_bridge.clone(),
            fs,
            &PathBuf::from("somedir/script.luau"),
        )
        .unwrap();

        mock_bridge.lock().unwrap().checkpoint();
    }

    #[test]
    pub fn run_script_read_error() {
        let mut fs = xfs::mockfs::MockFS::new();

        fs.add_r(
            &PathBuf::from("somedir/script.luau"),
            br#"content = read_file("someplace/foo.txt")"#.to_vec(),
        )
        .unwrap();

        let mut mock_bridge = MockBridge::new();
        mock_bridge
            .expect_read_file()
            .with(predicate::eq(PathBuf::from("someplace/foo.txt")))
            .returning(|_| Err(anyhow!("Read Failure")));

        let mock_bridge = Arc::new(Mutex::new(mock_bridge));
        let fs = Arc::new(Mutex::new(fs));

        let result = run_script(
            mock_bridge.clone(),
            fs,
            &PathBuf::from("somedir/script.luau"),
        );
        assert!(result.is_err());

        mock_bridge.lock().unwrap().checkpoint();
    }

    #[test]
    pub fn run_script_set_metadata() {
        todo!();
    }

    #[test]
    pub fn run_script_get_metadata() {
        todo!();
    }

    #[test]
    pub fn make_ai_query() {
        let mut fs = xfs::mockfs::MockFS::new();

        fs.add_r(
            &PathBuf::from("somedir/script.luau"),
            vec![
                r#"content = ai_query("Tell me a fun story")"#,
                r#"push_test_value(content)"#,
            ]
            .join("\n")
            .as_bytes()
            .to_vec(),
        )
        .unwrap();

        let mut mock_bridge = MockBridge::new();
        mock_bridge
            .expect_ai_query()
            .with(predicate::eq("Tell me a fun story".to_string()))
            .returning(|_| Ok("There once was a fish".to_string()));

        let mock_bridge = Arc::new(Mutex::new(mock_bridge));
        let fs = Arc::new(Mutex::new(fs));

        let test_values = Arc::new(Mutex::new(vec![]));
        let test_values_copy = test_values.clone();
        let result = run_script_ex(
            mock_bridge.clone(),
            fs,
            &PathBuf::from("somedir/script.luau"),
            |l| add_test_helpers(l, test_values_copy),
        );
        eprintln!("{:?}", result);
        assert!(result.is_ok());
        assert_eq!(
            test_values.lock().unwrap().clone(),
            vec!["There once was a fish"]
        );

        mock_bridge.lock().unwrap().checkpoint();
    }

    #[test]
    pub fn make_ai_query_error() {
        let mut fs = xfs::mockfs::MockFS::new();

        fs.add_r(
            &PathBuf::from("somedir/script.luau"),
            vec![
                r#"content = ai_query("Tell me a fun story")"#,
                r#"push_test_value(content)"#,
            ]
            .join("\n")
            .as_bytes()
            .to_vec(),
        )
        .unwrap();

        let mut mock_bridge = MockBridge::new();
        mock_bridge
            .expect_ai_query()
            .with(predicate::eq("Tell me a fun story".to_string()))
            .returning(|_| Err(anyhow!("Network is tofu")));

        let mock_bridge = Arc::new(Mutex::new(mock_bridge));
        let fs = Arc::new(Mutex::new(fs));

        let test_values = Arc::new(Mutex::new(vec![]));
        let test_values_copy = test_values.clone();
        let result = run_script_ex(
            mock_bridge.clone(),
            fs,
            &PathBuf::from("somedir/script.luau"),
            |l| add_test_helpers(l, test_values_copy),
        );
        assert!(result.is_err());
        assert!(test_values.lock().unwrap().is_empty());

        mock_bridge.lock().unwrap().checkpoint();
    }
}
