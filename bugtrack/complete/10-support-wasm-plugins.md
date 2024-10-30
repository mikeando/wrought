# Add support fro WASM plugins

Writing plugins in lua is nice, but it'd be nicer to
have all the features of rust available to use.

If we compile rust to a WASM plugin we get lovely things like
serde JSON support and Tera templating.

Some of these will need to be moved into the core eventually, 
but this way we can have them running in the plugin

## Status: Done

## Problem 

## Design

## Issue log

* [X] Using a block_on inside a block_on causes a panic. This happens if you try 
      to use the LLM inside a WASM plugin.

      To fix this we need to make a lot of things async.

* [X] Initial wasmcb didn't help provide error reporting/handling for `Result<()>`
      and `panic!()`. Now it does.

* [X] We initially used the name "main" for the plugin exported function. But if you have 
      a main that returns an int, the compiler silently converts it to `i32 main(i32,i32)`
      which we then fail to load. Just pick another name instead - `plugin`.

* [-] Allow more than one exported function per wasm file. 
      For now this is moved to `18-wasm-multifunction.md`