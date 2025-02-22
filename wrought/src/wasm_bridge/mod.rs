pub mod io;
pub mod ai;
pub mod templates;

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use anyhow::Context;
use wasmtime::{Caller, Config, Engine, Linker, Module, Store};
use wasmtime_wasi::preview1::{self, WasiP1Ctx};
use wasmtime_wasi::WasiCtxBuilder;

use crate::bridge::Bridge;
use crate::llm::AiMessage;

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
    pub bridge: Arc<Mutex<dyn Bridge + Send + 'static>>,
    pub templating: BTreeMap<i32, tera::Tera>,
    pub next_template_id: i32,
    pub ai_messages: BTreeMap<i32, AiMessage>,
    pub next_ai_message_id: i32,
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

pub fn run_script(
    bridge: Arc<Mutex<dyn Bridge + Send + 'static>>,
    fs: Arc<Mutex<dyn xfs::Xfs>>,
    script_path: &Path,
) -> anyhow::Result<()> {
    run_script_ex(bridge, fs, script_path, |_| Ok(()))
}


// The additional F function is used to add hooks when testing
pub fn run_script_ex<F>(
    bridge: Arc<Mutex<dyn Bridge + Send + 'static>>,
    fs: Arc<Mutex<dyn xfs::Xfs>>,
    script_path: &Path,
    f: F,
) -> anyhow::Result<()>
where
    F: FnOnce(&Linker<CombinedContext>) -> anyhow::Result<()>,
{
    let config = Config::new();
    // config.async_support(true);
    let engine = Engine::new(&config).with_context(|| "error creating wasm context")?;
    let stdout_buffer = Arc::new(Mutex::new(vec![]));
    let stderr_buffer = Arc::new(Mutex::new(vec![]));

    // Add the WASI preview1 API to the linker (will be implemented in terms of
    // the preview2 API)
    let mut linker: Linker<CombinedContext> = Linker::new(&engine);
    preview1::add_to_linker_sync(&mut linker, |t| &mut t.1)
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
        ai_messages: BTreeMap::new(),
        next_ai_message_id: 0,
        call_buffer: wasmcb::CallBuffer::new(),
    };

    let mut store = Store::new(&engine, CombinedContext(app_state, wasi_ctx));
    wasmcb::add_to_linker(&mut linker)?;

    io::reigister_functions(&mut linker)?;
    ai::reigister_functions(&mut linker)?;
    templates::reigister_functions(&mut linker)?;

    let errors = Arc::new(Mutex::new(Vec::new()));
    let errors_clone = errors.clone();

    linker.func_wrap(
        "env",
        "host_report_error",
        move |mut caller: Caller<'_, _>, error_type: i32, ptr: i32, len: i32| {
            let errors_clone = errors_clone.clone();
            let memory = caller.get_export("memory").unwrap().into_memory().unwrap();
            let data = memory.data(&caller)[ptr as usize..(ptr + len) as usize].to_vec();
            let error = String::from_utf8(data).unwrap();
            let error = match error_type {
                ERROR_TYPE_NORMAL => WasmError::Normal(error),
                ERROR_TYPE_PANIC => WasmError::Panic(error),
                _ => WasmError::Normal(format!("Unknown error type: {}", error)),
            };
            errors_clone.lock().unwrap().push(error);
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
        .instantiate(&mut store, &module)
        .with_context(|| "Error instantiating WASM engine instance")?;
    let func = instance
        .get_typed_func::<(), i32>(&mut store, "plugin")
        .with_context(|| "Unable to load plugin function")?;
    let result = func.call(&mut store, ());

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