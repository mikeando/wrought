use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::Context;
use bytes::Bytes;
use wasmtime::{Caller, Config, Engine, Linker, Module, Store};
use wasmtime_wasi::preview1::{self, WasiP1Ctx};
use wasmtime_wasi::{HostOutputStream, StdoutStream, StreamResult, Subscribe, WasiCtxBuilder};
use wrought_wasm_bindings::WroughtResult;

use crate::bridge::Bridge;

type AsyncMutex<T> = tokio::sync::Mutex<T>;

// In your host code:
#[derive(Debug)]
enum WasmError {
    Normal(String),
    Panic(String),
    // Could add timestamp, module name, etc
}

const ERROR_TYPE_NORMAL: i32 = 1;
const ERROR_TYPE_PANIC: i32 = 2;

pub struct AppState {
    pub bridge: Arc<AsyncMutex<dyn Bridge + Send + 'static>>,
    pub templating: BTreeMap<i32, tera::Tera>,
    pub next_template_id: i32,
    pub call_buffer: wasmcb::CallBuffer,
}

pub struct CombinedContext(AppState, WasiP1Ctx);

impl wasmcb::ProvidesCallBuffer for CombinedContext {
    fn get_call_buffer(&self) -> &wasmcb::CallBuffer {
        &self.0.call_buffer
    }

    fn get_call_buffer_mut(&mut self) -> &mut wasmcb::CallBuffer {
        &mut self.0.call_buffer
    }
}

pub async fn run_script(
    bridge: Arc<AsyncMutex<dyn Bridge + Send + 'static>>,
    fs: Arc<Mutex<dyn xfs::Xfs>>,
    script_path: &Path,
) -> anyhow::Result<()> {
    run_script_ex(bridge, fs, script_path, |_| Ok(())).await
}

struct CustomHostOutputStream {
    buffer: Arc<Mutex<Vec<u8>>>,
}

struct CustomStdout {
    buffer: Arc<Mutex<Vec<u8>>>,
}

#[async_trait::async_trait]
impl Subscribe for CustomHostOutputStream {
    async fn ready(&mut self) {}
}

impl HostOutputStream for CustomHostOutputStream {
    fn write(&mut self, bytes: Bytes) -> StreamResult<()> {
        self.buffer.lock().unwrap().extend_from_slice(&bytes);
        StreamResult::Ok(())
    }

    fn flush(&mut self) -> StreamResult<()> {
        StreamResult::Ok(())
    }

    fn check_write(&mut self) -> StreamResult<usize> {
        StreamResult::Ok(usize::MAX)
    }
}

impl StdoutStream for CustomStdout {
    fn stream(&self) -> Box<dyn HostOutputStream> {
        Box::new(CustomHostOutputStream {
            buffer: self.buffer.clone(),
        })
    }

    fn isatty(&self) -> bool {
        true
    }
}

