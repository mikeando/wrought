// Wrappers for the rust_openai stuff 

use std::{path::Path, sync::{Arc, Mutex}};

pub trait LLM {
    fn query(&mut self, query: &str) -> anyhow::Result<String>;
}

pub struct OpenAILLM {
    llm: rust_openai::request::OpenAILLM;
}

pub struct OpenAIFsStub{
    fs: Arc<Mutex<dyn xfs::Xfs>>
}

impl OpenAILLM {
    pub fn create_with_key(
        openai_api_key: String, 
        fs: Arc<Mutex<dyn xfs::Xfs>>,
        cache_dir: &Path,
    ) -> OpenAILLM {
        let requester = rust_openai::request::OpenAIRawRequester { openai_api_key };

        // This is messy...
        let requester = Arc::new(AsyncMutex::new(requester));

        let fs_wrapper = OpenAIFsStub { fs };
        let fs_wrapper = Arc::new(AsyncMutex::new(fs_wrapper));
        let cache = rust_openai::request::DefaultRequestCache::new(
            fs_wrapper,
            cache_dir,
        ).await?;
        let cache = Arc::new(AsyncMutex::new(cache));

        let llm = rust_openai::request::OpenAILLM::new(requester, cache);
        OpenAILLM{llm}
    }
}

impl LLM for OpenAILLM {
    fn query(&mut self, query: &str) -> anyhow::Result<String> {
        todo!()
    }
}