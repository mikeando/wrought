use anyhow::Context;
use wasmtime::{Caller, Linker};
use wrought_wasm_bindings::WroughtResult;

use crate::llm::AiMessage;

use super::CombinedContext;


/*
    fn wrought_ai_query(
    query_ptr: *const u8,
    query_len: usize,
);
*/
pub fn wasm_ai_query(mut caller: Caller<'_, CombinedContext>, query_ptr: i32, query_len: i32) {
    let memory = caller.get_export("memory").unwrap().into_memory().unwrap();
    let data = memory.data(&caller);
    let query =
        std::str::from_utf8(&data[query_ptr as usize..(query_ptr + query_len) as usize]).unwrap();

    let result: wrought_wasm_bindings::WroughtResult<String> = caller
        .data()
        .0
        .bridge
        .lock()
        .unwrap()
        .ai_query(query)
        .map_err(|e| format!("{}", e));
    let out_buf = serde_json::to_vec(&result).unwrap();
    caller.data_mut().0.call_buffer.call_buffer = Some(Ok(out_buf));
}


pub fn wasm_ai_message(mut caller: Caller<'_, CombinedContext>) {
    let app_state = &mut caller.data_mut().0;
    let message_id = app_state.next_ai_message_id;
    app_state
        .ai_messages
        .insert(message_id, AiMessage::default());
    app_state.next_ai_message_id += 1;

    let result = WroughtResult::Ok(message_id);
    let out_buf = serde_json::to_vec(&result).unwrap();
    caller.data_mut().0.call_buffer.call_buffer = Some(Ok(out_buf));
}

// fn wrought_ai_message_request(id: i32);

pub fn wasm_ai_message_request(mut caller: Caller<'_, CombinedContext>, id: i32) {
    let app_state = &mut caller.data_mut().0;
    let message = app_state.ai_messages.get(&id).unwrap();
    let result = app_state.bridge.lock().unwrap().send_ai_message(message);
    let result: WroughtResult<serde_json::Value> = result.map_err(|e| format!("llm-error: {}", e));
    let out_buf = serde_json::to_vec(&result).unwrap();
    caller.data_mut().0.call_buffer.call_buffer = Some(Ok(out_buf));
}

pub fn wasm_ai_message_add_tool(
    mut caller: Caller<'_, CombinedContext>,
    id: i32,
    description_ptr: i32,
    description_len: i32,
    name_ptr: i32,
    name_len: i32,
    schema_ptr: i32,
    schema_len: i32,
) {
    let memory = caller.get_export("memory").unwrap().into_memory().unwrap();
    let data = memory.data(&caller);
    let description = std::str::from_utf8(
        &data[description_ptr as usize..(description_ptr + description_len) as usize],
    )
    .unwrap()
    .to_string();
    let name = std::str::from_utf8(&data[name_ptr as usize..(name_ptr + name_len) as usize])
        .unwrap()
        .to_string();
    let schema =
        std::str::from_utf8(&data[schema_ptr as usize..(schema_ptr + schema_len) as usize])
            .unwrap()
            .to_string();
    // let schema: serde_json::Value = serde_json::from_str(&schema).unwrap();

    let app_state = &mut caller.data_mut().0;
    app_state
        .ai_messages
        .get_mut(&id)
        .unwrap()
        .add_tool(&description, &name, &schema)
        .unwrap();

    let result = WroughtResult::Ok(());
    let out_buf = serde_json::to_vec(&result).unwrap();
    caller.data_mut().0.call_buffer.call_buffer = Some(Ok(out_buf));
}

pub fn wasm_ai_message_set_user_message(
    mut caller: Caller<'_, CombinedContext>,
    id: i32,
    message_ptr: i32,
    len: i32,
) {
    let memory = caller.get_export("memory").unwrap().into_memory().unwrap();
    let data = memory.data(&caller);
    let message = std::str::from_utf8(&data[message_ptr as usize..(message_ptr + len) as usize])
        .unwrap()
        .to_string();

    let app_state = &mut caller.data_mut().0;
    app_state
        .ai_messages
        .get_mut(&id)
        .unwrap()
        .add_user_message(&message)
        .unwrap();

    let result = WroughtResult::Ok(());
    let out_buf = serde_json::to_vec(&result).unwrap();
    caller.data_mut().0.call_buffer.call_buffer = Some(Ok(out_buf));
}

pub fn wasm_ai_message_set_system_message(
    mut caller: Caller<'_, CombinedContext>,
    id: i32,
    message_ptr: i32,
    len: i32,
) {
    let memory = caller.get_export("memory").unwrap().into_memory().unwrap();
    let data = memory.data(&caller);
    let message = std::str::from_utf8(&data[message_ptr as usize..(message_ptr + len) as usize])
        .unwrap()
        .to_string();

    let app_state = &mut caller.data_mut().0;
    app_state
        .ai_messages
        .get_mut(&id)
        .unwrap()
        .set_system_message(&message)
        .unwrap();

    let result = WroughtResult::Ok(());
    let out_buf = serde_json::to_vec(&result).unwrap();
    caller.data_mut().0.call_buffer.call_buffer = Some(Ok(out_buf));
}

pub fn wasm_drop_ai_message(mut caller: Caller<'_, CombinedContext>, id: i32) {
    let app_state = &mut caller.data_mut().0;
    // TODO: This should probably not be an assert, as that allows plugins to crash the host.
    assert!(app_state.ai_messages.contains_key(&id));
    app_state.ai_messages.remove(&id);
}

pub fn reigister_functions(linker: &mut Linker<CombinedContext>) -> anyhow::Result<()> {
    linker
        .func_wrap("env", "wrought_ai_query", wasm_ai_query)
        .with_context(|| "Error installing wrought_ai_query function")?;

    linker
        .func_wrap("env", "wrought_drop_ai_message", wasm_drop_ai_message)
        .with_context(|| "Error installing wrought_drop_ai_message function")?;

    linker
        .func_wrap("env", "wrought_ai_message", wasm_ai_message)
        .with_context(|| "Error installing wrought_ai_message function")?;

    linker
        .func_wrap(
            "env",
            "wrought_ai_message_set_system_message",
            wasm_ai_message_set_system_message,
        )
        .with_context(|| "Error installing wrought_ai_message_set_system_message function")?;

    linker
        .func_wrap(
            "env",
            "wrought_ai_message_set_user_message",
            wasm_ai_message_set_user_message,
        )
        .with_context(|| "Error installing wrought_ai_message_set_user_message function")?;

    linker
        .func_wrap("env", "wrought_ai_message_add_tool", wasm_ai_message_add_tool)
        .with_context(|| "Error installing wrought_ai_meswrought_ai_message_add_toolsage_set_user_message function")?;

    linker
        .func_wrap("env", "wrought_ai_message_request", wasm_ai_message_request)
        .with_context(|| "Error installing wrought_ai_message_send function")?;

    Ok(())
}