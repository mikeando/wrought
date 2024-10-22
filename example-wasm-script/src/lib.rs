use std::path::PathBuf;
use wrought_wasm_bindings::Wrought;

#[no_mangle]
pub extern "C" fn main() {
    let mut wrought = Wrought {};

    println!("In the init script");

    // TODO: Handle errors
    let _ = wrought.write_file(
        &PathBuf::from("outline.md"),
        [
            "Fill this in with an outline of your document",
            "",
            "* SOMETHING",
            "",
            "* SOMETHING ELSE",
            "",
        ]
        .join("\n")
        .as_bytes(),
    );

    // TODO: Handle errors
    let _ = wrought.write_file(
        &PathBuf::from(".wrought/packages/test/status/01_init.toml"),
        [
            "title=\"Setup the init\"",
            r#"status=""""#,
            "Setup the init",
            r#"""""#,
            r#"next_steps=["setup the init then do the thing"]"#,
            "",
        ]
        .join("\n")
        .as_bytes(),
    );
}
