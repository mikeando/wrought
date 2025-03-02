pub mod ai;
pub mod io;
pub mod luau_json;
pub mod templates;
pub mod kb;

#[cfg(test)]
pub mod tests;

use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;

use mlua::prelude::*;
use mlua::Lua;

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

pub fn add_bridge_function<'lua, F, A, R>(
    bridge: Arc<Mutex<dyn Bridge>>,
    lua: &'lua Lua,
    name: &str,
    f: F,
) -> anyhow::Result<()>
where
    F: for<'a> Fn(Arc<Mutex<dyn Bridge>>, &'a Lua, A) -> anyhow::Result<R> + Copy + 'static,
    A: FromLuaMulti<'lua> + 'lua,
    R: IntoLuaMulti<'lua>,
{
    let globals = lua.globals();
    globals.set(
        name,
        lua.create_function(move |l, v| f(bridge.clone(), l, v).map_err(convert_error))?,
    )?;
    Ok(())
}

pub fn run_script(
    bridge: Arc<Mutex<dyn Bridge>>,
    fs: Arc<Mutex<dyn xfs::Xfs>>,
    script_path: &Path,
) -> anyhow::Result<()> {
    run_script_ex(bridge, fs, script_path, |_| Ok(()))
}

// The additional F function is used to add hooks when testing
pub fn run_script_ex<F>(
    bridge: Arc<Mutex<dyn Bridge>>,
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
    io::register_functions(&bridge, &lua)?;
    ai::register_functions(&bridge, &lua)?;
    templates::register_functions(&bridge, &lua)?;
    kb::register_functions(&bridge, &lua)?;

    f(&lua)?;

    let mut script = String::new();
    fs.lock()
        .unwrap()
        .reader(script_path)?
        .read_to_string(&mut script)?;

    lua.load(script).set_name(format!("{}", script_path.display())).exec()?;
    Ok(())
}
