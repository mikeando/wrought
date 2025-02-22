use std::path::PathBuf;

use anyhow::Context;
use wasmtime::{Caller, Linker};

use super::CombinedContext;

// exposed in the bindings as:
// fn wrought_write_file(
//     path_ptr: *const u8,
//     path_len: usize,
//     content_ptr: *const u8,
//     content_len: usize,
// );
pub fn wasm_write_file(
    mut caller: Caller<'_, CombinedContext>,
    path_ptr: i32,
    path_len: i32,
    content_ptr: i32,
    content_len: i32,
) {
    let memory = caller.get_export("memory").unwrap().into_memory().unwrap();
    let data = memory.data(&caller);
    let path =
        std::str::from_utf8(&data[path_ptr as usize..(path_ptr + path_len) as usize]).unwrap();
    let content = &data[content_ptr as usize..(content_ptr + content_len) as usize];
    let path = PathBuf::from(path);
    let result: wrought_wasm_bindings::WroughtResult<()> = caller
        .data()
        .0
        .bridge
        .lock()
        .unwrap()
        .write_file(&path, content)
        .map_err(|e| format!("{}", e));
    let out_buf = serde_json::to_vec(&result).unwrap();
    caller.data_mut().0.call_buffer.call_buffer = Some(Ok(out_buf));
}
/*
        fn wrought_read_file(
            path_ptr: *const u8,
            path_len: usize,
        );
*/
pub fn wasm_read_file(mut caller: Caller<'_, CombinedContext>, path_ptr: i32, path_len: i32) {
    let memory = caller.get_export("memory").unwrap().into_memory().unwrap();
    let data = memory.data(&caller);
    let path =
        std::str::from_utf8(&data[path_ptr as usize..(path_ptr + path_len) as usize]).unwrap();
    let path = PathBuf::from(path);
    let result: wrought_wasm_bindings::WroughtResult<Option<Vec<u8>>> = caller
        .data()
        .0
        .bridge
        .lock()
        .unwrap()
        .read_file(&path)
        .map_err(|e| format!("{}", e));
    let out_buf = serde_json::to_vec(&result).unwrap();
    caller.data_mut().0.call_buffer.call_buffer = Some(Ok(out_buf));
}
/*
fn wrought_get_metadata(
    path_ptr: *const u8,
    path_len: usize,
    key_ptr: *const u8,
    key_len: usize,
);
*/
pub fn wasm_get_metadata(
    mut caller: Caller<'_, CombinedContext>,
    path_ptr: i32,
    path_len: i32,
    key_ptr: i32,
    key_len: i32,
) {
    let memory = caller.get_export("memory").unwrap().into_memory().unwrap();
    let data = memory.data(&caller);
    let path =
        std::str::from_utf8(&data[path_ptr as usize..(path_ptr + path_len) as usize]).unwrap();
    let path = PathBuf::from(path);
    let key = std::str::from_utf8(&data[key_ptr as usize..(key_ptr + key_len) as usize]).unwrap();

    let result: wrought_wasm_bindings::WroughtResult<Option<String>> = caller
        .data()
        .0
        .bridge
        .lock()
        .unwrap()
        .get_metadata(&path, key)
        .map_err(|e| format!("{}", e));
    let out_buf = serde_json::to_vec(&result).unwrap();
    caller.data_mut().0.call_buffer.call_buffer = Some(Ok(out_buf));
}

/*
fn wrought_set_metadata(
    path_ptr: *const u8,
    path_len: usize,
    key_ptr: *const u8,
    key_len: usize,
    content_ptr: *const u8,
    content_len: usize
);
*/
pub fn wasm_set_metadata(
    mut caller: Caller<'_, CombinedContext>,
    path_ptr: i32,
    path_len: i32,
    key_ptr: i32,
    key_len: i32,
    content_ptr: i32,
    content_len: i32,
) {
    let memory = caller.get_export("memory").unwrap().into_memory().unwrap();
    let data = memory.data(&caller);
    let path =
        std::str::from_utf8(&data[path_ptr as usize..(path_ptr + path_len) as usize]).unwrap();
    let path = PathBuf::from(path);
    let key = std::str::from_utf8(&data[key_ptr as usize..(key_ptr + key_len) as usize]).unwrap();
    let content =
        std::str::from_utf8(&data[content_ptr as usize..(content_ptr + content_len) as usize])
            .unwrap();

    let result: wrought_wasm_bindings::WroughtResult<()> = caller
        .data()
        .0
        .bridge
        .lock()
        .unwrap()
        .set_metadata(&path, key, content)
        .map_err(|e| format!("{}", e));
    let out_buf = serde_json::to_vec(&result).unwrap();
    caller.data_mut().0.call_buffer.call_buffer = Some(Ok(out_buf));
}

pub fn reigister_functions(linker: &mut Linker<CombinedContext>) -> anyhow::Result<()> {
linker
    .func_wrap("env", "wrought_write_file", wasm_write_file)
    .with_context(|| "Error installing wrought_write_file function")?;

linker
    .func_wrap("env", "wrought_read_file", wasm_read_file)
    .with_context(|| "Error installing wrought_read_file function")?;

linker
    .func_wrap("env", "wrought_get_metadata", wasm_get_metadata)
    .with_context(|| "Error installing wrought_get_metadata function")?;

linker
    .func_wrap("env", "wrought_set_metadata", wasm_set_metadata)
    .with_context(|| "Error installing wrought_set_metadata function")?;
Ok(())
}