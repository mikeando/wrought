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
    eprintln!("In luad_set_metadata...");
    bridge
        .lock()
        .unwrap()
        .set_metadata(&PathBuf::from(file_name), &key, &value)?;
    Ok(())
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

pub fn run_script(bridge: Arc<Mutex<dyn Bridge>>, script_path: &Path) -> anyhow::Result<()> {
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

    let script = std::fs::read_to_string(script_path)?;
    lua.load(script).exec()?;
    Ok(())
}
