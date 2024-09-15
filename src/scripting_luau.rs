use std::path::Path;

use mlua::prelude::*;
use mlua::MultiValue;

pub fn run_script(script_path: &Path) -> anyhow::Result<()> {
    let lua = Lua::new();

    lua.sandbox(true)?;

    // Replace print with our own function.
    let globals = lua.globals();
    let print = lua.create_function(|_, vals: MultiValue| {
        println!(
            "Lua: {}",
            vals.iter()
                .map(|v| v.to_string())
                .collect::<Result<Vec<_>, _>>()
                .map(|v| v.join(" "))
                .unwrap()
        );
        Ok(())
    })?;
    globals.set("print", print)?;

    let script = std::fs::read_to_string(script_path)?;
    lua.load(script).exec()?;
    Ok(())
}