// exposed in the bindings as:
// fn wrought_write_file(
//     path_ptr: *const u8,
//     path_len: usize,
//     content_ptr: *const u8,
//     content_len: usize,
// );
async fn wasm_write_file(
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
        .await
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
async fn wasm_read_file(mut caller: Caller<'_, CombinedContext>, path_ptr: i32, path_len: i32) {
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
        .await
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
async fn wasm_get_metadata(
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
        .await
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
async fn wasm_set_metadata(
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
        .await
        .set_metadata(&path, key, content)
        .map_err(|e| format!("{}", e));
    let out_buf = serde_json::to_vec(&result).unwrap();
    caller.data_mut().0.call_buffer.call_buffer = Some(Ok(out_buf));
}

/*
    fn wrought_ai_query(
    query_ptr: *const u8,
    query_len: usize,
);
*/
async fn wasm_ai_query(mut caller: Caller<'_, CombinedContext>, query_ptr: i32, query_len: i32) {
    let memory = caller.get_export("memory").unwrap().into_memory().unwrap();
    let data = memory.data(&caller);
    let query =
        std::str::from_utf8(&data[query_ptr as usize..(query_ptr + query_len) as usize]).unwrap();

    let result: wrought_wasm_bindings::WroughtResult<String> = caller
        .data()
        .0
        .bridge
        .lock()
        .await
        .ai_query(query)
        .await
        .map_err(|e| format!("{}", e));
    let out_buf = serde_json::to_vec(&result).unwrap();
    caller.data_mut().0.call_buffer.call_buffer = Some(Ok(out_buf));
}

// fn wrought_init_template();
async fn wasm_init_template(mut caller: Caller<'_, CombinedContext>) {
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
async fn wasm_drop_template(mut caller: Caller<'_, CombinedContext>, id: i32) {
    let app_state = &mut caller.data_mut().0;
    // TODO: This should probably not be an assert, as that allows plugins to crash the host.
    assert!(app_state.templating.contains_key(&id));
    app_state.templating.remove(&id);
}

// fn wrought_add_templates(id: i32, encoded_templates_ptr: *const u8, len: usize);
async fn wasm_add_templates(
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
async fn wasm_render_template(
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

// The additional F function is used to add hooks when testing
pub async fn run_script_ex<F>(
    bridge: Arc<AsyncMutex<dyn Bridge + Send + 'static>>,
    fs: Arc<Mutex<dyn xfs::Xfs>>,
    script_path: &Path,
    f: F,
) -> anyhow::Result<()>
where
    F: FnOnce(&Linker<CombinedContext>) -> anyhow::Result<()>,
{
    // Construct the wasm engine with async support enabled.
    let mut config = Config::new();
    config.async_support(true);
    let engine = Engine::new(&config).with_context(|| "error creating wasm context")?;
    let stdout_buffer = Arc::new(Mutex::new(vec![]));
    let stderr_buffer = Arc::new(Mutex::new(vec![]));
    let custom_stdout = CustomStdout {
        buffer: stdout_buffer.clone(),
    };
    let custom_stderr = CustomStdout {
        buffer: stderr_buffer.clone(),
    };

    // Add the WASI preview1 API to the linker (will be implemented in terms of
    // the preview2 API)
    let mut linker: Linker<CombinedContext> = Linker::new(&engine);
    preview1::add_to_linker_async(&mut linker, |t| &mut t.1)
        .with_context(|| "error installing WASI libraries to core engine")?;

    // Add capabilities (e.g. filesystem access) to the WASI preview2 context
    // here. Here only stdio is inherited, but see docs of `WasiCtxBuilder` for
    // more.
    let wasi_ctx = WasiCtxBuilder::new()
        .inherit_stdin()
        // .stdout(custom_stdout)
        // .stderr(custom_stderr)
        .inherit_stdout()
        .inherit_stderr()
        .build_p1();
    let app_state = AppState {
        bridge,
        templating: BTreeMap::new(),
        next_template_id: 0,
        call_buffer: wasmcb::CallBuffer::new(),
    };

    let mut store = Store::new(&engine, CombinedContext(app_state, wasi_ctx));
    wasmcb::add_to_linker(&mut linker)?;

    linker
        .func_wrap_async(
            "env",
            "wrought_write_file",
            |caller: Caller<'_, CombinedContext>,
             (path_ptr, path_len, content_ptr, content_len): (i32, i32, i32, i32)| {
                Box::new(async move {
                    wasm_write_file(caller, path_ptr, path_len, content_ptr, content_len).await
                })
            },
        )
        .with_context(|| "Error installing wrought_write_file function")?;

    linker
        .func_wrap_async(
            "env",
            "wrought_read_file",
            |caller: Caller<'_, CombinedContext>, (path_ptr, path_len): (i32, i32)| {
                Box::new(async move { wasm_read_file(caller, path_ptr, path_len).await })
            },
        )
        .with_context(|| "Error installing wrought_read_file function")?;

    linker
        .func_wrap_async(
            "env",
            "wrought_get_metadata",
            |caller: Caller<'_, CombinedContext>,
             (path_ptr, path_len, key_ptr, key_len): (i32, i32, i32, i32)| {
                Box::new(async move {
                    wasm_get_metadata(caller, path_ptr, path_len, key_ptr, key_len).await
                })
            },
        )
        .with_context(|| "Error installing wrought_get_metadata function")?;

    linker
        .func_wrap_async(
            "env",
            "wrought_set_metadata",
            |caller: Caller<'_, CombinedContext>,
             (path_ptr, path_len, key_ptr, key_len, content_ptr, content_len): (
                i32,
                i32,
                i32,
                i32,
                i32,
                i32,
            )| {
                Box::new(async move {
                    wasm_set_metadata(
                        caller,
                        path_ptr,
                        path_len,
                        key_ptr,
                        key_len,
                        content_ptr,
                        content_len,
                    )
                    .await
                })
            },
        )
        .with_context(|| "Error installing wrought_set_metadata function")?;

    linker
        .func_wrap_async(
            "env",
            "wrought_ai_query",
            |caller: Caller<'_, CombinedContext>, (query_ptr, query_len): (i32, i32)| {
                Box::new(async move { wasm_ai_query(caller, query_ptr, query_len).await })
            },
        )
        .with_context(|| "Error installing wrought_ai_query function")?;

    linker.func_wrap_async(
            "env",
            "wrought_render_template",
            |caller: Caller<'_, CombinedContext>, (id, key_ptr, key_len, content_ptr, content_len): (i32, i32, i32, i32, i32)| {
                Box::new(async move { wasm_render_template( caller, id, key_ptr, key_len, content_ptr, content_len).await })
            },
        )
        .with_context(|| "Error installing wrought_render_template function")?;

    linker
        .func_wrap_async(
            "env",
            "wrought_drop_template",
            |caller: Caller<'_, CombinedContext>, (id,): (i32,)| {
                Box::new(async move { wasm_drop_template(caller, id).await })
            },
        )
        .with_context(|| "Error installing wrought_drop_template function")?;

    linker
        .func_wrap_async(
            "env",
            "wrought_init_template",
            |caller: Caller<'_, CombinedContext>, _params: ()| {
                Box::new(async move { wasm_init_template(caller).await })
            },
        )
        .with_context(|| "Error installing wrought_drop_template function")?;

    linker
        .func_wrap_async(
            "env",
            "wrought_add_templates",
            |caller: Caller<'_, CombinedContext>,
             (id, encoded_templates_ptr, len): (i32, i32, i32)| {
                Box::new(
                    async move { wasm_add_templates(caller, id, encoded_templates_ptr, len).await },
                )
            },
        )
        .with_context(|| "Error installing wrought_add_templates function")?;

    let errors = Arc::new(Mutex::new(Vec::new()));
    let errors_clone = errors.clone();

    linker.func_wrap_async(
        "env",
        "host_report_error",
        move |mut caller: Caller<'_, _>, (error_type, ptr, len): (i32, i32, i32)| {
            let errors_clone = errors_clone.clone();
            Box::new(async move {
                let memory = caller.get_export("memory").unwrap().into_memory().unwrap();
                let data = memory.data(&caller)[ptr as usize..(ptr + len) as usize].to_vec();
                let error = String::from_utf8(data).unwrap();
                let error = match error_type {
                    ERROR_TYPE_NORMAL => WasmError::Normal(error),
                    ERROR_TYPE_PANIC => WasmError::Panic(error),
                    _ => WasmError::Normal(format!("Unknown error type: {}", error)),
                };
                errors_clone.lock().unwrap().push(error);
            })
        },
    )?;

    f(&linker).with_context(|| "Error installing utility functions")?;

    // Instantiate our wasm module.
    // Note: This is a module built against the preview1 WASI API.
    let mut reader = fs
        .lock()
        .unwrap()
        .reader(script_path)
        .with_context(|| format!("Error reading script file {:?}", script_path))?;
    let mut content = vec![];
    reader.read_to_end(&mut content)?;
    let module = Module::new(&engine, &content)?;

    let instance = linker
        .instantiate_async(&mut store, &module)
        .await
        .with_context(|| "Error instantiating WASM engine instance")?;
    let func = instance
        .get_typed_func::<(), i32>(&mut store, "plugin")
        .with_context(|| "Unable to load plugin function")?;
    let result = func.call_async(&mut store, ()).await;

    match result {
        Ok(0) => {}
        Ok(_) => {
            // Handle normal error(s)
            let errors = errors.lock().unwrap();
            if !errors.is_empty() {
                let error_msgs: Vec<String> = errors
                    .iter()
                    .map(|e| match e {
                        WasmError::Normal(msg) => format!("Error: {}", msg),
                        WasmError::Panic(msg) => format!("Panic: {}", msg),
                    })
                    .collect();
                anyhow::bail!("WASM execution failed:\n{}", error_msgs.join("\n"));
            } else {
                anyhow::bail!("WASM execution failed with unknown error");
            }
        }
        Err(trap) => {
            // Handle trap (like panics)
            let errors = errors.lock().unwrap();
            if !errors.is_empty() {
                let error_msgs: Vec<String> = errors
                    .iter()
                    .map(|e| match e {
                        WasmError::Normal(msg) => format!("Error: {}", msg),
                        WasmError::Panic(msg) => format!("Panic: {}", msg),
                    })
                    .collect();
                anyhow::bail!("WASM execution trapped:\n{}", error_msgs.join("\n"));
            } else {
                anyhow::bail!("WASM execution trapped: {}", trap);
            }
        }
    }

    println!(
        "WASM STDOUT\n{}",
        String::from_utf8_lossy(stdout_buffer.lock().unwrap().as_slice())
    );
    println!(
        "WASM STDERR\n{}",
        String::from_utf8_lossy(stderr_buffer.lock().unwrap().as_slice())
    );

    Ok(())
}
