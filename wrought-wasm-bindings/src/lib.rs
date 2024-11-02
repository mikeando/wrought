pub type WroughtResult<T> = Result<T, String>;

#[cfg(not(feature = "host"))]
mod client {
    use serde::Serialize;

    use super::*;
    use std::path::Path;

    // Declare the extern functions that will be provided by the host
    #[link(wasm_import_module = "env")]
    extern "C" {
        fn wrought_write_file(
            path_ptr: *const u8,
            path_len: usize,
            content_ptr: *const u8,
            content_len: usize,
        );
        fn wrought_read_file(path_ptr: *const u8, path_len: usize);
        fn wrought_get_metadata(
            path_ptr: *const u8,
            path_len: usize,
            key_ptr: *const u8,
            key_len: usize,
        );
        fn wrought_set_metadata(
            path_ptr: *const u8,
            path_len: usize,
            key_ptr: *const u8,
            key_len: usize,
            content_ptr: *const u8,
            content_len: usize,
        );
        fn wrought_ai_query(query_ptr: *const u8, query_len: usize);

        // TODO: Expose these in the Bridge
        fn wrought_init_template();
        fn wrought_drop_template(id: i32);
        fn wrought_add_templates(id: i32, encoded_templates_ptr: *const u8, len: usize);
        fn wrought_render_template(id: i32, key_ptr: *const u8, key_len: usize, content_ptr: *const u8, content_len: usize);
    }

    pub struct Wrought {}

    impl Wrought {
        pub fn write_file(&mut self, path: &Path, value: &[u8]) -> WroughtResult<()> {
            let path = format!("{}", path.display());
            let path_buf = path.as_bytes();
            let len = unsafe {
                wrought_write_file(
                    path_buf.as_ptr(),
                    path_buf.len(),
                    value.as_ptr(),
                    value.len(),
                );
                wasmcb::get_call_buffer_len()
            };
            let mut out_buf = vec![0u8; len];
            unsafe {
                wasmcb::read_call_buffer(out_buf.as_mut_ptr(), out_buf.len());
            }
            serde_json::from_slice(&out_buf).unwrap()
        }

        pub fn read_file(&mut self, path: &Path) -> WroughtResult<Option<Vec<u8>>> {
            let path = format!("{}", path.display());
            let path_buf = path.as_bytes();
            let len = unsafe {
                wrought_read_file(path_buf.as_ptr(), path_buf.len());
                wasmcb::get_call_buffer_len()
            };
            let mut out_buf = vec![0u8; len];
            unsafe {
                wasmcb::read_call_buffer(out_buf.as_mut_ptr(), out_buf.len());
            }
            serde_json::from_slice(&out_buf).unwrap()
        }

        pub fn get_metadata(&mut self, path: &Path, key: &str) -> WroughtResult<Option<String>> {
            let path = format!("{}", path.display());
            let path_buf = path.as_bytes();
            let key_buf = key.as_bytes();
            let len = unsafe {
                wrought_get_metadata(
                    path_buf.as_ptr(),
                    path_buf.len(),
                    key_buf.as_ptr(),
                    key_buf.len(),
                );
                wasmcb::get_call_buffer_len()
            };
            let mut out_buf = vec![0u8; len];
            unsafe {
                wasmcb::read_call_buffer(out_buf.as_mut_ptr(), out_buf.len());
            }
            serde_json::from_slice(&out_buf).unwrap()
        }

        pub fn set_metadata(&mut self, path: &Path, key: &str, value: &str) -> WroughtResult<()> {
            let path = format!("{}", path.display());
            let path_buf = path.as_bytes();
            let key_buf = key.as_bytes();
            let value_buf = value.as_bytes();
            let len = unsafe {
                wrought_set_metadata(
                    path_buf.as_ptr(),
                    path_buf.len(),
                    key_buf.as_ptr(),
                    key_buf.len(),
                    value_buf.as_ptr(),
                    value_buf.len(),
                );
                wasmcb::get_call_buffer_len()
            };
            let mut out_buf = vec![0u8; len];
            unsafe {
                wasmcb::read_call_buffer(out_buf.as_mut_ptr(), out_buf.len());
            }
            serde_json::from_slice(&out_buf).unwrap()
        }

        pub fn ai_query(&mut self, query: &str) -> WroughtResult<String> {
            let query_buf = query.as_bytes();
            let len = unsafe {
                wrought_ai_query(query_buf.as_ptr(), query_buf.len());
                wasmcb::get_call_buffer_len()
            };
            let mut out_buf = vec![0u8; len];
            unsafe {
                wasmcb::read_call_buffer(out_buf.as_mut_ptr(), out_buf.len());
            }
            serde_json::from_slice(&out_buf).unwrap()
        }

        pub fn template(&mut self) -> WroughtResult<WroughtTemplate> {
            let len = unsafe {
                wrought_init_template();
                wasmcb::get_call_buffer_len()
            };
            let mut out_buf = vec![0u8; len];
            unsafe {
                wasmcb::read_call_buffer(out_buf.as_mut_ptr(), out_buf.len());
            }
            let result: WroughtResult<i32> = serde_json::from_slice(&out_buf).unwrap();
            Ok(WroughtTemplate { id: result? })
        }

    }

    pub struct WroughtTemplate {
        id: i32,
    }

    impl WroughtTemplate {

        pub fn add_template(&mut self, key: &str, template: &str) -> WroughtResult<()> {
            self.add_templates(&[(key, template)])

        }

        pub fn add_templates(&mut self, templates: &[(&str, &str)]) -> WroughtResult<()> {
            let templates_json = serde_json::to_vec(templates).map_err(|e| e.to_string())?;
            let len = unsafe {
                wrought_add_templates(self.id, templates_json.as_ptr(), templates_json.len());
                wasmcb::get_call_buffer_len()
            };
            let mut out_buf = vec![0u8; len];
            unsafe {
                wasmcb::read_call_buffer(out_buf.as_mut_ptr(), out_buf.len());
            }
            serde_json::from_slice(&out_buf).unwrap()
        }

        pub fn render_template(&self, key: &str, values: &impl Serialize) -> WroughtResult<String> {
            let content_json = serde_json::to_vec(values).map_err(|e| e.to_string())?;
            let key_buf = key.as_bytes();
            let len = unsafe {
                wrought_render_template(self.id, key_buf.as_ptr(), key_buf.len(), content_json.as_ptr(), content_json.len());
                wasmcb::get_call_buffer_len()
            };
            let mut out_buf = vec![0u8; len];
            unsafe {
                wasmcb::read_call_buffer(out_buf.as_mut_ptr(), out_buf.len());
            }
            serde_json::from_slice(&out_buf).unwrap()
        }
    }

    impl Drop for WroughtTemplate {
        fn drop(&mut self) {
            unsafe {
                wrought_drop_template(self.id);
            }
        }
    }

}



#[cfg(not(feature = "host"))]
pub use client::*;
