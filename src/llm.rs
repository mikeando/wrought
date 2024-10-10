// Wrappers for the rust_openai stuff

use std::{
    future::Future,
    path::Path,
    sync::{Arc, Mutex},
};

use async_trait::async_trait;
use rust_openai::types::{ChatRequest, SystemMessage};

pub type AsyncMutex<T> = tokio::sync::Mutex<T>;

pub trait LLM {
    fn query(&mut self, query: &str) -> anyhow::Result<String>;
}

pub struct OpenAILLM {
    llm: rust_openai::request::OpenAILLM,
}

pub struct OpenAIFsStub {
    fs: Arc<Mutex<dyn xfs::Xfs + Send>>,
}

#[async_trait]
impl rust_openai::request::TrivialFS for OpenAIFsStub {
    async fn read_to_string(&self, p: &Path) -> anyhow::Result<String> {
        let mut reader = self.fs.lock().unwrap().reader(p)?;
        let mut result = String::new();
        reader.read_to_string(&mut result)?;
        Ok(result)
    }

    async fn write(&self, p: &Path, value: &str) -> anyhow::Result<()> {
        let mut writer = self.fs.lock().unwrap().writer(p)?;
        writer.write_all(value.as_bytes())?;
        Ok(())
    }

    async fn path_type(&self, p: &Path) -> anyhow::Result<rust_openai::request::TrivialFSPathType> {
        use anyhow::anyhow;
        match self.fs.lock().unwrap().metadata(p) {
            Ok(md) => {
                if md.is_dir() {
                    Ok(rust_openai::request::TrivialFSPathType::Directory)
                } else if md.is_file() {
                    Ok(rust_openai::request::TrivialFSPathType::File)
                } else {
                    Err(anyhow!(
                        "OpenAIFsStub::path_type - invalid path type for '{}'",
                        p.display()
                    ))
                }
            }
            Err(xfs::XfsError::FileNotFound(_, _, _)) => {
                Ok(rust_openai::request::TrivialFSPathType::NoSuchPath)
            }
            Err(e) => Err(anyhow!(
                "OpenAIFsStub::path_type - error reading '{}' : {}",
                p.display(),
                e
            )),
        }
    }
}

impl OpenAILLM {
    fn run_async<F: Future>(f: F) -> F::Output {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(f)
    }

    pub fn create_with_key(
        openai_api_key: String,
        fs: Arc<Mutex<dyn xfs::Xfs + Send>>,
        cache_dir: &Path,
    ) -> anyhow::Result<OpenAILLM> {
        Self::run_async(Self::create_with_key_async(openai_api_key, fs, cache_dir))
    }

    pub async fn create_with_key_async(
        openai_api_key: String,
        fs: Arc<Mutex<dyn xfs::Xfs + Send>>,
        cache_dir: &Path,
    ) -> anyhow::Result<OpenAILLM> {
        let requester = rust_openai::request::OpenAIRawRequester { openai_api_key };

        // This is messy...
        let requester = Arc::new(AsyncMutex::new(requester));

        let fs_wrapper = OpenAIFsStub { fs };
        let fs_wrapper = Arc::new(AsyncMutex::new(fs_wrapper));
        let cache =
            rust_openai::request::DefaultRequestCache::new(fs_wrapper, cache_dir.to_path_buf())
                .await?;
        let cache = Arc::new(AsyncMutex::new(cache));

        let llm = rust_openai::request::OpenAILLM::new(requester, cache);
        Ok(OpenAILLM { llm })
    }

    pub async fn query_async(&mut self, query: &str) -> anyhow::Result<String> {
        let messages = vec![SystemMessage::new(query).into()];
        let request = ChatRequest::new(rust_openai::types::ModelId::Gpt4oMini, messages);
        let (response, _) = self.llm.make_request(&request).await?;
        Ok(response.choices[0]
            .message
            .as_assistant_message()
            .as_ref()
            .unwrap()
            .content
            .as_ref()
            .unwrap()
            .clone())
    }
}

impl LLM for OpenAILLM {
    fn query(&mut self, query: &str) -> anyhow::Result<String> {
        Self::run_async(self.query_async(query))
    }
}
