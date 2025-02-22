use std::sync::{Arc, Mutex};

use mlua::prelude::*;
use mlua::Lua;

use crate::bridge::Bridge;

use super::{add_bridge_function, convert_error, luau_json::json_value_to_lua_value};

pub fn lua_ai_query(
    bridge: Arc<Mutex<dyn Bridge>>,
    _lua: &Lua,
    query: String,
) -> anyhow::Result<String> {
    bridge.lock().unwrap().ai_query(&query)
}

#[derive(Clone)]
struct AiMessage {
    bridge: Arc<Mutex<dyn Bridge>>,
    internal: crate::llm::AiMessage,
}

impl AiMessage {
    fn set_system_message(&mut self, system_message: String) -> anyhow::Result<()> {
        self.internal.set_system_message(&system_message)
    }

    fn add_user_message(&mut self, user_message: String) -> anyhow::Result<()> {
        self.internal.add_user_message(&user_message)
    }

    fn add_tool(
        &mut self,
        tool_description: String,
        tool_name: String,
        tool_schema: String,
    ) -> anyhow::Result<()> {
        self.internal
            .add_tool(&tool_description, &tool_name, &tool_schema)
    }

    fn request<'lua>(&mut self, lua: &'lua Lua) -> anyhow::Result<mlua::Value<'lua>> {
        let json = self
            .bridge
            .lock()
            .unwrap()
            .send_ai_message(&self.internal)
            .map_err(convert_error)?;
        let lua_value = json_value_to_lua_value(lua, &json)?;
        Ok(lua_value)
    }
}

impl LuaUserData for AiMessage {
    fn add_methods<'lua, M: LuaUserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_method_mut(
            "set_system_message",
            |_, this, (system_message,): (String,)| {
                this.set_system_message(system_message)
                    .map_err(convert_error)
            },
        );
        methods.add_method_mut("add_user_message", |_, this, (user_message,): (String,)| {
            this.add_user_message(user_message).map_err(convert_error)
        });
        methods.add_method_mut(
            "add_tool",
            |_, this, (tool_description, tool_name, tool_schema): (String, String, String)| {
                this.add_tool(tool_description, tool_name, tool_schema)
                    .map_err(convert_error)
            },
        );

        methods.add_method_mut("request", |lua, this, _: ()| {
            this.request(lua).map_err(convert_error)
        })
    }
}

fn lua_ai_message(
    bridge: Arc<Mutex<dyn Bridge>>,
    _lua: &Lua,
    _params: (),
) -> anyhow::Result<AiMessage> {
    let internal = bridge.lock().unwrap().ai_message()?;
    Ok(AiMessage { internal, bridge })
}

pub fn register_functions(bridge: &Arc<Mutex<dyn Bridge>>, lua: &Lua) -> anyhow::Result<()> {
    add_bridge_function(bridge.clone(), &lua, "ai_query", lua_ai_query)?;
    add_bridge_function(bridge.clone(), &lua, "wrought_ai_message", lua_ai_message)?;
    Ok(())
}
