use std::collections::BTreeMap;
use std::fmt;
use std::future::Future;
use std::path::Path;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;

use mlua::prelude::*;
use mlua::{Lua, Value as LuaValue};
use serde_json::json;
use serde_json::{Number, Value as JsonValue};

use crate::bridge::Bridge;
type AsyncMutex<T> = tokio::sync::Mutex<T>;

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

pub fn lua_write_file<'lua>(
    bridge: Arc<AsyncMutex<dyn Bridge>>,
    _lua: &'lua Lua,
    (file_name, value): (String, String),
) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + 'lua>> {
    Box::pin(async move {
        bridge
            .lock()
            .await
            .write_file(&PathBuf::from(file_name), value.as_bytes())?;
        Ok(())
    })
}

pub fn lua_read_file<'lua>(
    bridge: Arc<AsyncMutex<dyn Bridge>>,
    _lua: &'lua Lua,
    file_name: String,
) -> Pin<Box<dyn Future<Output = anyhow::Result<Option<String>>> + 'lua>> {
    Box::pin(async move {
        let result = bridge.lock().await.read_file(&PathBuf::from(file_name))?;
        let Some(result) = result else {
            return Ok(None);
        };
        let result = String::from_utf8(result)?;
        Ok(Some(result))
    })
}

pub fn lua_get_metadata<'lua>(
    bridge: Arc<AsyncMutex<dyn Bridge>>,
    _lua: &'lua Lua,
    (file_name, key): (String, String),
) -> Pin<Box<dyn Future<Output = anyhow::Result<Option<String>>> + 'lua>> {
    Box::pin(async move {
        let result = bridge
            .lock()
            .await
            .get_metadata(&PathBuf::from(file_name), &key)?;
        Ok(result)
    })
}

pub fn lua_set_metadata<'lua>(
    bridge: Arc<AsyncMutex<dyn Bridge>>,
    _lua: &'lua Lua,
    (file_name, key, value): (String, String, String),
) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + 'lua>> {
    Box::pin(async move {
        bridge
            .lock()
            .await
            .set_metadata(&PathBuf::from(file_name), &key, &value)?;
        Ok(())
    })
}

pub fn lua_ai_query<'lua>(
    bridge: Arc<AsyncMutex<dyn Bridge>>,
    _lua: &'lua Lua,
    query: String,
) -> Pin<Box<dyn Future<Output = anyhow::Result<String>> + 'lua>> {
    Box::pin(async move { bridge.lock().await.ai_query(&query).await })
}

struct LuaTemplater {
    tera: tera::Tera,
}

#[derive(Debug)]
pub enum ConversionError {
    UnsupportedType(String),
    InvalidTableKey(String),
    InvalidNumber(String),
    LuaError(mlua::Error),
    NonStringKeyInObject,
    MixedArrayKeys,
}

impl ConversionError {
    pub fn unsupported_type(typ: impl ToString) -> Self {
        Self::UnsupportedType(typ.to_string())
    }

    pub fn invalid_table_key(key: impl ToString) -> Self {
        Self::InvalidTableKey(key.to_string())
    }

    pub fn invalid_number(num: impl ToString) -> Self {
        Self::InvalidNumber(num.to_string())
    }

    pub fn lua_error(err: mlua::Error) -> Self {
        Self::LuaError(err)
    }
}

impl fmt::Display for ConversionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedType(t) => write!(f, "Unsupported type: {}", t),
            Self::InvalidTableKey(k) => write!(f, "Invalid table key: {}", k),
            Self::InvalidNumber(n) => write!(f, "Invalid number: {}", n),
            Self::LuaError(e) => write!(f, "Lua error: {}", e),
            Self::NonStringKeyInObject => write!(f, "Non-string key in object"),
            Self::MixedArrayKeys => write!(f, "Mixed array keys"),
        }
    }
}

impl std::error::Error for ConversionError {}

impl From<mlua::Error> for ConversionError {
    fn from(err: mlua::Error) -> Self {
        Self::lua_error(err)
    }
}

// Now we can update the conversion functions to use these constructors:

