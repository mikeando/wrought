use std::sync::{Arc, Mutex};

use crate::bridge::{Bridge, KBDoc};

use super::{add_bridge_function, convert_error, luau_json::lua_table_to_json};
use mlua::prelude::*;

struct LuaKB {
    bridge: Arc<Mutex<dyn Bridge>>,
}

impl LuaKB {
}

impl LuaUserData for LuaKB {
    fn add_methods<'lua, M: LuaUserDataMethods<'lua, Self>>(methods: &mut M) {

        methods.add_method("doc_from_content", |_, this, (content, ): (String, )| {
            Ok(LuaKBDoc{
                bridge: this.bridge.clone(),
                content,
                tags: vec![],
            })
        });
    }
}

#[derive(Clone)]
struct LuaKBDoc {
    bridge: Arc<Mutex<dyn Bridge>>,
    content: String,
    tags: Vec<String>,
}
impl LuaKBDoc {}

impl std::fmt::Debug for LuaKBDoc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {

        let content = if self.content.len() > 50 {
            // TODO: This doesn't work for non-single-byte utf-8 characters.
            format!("{}...{}", &self.content[..23], &self.content[(self.content.len()-23)..])
        } else {
            self.content.clone()
        };
        f.debug_struct("LuaKBDoc").field("content", &content).field("tags", &self.tags).finish()
    }
}

impl LuaUserData for LuaKBDoc {
    fn add_methods<'lua, M: LuaUserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_method("tagged", |_, this, (tag, ): (String, )| {
            let mut copy = this.clone();
            copy.tags.push(tag);
            Ok(copy)
        });
        methods.add_method_mut("ingest", |_, this, _: ()| {
            let doc = KBDoc{
                content: this.content.clone(),
                tags: this.tags.clone(),
            };
            this.bridge.lock().unwrap().kb_ingest(&doc).map_err(convert_error)?;
            Ok(())
        });
    }

}

fn lua_kb(
    bridge: Arc<Mutex<dyn Bridge>>,
    _lua: &Lua,
    _params: (),
) -> anyhow::Result<LuaKB> {
    Ok(LuaKB {bridge})
}

pub fn register_functions(bridge: &Arc<Mutex<dyn Bridge>>, lua: &Lua) -> anyhow::Result<()> {
    add_bridge_function(bridge.clone(), &lua, "wrought_kb", lua_kb)?;
    Ok(())
}