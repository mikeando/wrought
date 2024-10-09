use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

use mlua::prelude::*;

use crate::bridge::Bridge;

// pub fn lua_print(_lua: &Lua, vals: MultiValue) -> mlua::Result<()> {
//     println!(
//         "Lua: {}",
//         vals.iter()
//             .map(|v| v.to_string())
//             .collect::<Result<Vec<_>, _>>()
//             .map(|v| v.join(" "))
//             .unwrap()
//     );
//     Ok(())
// }

pub fn convert_error(e: anyhow::Error) -> mlua::Error {
    mlua::Error::runtime(format!("{}", e))
}

pub fn lua_write_file(
    bridge: &Arc<Mutex<dyn Bridge>>,
    _lua: &Lua,
    (file_name, value): (String, String),
) -> anyhow::Result<()> {
    bridge
        .lock()
        .unwrap()
        .write_file(&PathBuf::from(file_name), value.as_bytes())?;
    Ok(())
}

pub fn lua_read_file(
    bridge: &Arc<Mutex<dyn Bridge>>,
    _lua: &Lua,
    file_name: String,
) -> anyhow::Result<Option<String>> {
    eprintln!("in lua_read_file...");
    let result = bridge
        .lock()
        .unwrap()
        .read_file(&PathBuf::from(file_name))?;
    let Some(result) = result else {
        return Ok(None);
    };
    let result = String::from_utf8(result)?;
    Ok(Some(result))
}

pub fn lua_get_metadata(
    bridge: &Arc<Mutex<dyn Bridge>>,
    _lua: &Lua,
    (file_name, key): (String, String),
) -> anyhow::Result<Option<String>> {
    eprintln!("In luad_get_metadata...");
    let result = bridge
        .lock()
        .unwrap()
        .get_metadata(&PathBuf::from(file_name), &key)?;
    Ok(result)
}

pub fn lua_set_metadata(
    bridge: &Arc<Mutex<dyn Bridge>>,
    _lua: &Lua,
    (file_name, key, value): (String, String, String),
) -> anyhow::Result<()> {
    eprintln!("In lua_set_metadata...");
    bridge
        .lock()
        .unwrap()
        .set_metadata(&PathBuf::from(file_name), &key, &value)?;
    Ok(())
}

pub fn lua_ai_query(
    bridge: &Arc<Mutex<dyn Bridge>>,
    _lua: &Lua, 
    query: String) -> anyhow::Result<String> {
    bridge.lock().unwrap().ai_query(&query)
}


fn add_bridge_function<'lua, F, A, R>(
    bridge: &Arc<Mutex<dyn Bridge>>,
    lua: &'lua Lua,
    name: &str,
    f: F,
) -> anyhow::Result<()>
where
    F: Fn(&Arc<Mutex<dyn Bridge>>, &Lua, A) -> anyhow::Result<R> + 'static,
    A: FromLuaMulti<'lua>,
    R: IntoLuaMulti<'lua>,
{
    let be = bridge.clone();
    let globals = lua.globals();
    globals.set(
        name,
        lua.create_function(move |l, v| f(&be, l, v).map_err(convert_error))?,
    )?;
    Ok(())
}

pub fn run_script(
    bridge: Arc<Mutex<dyn Bridge>>,
    fs: Arc<Mutex<dyn xfs::Xfs>>,
    script_path: &Path,
) -> anyhow::Result<()> {
    run_script_ex(bridge, fs, script_path, |_|Ok(()))
}