fn lua_value_to_json_value(
    v: LuaValue,
    empty_table_is_array: bool,
) -> Result<JsonValue, ConversionError> {
    match v {
        LuaValue::Nil => Ok(JsonValue::Null),
        LuaValue::Boolean(b) => Ok(JsonValue::Bool(b)),
        LuaValue::Integer(i) => Ok(JsonValue::Number(Number::from(i))),
        LuaValue::Number(n) => {
            if !n.is_finite() {
                return Err(ConversionError::invalid_number(n));
            }
            Ok(JsonValue::Number(
                Number::from_f64(n).ok_or_else(|| ConversionError::invalid_number(n))?,
            ))
        }
        LuaValue::String(s) => Ok(JsonValue::String(s.to_str()?.to_string())),
        LuaValue::Table(table) => lua_table_to_json(table, empty_table_is_array),
        LuaValue::LightUserData(_) => Err(ConversionError::unsupported_type("LightUserData")),
        LuaValue::Function(_) => Err(ConversionError::unsupported_type("Function")),
        LuaValue::Thread(_) => Err(ConversionError::unsupported_type("Thread")),
        LuaValue::UserData(_) => Err(ConversionError::unsupported_type("UserData")),
        LuaValue::Error(_) => Err(ConversionError::unsupported_type("Error")),
        _ => Err(ConversionError::unsupported_type("Unknown")),
    }
}

fn json_value_to_lua_value<'a>(
    lua: &'a Lua,
    value: &JsonValue,
) -> Result<LuaValue<'a>, ConversionError> {
    match value {
        JsonValue::Null => Ok(LuaValue::Nil),
        JsonValue::Bool(b) => Ok(LuaValue::Boolean(*b)),
        JsonValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(LuaValue::Integer(i as i32))
            } else if let Some(f) = n.as_f64() {
                if f.is_finite() {
                    Ok(LuaValue::Number(f))
                } else {
                    Err(ConversionError::invalid_number(f))
                }
            } else {
                Err(ConversionError::invalid_number(n))
            }
        }
        JsonValue::String(s) => Ok(LuaValue::String(lua.create_string(s)?)),
        JsonValue::Array(arr) => {
            let table = lua.create_table_with_capacity(arr.len(), 0)?;
            for (i, value) in arr.iter().enumerate() {
                table.set(i + 1, json_value_to_lua_value(lua, value)?)?;
            }
            Ok(LuaValue::Table(table))
        }
        JsonValue::Object(obj) => {
            let table = lua.create_table_with_capacity(0, obj.len())?;
            for (key, value) in obj {
                table.set(key.clone(), json_value_to_lua_value(lua, value)?)?;
            }
            Ok(LuaValue::Table(table))
        }
    }
}

fn to_array_index(key: &LuaValue) -> Option<usize> {
    match key {
        LuaValue::Integer(i) if *i > 0 => Some(*i as usize),
        LuaValue::Number(n) if n.is_finite() && n.fract() == 0.0 && *n > 0.0 => Some(*n as usize),
        _ => None,
    }
}

fn lua_table_to_json(
    table: mlua::Table,
    empty_table_is_array: bool,
) -> Result<JsonValue, ConversionError> {
    let len = table.len()? as usize;

    eprintln!("lua_table_to_json: table.len={}", len);

    let mut is_array = true;
    let mut is_object = true;

    let mut array = vec![];
    let mut object = BTreeMap::new();

    let mut is_empty = true;
    for pair in table.pairs::<LuaValue, LuaValue>() {
        is_empty = false;
        let (key, value) = pair?;

        if let Some(index) = to_array_index(&key) {
            is_object = false;
            if !is_array {
                break;
            }
            // Ensure the vector is long enough
            let i = index - 1;
            if i + 1 > array.len() {
                let n = i + 1 - array.len();
                for _ in 0..n {
                    array.push(LuaValue::Nil);
                }
            }
            // Now we can update the value
            array[i] = value;
            // TODO: Should we have a maximum gap size?
        } else {
            is_array = false;
            if !is_object {
                break;
            }
            if let LuaValue::String(s) = key {
                let ss = s.to_str()?;
                object.insert(ss.to_string(), value);
            } else {
                is_object = false;
                break;
            }
        }
    }

    if is_empty {
        if empty_table_is_array {
            return Ok(json!([]));
        } else {
            return Ok(json!({}));
        }
    }

    if !is_array && !is_object {
        //TODO: Add more details
        return Err(ConversionError::invalid_table_key("invalid table key"));
    }

    if is_array {
        let mut json_array = Vec::with_capacity(array.len());
        for value in array {
            json_array.push(lua_value_to_json_value(value, empty_table_is_array)?);
        }
        Ok(JsonValue::Array(json_array))
    } else {
        let mut map = serde_json::Map::with_capacity(object.len());
        for (key, value) in object {
            map.insert(key, lua_value_to_json_value(value, empty_table_is_array)?);
        }
        Ok(JsonValue::Object(map))
    }
}

impl LuaTemplater {
    pub fn add_template(&mut self, key: String, value: String) -> anyhow::Result<()> {
        self.tera.add_raw_template(&key, &value)?;
        Ok(())
    }
    pub fn render_template(&self, key: String, table: mlua::Table) -> anyhow::Result<String> {
        let value = lua_table_to_json(table, true)?;
        let context = tera::Context::from_value(value)?;
        Ok(self.tera.render(&key, &context)?)
    }
}

