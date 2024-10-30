use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use wasmcb::{default_panic_hook, report_error};
use wrought_wasm_bindings::Wrought;

#[derive(Serialize, Deserialize)]
struct DemoStruct {
    call_count: u32,
}

#[no_mangle]
pub extern "C" fn plugin() -> i32 {
    std::panic::set_hook(Box::new(default_panic_hook));

    return match plugin_impl() {
        Ok(()) => 0,
        Err(e) => {
            report_error(&e.to_string());
            -1
        }
    };
}

fn plugin_impl() -> anyhow::Result<()> {
    let mut wrought = Wrought {};

    println!("In the demo script");
    let demo_path = PathBuf::from("demo.json");
    let demo_content = wrought.read_file(&demo_path).unwrap();
    let mut demo: DemoStruct = match demo_content {
        Some(demo_content) => serde_json::from_slice(&demo_content).unwrap(),
        None => DemoStruct { call_count: 0 },
    };

    demo.call_count += 1;

    wrought
        .write_file(&demo_path, &serde_json::to_vec(&demo).unwrap())
        .unwrap();

    wrought
        .set_metadata(&demo_path, "some_metatdata", "hello")
        .unwrap();

    //TODO: Try both ai_query and get_metadata here
    let story = wrought.ai_query("Tell me a fun story").unwrap();
    wrought
        .write_file(&PathBuf::from("story.md"), story.as_bytes())
        .unwrap();

    println!("WASM DONE");

    // panic!("BANG");
    // anyhow::bail!("NOPE");
    return Ok(());
}
