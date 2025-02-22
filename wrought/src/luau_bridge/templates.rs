use std::sync::{Arc, Mutex};

use crate::bridge::Bridge;

use super::{add_bridge_function, convert_error, luau_json::lua_table_to_json};
use mlua::prelude::*;

struct LuaTemplater {
    tera: tera::Tera,
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

fn lua_template(
    _bridge: Arc<Mutex<dyn Bridge>>,
    _lua: &Lua,
    _params: (),
) -> anyhow::Result<LuaTemplater> {
    Ok(LuaTemplater {
        tera: tera::Tera::default(),
    })
}

pub fn register_functions(bridge: &Arc<Mutex<dyn Bridge>>, lua: &Lua) -> anyhow::Result<()> {
    add_bridge_function(bridge.clone(), &lua, "wrought_template", lua_template)?;
    Ok(())
}
