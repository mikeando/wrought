// Wrappers for the rust_openai stuff

use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    thread::JoinHandle,
};

use anyhow::bail;
use async_trait::async_trait;
use rust_openai::types::{ChatRequest, SystemMessage};
use xfs::Xfs;

type AsyncMutex<T> = tokio::sync::Mutex<T>;

// Our big problem is that the AI library we use uses async, but we dont want that in
// our interface. Mostly because async + lua and async + WASM both add some complexities.
// So what we do is create a worker thread for handling AI requests,
// give it its own tokio threadpool for its workers and our synchronous code just posts stuff to that
// main worker via channels.

enum AiWorkRequest {
    Query(AiWorkQueryRequest),
}

struct AiWorkQueryRequest {
    query: String,
    response_channel: tokio::sync::oneshot::Sender<AiQueryResponse>,
}

struct AiQueryResponse {
    result: anyhow::Result<String>,
}

pub struct AiSettings {
    cache_dir: PathBuf,
    openai_api_key: String,
    fs: Arc<Mutex<dyn Xfs + Send>>,
}

pub struct AiWorker {
    llm: rust_openai::request::OpenAILLM,
    rx: tokio::sync::mpsc::Receiver<AiWorkRequest>,
}

pub async fn run_as_worker_query_internal(
    worker: &mut AiWorker,
    query: &str,
) -> anyhow::Result<String> {
    let messages = vec![SystemMessage::new(query).into()];
    let request = ChatRequest::new(rust_openai::types::ModelId::Gpt4oMini, messages);
    let (response, _) = worker.llm.make_request(&request).await?;
    let result = response.choices[0]
        .message
        .as_assistant_message()
        .as_ref()
        .unwrap()
        .content
        .as_ref()
        .unwrap()
        .clone();
    Ok(result)
}

async fn run_ai_worker_query(
    worker: &mut AiWorker,
    query: AiWorkQueryRequest,
) -> anyhow::Result<()> {
    // Note we dont ues ? here as we want to forward failures down the channel.
    let result = run_as_worker_query_internal(worker, &query.query).await;
    query
        .response_channel
        .send(AiQueryResponse { result })
        .map_err(|_| anyhow::anyhow!("unable to send response"))?;
    Ok(())
}

async fn run_ai_worker(
    settings: AiSettings,
    rx: tokio::sync::mpsc::Receiver<AiWorkRequest>,
) -> anyhow::Result<()> {
    let requester = rust_openai::request::OpenAIRawRequester {
        openai_api_key: settings.openai_api_key,
    };
    let requester = Arc::new(AsyncMutex::new(requester));

    let fs_wrapper = OpenAIFsStub { fs: settings.fs };
    let fs_wrapper = Arc::new(AsyncMutex::new(fs_wrapper));

    let cache =
        rust_openai::request::DefaultRequestCache::new(fs_wrapper, settings.cache_dir).await?;
    let cache = Arc::new(AsyncMutex::new(cache));

    let llm = rust_openai::request::OpenAILLM::new(requester, cache);
    let mut worker = AiWorker { rx, llm };

    while let Some(request) = worker.rx.recv().await {
        match request {
            AiWorkRequest::Query(query) => {
                run_ai_worker_query(&mut worker, query).await?;
            }
        };
    }
    Ok(())
}

fn start_ai_workers(
    settings: AiSettings,
) -> (
    tokio::sync::mpsc::Sender<AiWorkRequest>,
    JoinHandle<anyhow::Result<()>>,
) {
    // Create a channel and get the sync receiver
    let (tx, rx) = tokio::sync::mpsc::channel::<AiWorkRequest>(32);

    let jh = std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(run_ai_worker(settings, rx))
    });
    (tx, jh)
}

pub struct OpenAILLM {
    channel: tokio::sync::mpsc::Sender<AiWorkRequest>,
    join_handle: std::thread::JoinHandle<anyhow::Result<()>>,
}

impl OpenAILLM {
    pub fn create_with_key(
        openai_api_key: String,
        fs: Arc<Mutex<dyn xfs::Xfs + Send>>,
        cache_dir: PathBuf,
    ) -> anyhow::Result<OpenAILLM> {
        // This is messy...
        let settings = AiSettings {
            cache_dir,
            openai_api_key,
            fs,
        };

        let (channel, join_handle) = start_ai_workers(settings);

        Ok(OpenAILLM {
            channel,
            join_handle,
        })
    }
}

impl LLM for OpenAILLM {
    fn query(&mut self, query: &str) -> anyhow::Result<String> {
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();
        let request = AiWorkRequest::Query(AiWorkQueryRequest {
            query: query.to_string(),
            response_channel: response_tx,
        });
        self.channel.blocking_send(request)?;

        // Wait for response synchronously
        let response = response_rx.blocking_recv().unwrap();
        response.result
    }
}

pub trait LLM {
    fn query(&mut self, query: &str) -> anyhow::Result<String>;
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

impl LLM for InvalidLLM {
    fn query(&mut self, _query: &str) -> anyhow::Result<String> {
        bail!("Unable to access LLM: {}", self.error_message)
    }
}
