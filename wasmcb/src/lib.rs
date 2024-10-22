
#[cfg(not(feature="host"))]
mod client {

    // Declare the extern functions that will be provided by the host
    #[link(wasm_import_module = "env")]
    extern "C" {
        pub fn get_call_buffer_len() -> usize;
        pub fn read_call_buffer(buf_ptr: *mut u8, buf_len: usize);
    }
}



#[cfg(feature="host")]
mod host {

    use wasmtime::{Caller, Linker, Result};

    pub struct CallBuffer {
        pub call_buffer: Option<Result<Vec<u8>, Vec<u8>>>,
    }

    impl CallBuffer {
        pub fn new() -> CallBuffer {
            CallBuffer { call_buffer: None }
        }
    }

    pub trait ProvidesCallBuffer {
        fn get_call_buffer(&self) -> &CallBuffer;
        fn get_call_buffer_mut(&mut self) -> &mut CallBuffer;
    }

    fn wasm_get_call_buffer_len<T>(caller: Caller<'_, T>) -> i32
    where
        T: ProvidesCallBuffer
     {
        match &(caller.data().get_call_buffer().call_buffer) {
            None => panic!("wasm_get_call_buffer_len called when call_buffer is None"),
            Some(Ok(buf)) => buf.len() as i32,
            Some(Err(buf)) => buf.len() as i32,
        }
    }

    fn wasm_read_call_buffer<T>(
        mut caller: Caller<'_, T>,
        buf_ptr: i32,
        buf_len: i32,
    ) 
    where
        T: ProvidesCallBuffer
    {
        let call_data = match caller.data_mut().get_call_buffer_mut().call_buffer.take() {
            None => panic!("wasm_read_call_buffer called when call_buffer is None"),
            Some(Ok(buf)) => buf,
            Some(Err(buf)) => buf,
        };
        assert!(call_data.len() <= buf_len as usize);

        let memory = caller.get_export("memory").unwrap().into_memory().unwrap();
        let data: &mut [u8] = memory.data_mut(&mut caller);
        let buf: &mut [u8] = &mut data[buf_ptr as usize..(buf_ptr as usize + call_data.len() as usize)];
        buf.copy_from_slice(&call_data);
    }

    pub fn add_to_linker<T>(linker: &mut Linker<T>) -> Result<()> 
    where 
        T: ProvidesCallBuffer +'static
    {
        linker.func_wrap("env", "get_call_buffer_len", wasm_get_call_buffer_len)?;
        linker.func_wrap("env", "read_call_buffer", wasm_read_call_buffer)?;
        Ok(())
    }

}


#[cfg(not(feature="host"))]
pub use client::*;

#[cfg(feature="host")]
pub use host::*;