impl LuaUserData for LuaTemplater {
    fn add_methods<'lua, M: LuaUserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_method_mut("add_template", |_, this, (key, value): (String, String)| {
            this.add_template(key, value).map_err(convert_error)
        });
        methods.add_method(
            "render_template",
            |_, this, (key, context): (String, mlua::Table)| {
                this.render_template(key, context).map_err(convert_error)
            },
        );
    }
}

fn lua_template<'lua>(
    bridge: Arc<AsyncMutex<dyn Bridge>>,
    _lua: &'lua Lua,
    _params: (),
) -> Pin<Box<dyn Future<Output = anyhow::Result<LuaTemplater>> + 'lua>> {
    Box::pin(async move {
        Ok(LuaTemplater {
            tera: tera::Tera::default(),
        })
    })
}

fn add_bridge_function<'lua, F, A, R>(
    bridge: Arc<AsyncMutex<dyn Bridge>>,
    lua: &'lua Lua,
    name: &str,
    f: F,
) -> anyhow::Result<()>
where
    F: for<'a> Fn(
            Arc<AsyncMutex<dyn Bridge>>,
            &'a Lua,
            A,
        ) -> Pin<Box<dyn Future<Output = anyhow::Result<R>> + 'a>>
        + Copy
        + 'static,
    A: FromLuaMulti<'lua> + 'lua,
    R: IntoLuaMulti<'lua>,
{
    let globals = lua.globals();
    globals.set(
        name,
        lua.create_async_function(move |l, v| {
            let be = bridge.clone();
            async move { f(be, l, v).await.map_err(convert_error) }
        })?,
    )?;
    Ok(())
}

pub fn run_script(
    bridge: Arc<AsyncMutex<dyn Bridge>>,
    fs: Arc<Mutex<dyn xfs::Xfs>>,
    script_path: &Path,
) -> anyhow::Result<()> {
    run_script_ex(bridge, fs, script_path, |_| Ok(()))
}

