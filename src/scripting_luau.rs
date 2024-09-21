use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

use mlua::prelude::*;
use mlua::MultiValue;

use crate::bridge::Bridge;

pub fn lua_print(_lua: &Lua, vals: MultiValue) -> mlua::Result<()> {
    println!(
        "Lua: {}",
        vals.iter()
            .map(|v| v.to_string())
            .collect::<Result<Vec<_>, _>>()
            .map(|v| v.join(" "))
            .unwrap()
    );
    Ok(())
}

pub fn convert_error(e: anyhow::Error) -> mlua::Error {
    mlua::Error::runtime(format!("{}", e))
}

pub fn lua_write_file(
    bridge: Arc<Mutex<dyn Bridge>>,
    _lua: &Lua,
    (file_name, value): (String, String),
) -> mlua::Result<()> {
    bridge
        .lock()
        .unwrap()
        .write_file(&PathBuf::from(file_name), value.as_bytes())
        .map_err(convert_error)?;
    Ok(())
}

pub fn run_script(bridge: Arc<Mutex<dyn Bridge>>, script_path: &Path) -> anyhow::Result<()> {
    let lua = Lua::new();

    lua.sandbox(true)?;

    // Replace print with our own function.
    let globals = lua.globals();
    let print = lua.create_function(lua_print)?;
    globals.set("print", print)?;
    let be = bridge.clone();
    globals.set(
        "write_file",
        lua.create_function(move |l, v| lua_write_file(be.clone(), l, v))?,
    )?;

    let script = std::fs::read_to_string(script_path)?;
    lua.load(script).exec()?;
    Ok(())
}