// The additional F function is used to add hooks when testing
pub fn run_script_ex<F>(
    bridge: Arc<Mutex<dyn Bridge>>,
    fs: Arc<Mutex<dyn xfs::Xfs>>,
    script_path: &Path,
    f: F,
) -> anyhow::Result<()>
where 
    F: FnOnce(&Lua) -> anyhow::Result<()>
{
    let lua = Lua::new();

    lua.sandbox(true)?;

    // Replace print with our own function.
    // let globals = lua.globals();
    // let print = lua.create_function(lua_print)?;
    // globals.set("print", print)?;
    add_bridge_function(&bridge, &lua, "write_file", lua_write_file)?;
    add_bridge_function(&bridge, &lua, "read_file", lua_read_file)?;
    add_bridge_function(&bridge, &lua, "set_metadata", lua_set_metadata)?;
    add_bridge_function(&bridge, &lua, "get_metadata", lua_get_metadata)?;
    add_bridge_function(&bridge, &lua, "ai_query", lua_ai_query)?;

    f(&lua)?;

    let mut script = String::new();
    fs.lock()
        .unwrap()
        .reader(script_path)?
        .read_to_string(&mut script)?;

    lua.load(script).exec()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::anyhow;
    use std::sync::{Arc, Mutex};

    pub struct MockBridge {
        write_calls: Vec<(PathBuf, Vec<u8>, anyhow::Result<()>)>,
        read_calls: Vec<(PathBuf, anyhow::Result<Option<Vec<u8>>>)>,
        ai_query_calls: Vec<(String, anyhow::Result<String>)>,
        errors: Vec<String>,
    }

    impl MockBridge {
        pub fn new() -> MockBridge {
            MockBridge {
                write_calls: vec![],
                read_calls: vec![],
                ai_query_calls: vec![],
                errors: vec![],
            }
        }

        pub fn expect_write_file<P: Into<PathBuf>>(
            &mut self,
            path: P,
            value: &[u8],
            result: anyhow::Result<()>,
        ) {
            self.write_calls.push((path.into(), value.to_vec(), result))
        }

        pub fn expect_read_file<P: Into<PathBuf>>(
            &mut self,
            path: P,
            result: anyhow::Result<Option<Vec<u8>>>,
        ) {
            self.read_calls.push((path.into(), result))
        }

        pub fn expect_ai_query<Q: Into<String>>( 
            &mut self, 
            query: Q, 
            result: anyhow::Result<String>,
        ) {
            self.ai_query_calls.push((query.into(), result))
        }

        pub fn check(&self) {
            assert!(self.write_calls.is_empty());
            assert!(self.read_calls.is_empty());
            assert!(self.ai_query_calls.is_empty());
            if !self.errors.is_empty() {
                panic!("Unexpected errors in mock: {:?}", self.errors);
            }
        }
    }

    impl Bridge for MockBridge {
        fn write_file(&mut self, path: &Path, value: &[u8]) -> anyhow::Result<()> {
            // Impolite to panic inside luau, so instead we error and add a failure message to the mock.
            let Some(expected) = self.write_calls.pop() else {
                self.errors
                    .push(format!("Call to write_file when expected calls is empty"));
                return Err(anyhow!("Call to write_file when expected calls is empty"));
            };
            if path != expected.0 || value != expected.1 {
                self.errors
                    .push(format!("Call to write_file does not match expected"));
                return Err(anyhow!("Call to write_file does not match expected"));
            }
            expected.2
        }

        fn read_file(&mut self, path: &Path) -> anyhow::Result<Option<Vec<u8>>> {
            // Impolite to panic inside luau, so instead we error and add a failure message to the mock.
            let Some(expected) = self.read_calls.pop() else {
                self.errors
                    .push(format!("Call to read_file when expected calls is empty"));
                return Err(anyhow!("Call to read_file when expected calls is empty"));
            };
            if path != expected.0 {
                self.errors
                    .push(format!("Call to read_file does not match expected"));
                return Err(anyhow!("Call to read_file does not match expected"));
            }
            expected.1
        }

        fn get_metadata(&mut self, path: &Path, key: &str) -> anyhow::Result<Option<String>> {
            todo!()
        }

        fn set_metadata(&mut self, path: &Path, key: &str, value: &str) -> anyhow::Result<()> {
            todo!()
        }

        fn get_event_group(&self) -> Option<crate::events::EventGroup> {
            todo!()
        }
        
        fn ai_query(&mut self, query: &str) -> anyhow::Result<String> {
            // Impolite to panic inside luau, so instead we error and add a failure message to the mock.
            let Some(expected) = self.ai_query_calls.pop() else {
                self.errors
                    .push(format!("Call to ai_query when expected calls is empty"));
                return Err(anyhow!("Call to ai_query when expected calls is empty"));
            };
            if query != expected.0 {
                self.errors
                    .push(format!("Call to read_file does not match expected"));
                return Err(anyhow!("Call to read_file does not match expected"));
            }
            expected.1
        }
        
    }


    pub fn add_test_helpers(lua: &Lua, calls: Arc<Mutex<Vec<String>>>) -> anyhow::Result<()> 
    {
        let globals = lua.globals();
        globals.set(
            "push_test_value",
            lua.create_function( move|_l, v: String| {
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
        mock_bridge.expect_write_file("someplace/foo.txt", b"some content", Ok(()));

        let mock_bridge = Arc::new(Mutex::new(mock_bridge));
        let fs = Arc::new(Mutex::new(fs));

        run_script(
            mock_bridge.clone(),
            fs,
            &PathBuf::from("somedir/script.luau"),
        )
        .unwrap();

        mock_bridge.lock().unwrap().check();
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
        mock_bridge.expect_write_file(
            "someplace/foo.txt",
            b"some content",
            Err(anyhow!("Write Failure")),
        );

        let mock_bridge = Arc::new(Mutex::new(mock_bridge));
        let fs = Arc::new(Mutex::new(fs));

        let result = run_script(
            mock_bridge.clone(),
            fs,
            &PathBuf::from("somedir/script.luau"),
        );
        assert!(result.is_err());

        mock_bridge.lock().unwrap().check();
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
        mock_bridge.expect_read_file("someplace/foo.txt", Ok(Some(b"some content".to_vec())));

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

        mock_bridge.lock().unwrap().check();
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
        mock_bridge.expect_read_file("someplace/foo.txt", Ok(None));

        let mock_bridge = Arc::new(Mutex::new(mock_bridge));
        let fs = Arc::new(Mutex::new(fs));

        run_script(
            mock_bridge.clone(),
            fs,
            &PathBuf::from("somedir/script.luau"),
        )
        .unwrap();

        mock_bridge.lock().unwrap().check();
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
        mock_bridge.expect_read_file("someplace/foo.txt", Err(anyhow!("Read Failure")));

        let mock_bridge = Arc::new(Mutex::new(mock_bridge));
        let fs = Arc::new(Mutex::new(fs));

        let result = run_script(
            mock_bridge.clone(),
            fs,
            &PathBuf::from("somedir/script.luau"),
        );
        assert!(result.is_err());

        mock_bridge.lock().unwrap().check();
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
            vec![r#"content = ai_query("Tell me a fun story")"#,
            r#"push_test_value(content)"#].join("\n").as_bytes().to_vec(),
        )
        .unwrap();

        let mut mock_bridge = MockBridge::new();
        mock_bridge.expect_ai_query("Tell me a fun story", Ok("There once was a fish".to_string()));

        let mock_bridge = Arc::new(Mutex::new(mock_bridge));
        let fs = Arc::new(Mutex::new(fs));

        let test_values = Arc::new(Mutex::new(vec![]));
        let test_values_copy = test_values.clone();
        let result = run_script_ex(
            mock_bridge.clone(),
            fs,
            &PathBuf::from("somedir/script.luau"),
            |l| add_test_helpers(l, test_values_copy)
        );
        eprintln!("{:?}", result);
        assert!(result.is_ok());
        assert_eq!(test_values.lock().unwrap().clone(), vec!["There once was a fish"]);

        mock_bridge.lock().unwrap().check();
    }


    #[test]
    pub fn make_ai_query_error() {
        let mut fs = xfs::mockfs::MockFS::new();

        fs.add_r(
            &PathBuf::from("somedir/script.luau"),
            vec![r#"content = ai_query("Tell me a fun story")"#,
            r#"push_test_value(content)"#].join("\n").as_bytes().to_vec(),
        )
        .unwrap();

        let mut mock_bridge = MockBridge::new();
        mock_bridge.expect_ai_query("Tell me a fun story", Err(anyhow!("Network is tofu")));

        let mock_bridge = Arc::new(Mutex::new(mock_bridge));
        let fs = Arc::new(Mutex::new(fs));

        let test_values = Arc::new(Mutex::new(vec![]));
        let test_values_copy = test_values.clone();
        let result = run_script_ex(
            mock_bridge.clone(),
            fs,
            &PathBuf::from("somedir/script.luau"),
            |l| add_test_helpers(l, test_values_copy)
        );
        assert!(result.is_err());
        assert!(test_values.lock().unwrap().is_empty());

        mock_bridge.lock().unwrap().check();
    }
}
