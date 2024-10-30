pub type WroughtResult<T> = Result<T, String>;

#[cfg(not(feature = "host"))]
mod client {
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
    }
}

#[cfg(not(feature = "host"))]
pub use client::*;