// The additional F function is used to add hooks when testing
pub fn run_script_ex<F>(
    bridge: Arc<AsyncMutex<dyn Bridge>>,
    fs: Arc<Mutex<dyn xfs::Xfs>>,
    script_path: &Path,
    f: F,
) -> anyhow::Result<()>
where
    F: FnOnce(&Lua) -> anyhow::Result<()>,
{
    let lua = Lua::new();

    lua.sandbox(true)?;

    // Replace print with our own function.
    // let globals = lua.globals();
    // let print = lua.create_function(lua_print)?;
    // globals.set("print", print)?;
    add_bridge_function(bridge.clone(), &lua, "write_file", lua_write_file)?;
    add_bridge_function(bridge.clone(), &lua, "read_file", lua_read_file)?;
    add_bridge_function(bridge.clone(), &lua, "set_metadata", lua_set_metadata)?;
    add_bridge_function(bridge.clone(), &lua, "get_metadata", lua_get_metadata)?;
    add_bridge_function(bridge.clone(), &lua, "ai_query", lua_ai_query)?;
    add_bridge_function(bridge.clone(), &lua, "wrought_template", lua_template)?;

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
    use crate::events::EventGroup;

    use super::*;
    use anyhow::anyhow;
    use async_trait::async_trait;
    use mockall::{mock, predicate};
    use std::sync::{Arc, Mutex};

    mock! {
        pub Bridge {}

        #[async_trait]
        impl Bridge for Bridge {
            fn write_file(&mut self, path: &Path, value: &[u8]) -> anyhow::Result<()>;
            fn read_file(&mut self, path: &Path) -> anyhow::Result<Option<Vec<u8>>>;
            fn get_metadata(&mut self, path: &Path, key: &str) -> anyhow::Result<Option<String>>;
            fn set_metadata(&mut self, path: &Path, key: &str, value: &str) -> anyhow::Result<()>;
            async fn ai_query(&mut self, query: &str) -> anyhow::Result<String>;
            fn get_event_group(&self) -> Option<EventGroup>;
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

    #[tokio::test]
    pub async fn run_script_write_file() {
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

        let mock_bridge = Arc::new(AsyncMutex::new(mock_bridge));
        let fs = Arc::new(Mutex::new(fs));

        run_script(
            mock_bridge.clone(),
            fs,
            &PathBuf::from("somedir/script.luau"),
        )
        .unwrap();

        mock_bridge.lock().await.checkpoint();
    }

    #[tokio::test]
    pub async fn run_script_write_file_invalid() {
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

        let mock_bridge = Arc::new(AsyncMutex::new(mock_bridge));
        let fs = Arc::new(Mutex::new(fs));

        let result = run_script(
            mock_bridge.clone(),
            fs,
            &PathBuf::from("somedir/script.luau"),
        );
        assert!(result.is_err());

        mock_bridge.lock().await.checkpoint();
    }

    #[tokio::test]
    pub async fn run_script_read_file() {
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

        let mock_bridge = Arc::new(AsyncMutex::new(mock_bridge));
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

        mock_bridge.lock().await.checkpoint();
    }

    #[tokio::test]
    pub async fn run_script_read_empty() {
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

        let mock_bridge = Arc::new(AsyncMutex::new(mock_bridge));
        let fs = Arc::new(Mutex::new(fs));

        run_script(
            mock_bridge.clone(),
            fs,
            &PathBuf::from("somedir/script.luau"),
        )
        .unwrap();

        mock_bridge.lock().await.checkpoint();
    }

    #[tokio::test]
    pub async fn run_script_read_error() {
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

        let mock_bridge = Arc::new(AsyncMutex::new(mock_bridge));
        let fs = Arc::new(Mutex::new(fs));

        let result = run_script(
            mock_bridge.clone(),
            fs,
            &PathBuf::from("somedir/script.luau"),
        );
        assert!(result.is_err());

        mock_bridge.lock().await.checkpoint();
    }

    #[test]
    pub fn run_script_set_metadata() {
        todo!();
    }

    #[test]
    pub fn run_script_get_metadata() {
        todo!();
    }

    #[tokio::test]
    pub async fn make_ai_query() {
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

        let mock_bridge = Arc::new(AsyncMutex::new(mock_bridge));
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

        mock_bridge.lock().await.checkpoint();
    }

    #[tokio::test]
    pub async fn make_ai_query_error() {
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

        let mock_bridge = Arc::new(AsyncMutex::new(mock_bridge));
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

        mock_bridge.lock().await.checkpoint();
    }
}

// Testing the new error constructors
#[cfg(test)]
mod json_tests {
    use super::*;

    use super::*;
    use mlua::ffi::lua;
    use mlua::Lua;
    use serde_json::json;

    #[test]
    fn test_error_constructors() {
        let err = ConversionError::unsupported_type("CustomType");
        assert!(matches!(err, ConversionError::UnsupportedType(_)));

        let err = ConversionError::invalid_number(f64::INFINITY);
        assert!(matches!(err, ConversionError::InvalidNumber(_)));

        let err = ConversionError::invalid_table_key("invalid key");
        assert!(matches!(err, ConversionError::InvalidTableKey(_)));
    }

    #[test]
    fn test_roundtrip_array() -> Result<(), Box<dyn std::error::Error>> {
        let lua = Lua::new();

        // Test various array types
        let test_cases = vec![
            json!([]),
            json!([1, 2, 3]),
            // We can't handle a null as the last entry in an array, but we shoulud be able to handle an internal null,
            json!([1, "two", null, true,]),
            json!([[1, 2], ["a", "b"]]),
        ];

        for case in test_cases {
            let lua_table = json_value_to_lua_value(&lua, &case)?;
            let round_trip = lua_value_to_json_value(lua_table, true)?;
            assert_eq!(case, round_trip, "Failed roundtrip for case: {}", case);
        }

        Ok(())
    }

    #[test]
    fn test_roundtrip_object() -> Result<(), Box<dyn std::error::Error>> {
        let lua = Lua::new();

        // Test various object types
        let test_cases = vec![
            json!({}),
            json!({"a": 1, "b": 2}),
            json!({"nested": {"array": [1,2,3], "obj": {"x": true}}}),
            json!({"mixed": [{"a": 1}, null, [1,2,3]]}),
        ];

        for case in test_cases {
            eprintln!("test_roundtrip_object: case='{}'", case);
            let lua_table = json_value_to_lua_value(&lua, &case)?;
            eprintln!("lua_table: {:?}", lua_table);
            let round_trip = lua_value_to_json_value(lua_table, false)?;
            assert_eq!(case, round_trip, "Failed roundtrip for case: {}", case);
        }

        Ok(())
    }

    #[test]
    fn test_error_cases() -> Result<(), Box<dyn std::error::Error>> {
        let lua = Lua::new();

        // Test function conversion
        let func = lua.create_function(|_, ()| Ok(()))?;
        assert!(matches!(
            lua_value_to_json_value(LuaValue::Function(func), false),
            Err(ConversionError::UnsupportedType(_))
        ));

        // Test invalid numbers
        let infinity = std::f64::INFINITY;
        assert!(matches!(
            lua_value_to_json_value(LuaValue::Number(infinity), false),
            Err(ConversionError::InvalidNumber(_))
        ));

        Ok(())
    }
}
