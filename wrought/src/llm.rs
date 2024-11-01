// Wrappers for the rust_openai stuff

use std::{
    path::Path,
    sync::{Arc, Mutex},
};

use anyhow::bail;
use async_trait::async_trait;
use rust_openai::types::{ChatRequest, SystemMessage};

pub type AsyncMutex<T> = tokio::sync::Mutex<T>;

#[async_trait]
pub trait LLM {
    async fn query(&mut self, query: &str) -> anyhow::Result<String>;
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
    pub async fn create_with_key(
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
}

#[async_trait]
impl LLM for OpenAILLM {
    async fn query(&mut self, query: &str) -> anyhow::Result<String> {
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

pub struct InvalidLLM {
    error_message: String,
}

impl InvalidLLM {
    pub(crate) fn create_with_error_message<T: Into<String>>(error_message: T) -> InvalidLLM {
        InvalidLLM {
            error_message: error_message.into(),
        }
    }
}

#[async_trait]
impl LLM for InvalidLLM {
    async fn query(&mut self, _query: &str) -> anyhow::Result<String> {
        bail!("Unable to access LLM: {}", self.error_message)
    }
}
