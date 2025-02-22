use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use mlua::Lua;

use crate::bridge::Bridge;

use super::add_bridge_function;

pub fn lua_write_file(
    bridge: Arc<Mutex<dyn Bridge>>,
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
    bridge: Arc<Mutex<dyn Bridge>>,
    _lua: &Lua,
    file_name: String,
) -> anyhow::Result<Option<String>> {
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
    bridge: Arc<Mutex<dyn Bridge>>,
    _lua: &Lua,
    (file_name, key): (String, String),
) -> anyhow::Result<Option<String>> {
    let result = bridge
        .lock()
        .unwrap()
        .get_metadata(&PathBuf::from(file_name), &key)?;
    Ok(result)
}

pub fn lua_set_metadata(
    bridge: Arc<Mutex<dyn Bridge>>,
    _lua: &Lua,
    (file_name, key, value): (String, String, String),
) -> anyhow::Result<()> {
    bridge
        .lock()
        .unwrap()
        .set_metadata(&PathBuf::from(file_name), &key, &value)?;
    Ok(())
}

pub fn register_functions(bridge: &Arc<Mutex<dyn Bridge>>, lua: &Lua) -> anyhow::Result<()> {
    add_bridge_function(bridge.clone(), &lua, "write_file", lua_write_file)?;
    add_bridge_function(bridge.clone(), &lua, "read_file", lua_read_file)?;
    add_bridge_function(bridge.clone(), &lua, "set_metadata", lua_set_metadata)?;
    add_bridge_function(bridge.clone(), &lua, "get_metadata", lua_get_metadata)?;
    Ok(())
}
