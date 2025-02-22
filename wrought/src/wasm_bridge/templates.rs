use anyhow::Context;
use wasmtime::{Caller, Linker};
use wrought_wasm_bindings::WroughtResult;

use super::CombinedContext;

// fn wrought_init_template();
pub fn wasm_init_template(mut caller: Caller<'_, CombinedContext>) {
    // let memory = caller.get_export("memory").unwrap().into_memory().unwrap();

    let app_state = &mut caller.data_mut().0;
    let template_id = app_state.next_template_id;
    assert!(!app_state.templating.contains_key(&template_id));
    app_state.next_template_id += 1;
    app_state
        .templating
        .insert(template_id, tera::Tera::default());

    let result = WroughtResult::Ok(template_id);
    let out_buf = serde_json::to_vec(&result).unwrap();
    caller.data_mut().0.call_buffer.call_buffer = Some(Ok(out_buf));
}

// fn wrought_drop_template(id: i32);
pub fn wasm_drop_template(mut caller: Caller<'_, CombinedContext>, id: i32) {
    let app_state = &mut caller.data_mut().0;
    // TODO: This should probably not be an assert, as that allows plugins to crash the host.
    assert!(app_state.templating.contains_key(&id));
    app_state.templating.remove(&id);
}

// fn wrought_add_templates(id: i32, encoded_templates_ptr: *const u8, len: usize);
pub fn wasm_add_templates(
    mut caller: Caller<'_, CombinedContext>,
    id: i32,
    encoded_templates_ptr: i32,
    len: i32,
) {
    let memory = caller.get_export("memory").unwrap().into_memory().unwrap();
    let data = memory.data(&caller);
    let encoded_templates = std::str::from_utf8(
        &data[encoded_templates_ptr as usize..(encoded_templates_ptr + len) as usize],
    )
    .unwrap();

    // decode them...
    // TODO: Not an unwrap....
    let templates: Vec<(String, String)> = serde_json::from_str(encoded_templates).unwrap();

    // Then add them all
    let app_state = &mut caller.data_mut().0;
    // TODO: These should probably not be an unwrap, as that allows plugins to crash the host.
    app_state
        .templating
        .get_mut(&id)
        .unwrap()
        .add_raw_templates(templates)
        .unwrap();

    let result = WroughtResult::Ok(());
    let out_buf = serde_json::to_vec(&result).unwrap();
    caller.data_mut().0.call_buffer.call_buffer = Some(Ok(out_buf));
}

// fn wrought_render_template(id: i32, key_ptr: *const u8, key_len: usize, content_ptr: *const u8, content_len: usize);
pub fn wasm_render_template(
    mut caller: Caller<'_, CombinedContext>,
    id: i32,
    key_ptr: i32,
    key_len: i32,
    content_ptr: i32,
    content_len: i32,
) {
    let memory = caller.get_export("memory").unwrap().into_memory().unwrap();
    let data = memory.data(&caller);
    let key = std::str::from_utf8(&data[key_ptr as usize..(key_ptr + key_len) as usize]).unwrap();
    let content =
        std::str::from_utf8(&data[content_ptr as usize..(content_ptr + content_len) as usize])
            .unwrap();

    // TODO: Should not be unwrap!
    let context: serde_json::Value = serde_json::from_str(content).unwrap();

    // Then add them all
    let app_state = &caller.data().0;
    // TODO: These should probably not be an unwrap, as that allows plugins to crash the host.
    let result = app_state
        .templating
        .get(&id)
        .unwrap()
        .render(key, &tera::Context::from_value(context).unwrap())
        .unwrap();

    let result = WroughtResult::Ok(result);
    let out_buf = serde_json::to_vec(&result).unwrap();
    caller.data_mut().0.call_buffer.call_buffer = Some(Ok(out_buf));
}

pub fn reigister_functions(linker: &mut Linker<CombinedContext>) -> anyhow::Result<()> {
    linker
        .func_wrap("env", "wrought_render_template", wasm_render_template)
        .with_context(|| "Error installing wrought_render_template function")?;

    linker
        .func_wrap("env", "wrought_drop_template", wasm_drop_template)
        .with_context(|| "Error installing wrought_drop_template function")?;

    linker
        .func_wrap("env", "wrought_init_template", wasm_init_template)
        .with_context(|| "Error installing wrought_drop_template function")?;

    linker
        .func_wrap("env", "wrought_add_templates", wasm_add_templates)
        .with_context(|| "Error installing wrought_add_templates function")?;
    Ok(())
}
