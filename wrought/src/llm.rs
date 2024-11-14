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
    Message(AiWorkMessageRequest),
}

struct AiWorkQueryRequest {
    query: String,
    response_channel: tokio::sync::oneshot::Sender<AiQueryResponse>,
}

struct AiQueryResponse {
    result: anyhow::Result<String>,
}

struct AiWorkMessageRequest {
    message: AiMessage,
    response_channel: tokio::sync::oneshot::Sender<AiMessageResponse>,
}

struct AiMessageResponse {
    result: anyhow::Result<serde_json::Value>,
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

async fn run_ai_worker_message_internal(
    worker: &mut AiWorker,
    message: &AiMessage,
) -> anyhow::Result<serde_json::Value> {
    let mut messages = vec![];
    if let Some(system_message) = &message.system_message {
        messages.push(rust_openai::types::Message::system_message(system_message))
    }
    for m in &message.user_messages {
        messages.push(rust_openai::types::Message::user_message(m))
    }
    let request: rust_openai::types::ChatRequest =
        rust_openai::types::ChatRequest::new(rust_openai::types::ModelId::Gpt4oMini, messages)
            .with_tools(vec![message.tool.clone().unwrap()]);
    // .with_max_tokens(Some(ai_options.max_tokens));

    let (response, _) = worker.llm.make_request(&request).await?;

    let result: serde_json::Value = serde_json::from_str(
        &response.choices[0]
            .message
            .as_assistant_message()
            .as_ref()
            .unwrap()
            .tool_calls
            .as_ref()
            .unwrap()[0]
            .function
            .arguments,
    )?;
    Ok(result)
}

async fn run_ai_worker_message(
    worker: &mut AiWorker,
    request: AiWorkMessageRequest,
) -> anyhow::Result<()> {
    // Note we dont ues ? here as we want to forward failures down the channel.
    let result = run_ai_worker_message_internal(worker, &request.message).await;
    request
        .response_channel
        .send(AiMessageResponse { result })
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
            AiWorkRequest::Message(message) => {
                run_ai_worker_message(&mut worker, message).await?;
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

    fn send_ai_message(&mut self, message: &AiMessage) -> anyhow::Result<serde_json::Value> {
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();
        let request = AiWorkRequest::Message(AiWorkMessageRequest {
            message: message.clone(),
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
    fn send_ai_message(&mut self, message: &AiMessage) -> anyhow::Result<serde_json::Value>;
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

    fn send_ai_message(&mut self, _message: &AiMessage) -> anyhow::Result<serde_json::Value> {
        bail!("Unable to access LLM: {}", self.error_message)
    }
}

#[derive(Default, Clone)]
pub struct AiMessage {
    system_message: Option<String>,
    user_messages: Vec<String>,
    tool: Option<rust_openai::types::Tool>,
}

impl AiMessage {
    pub(crate) fn set_system_message(&mut self, system_message: &str) -> anyhow::Result<()> {
        self.system_message = Some(system_message.to_string());
        Ok(())
    }

    pub(crate) fn add_user_message(&mut self, user_message: &str) -> anyhow::Result<()> {
        self.user_messages.push(user_message.to_string());
        Ok(())
    }

    pub(crate) fn add_tool(
        &mut self,
        tool_description: &str,
        tool_name: &str,
        tool_schema: &str,
    ) -> anyhow::Result<()> {
        let schema_value: serde_json::Value = serde_json::from_str(tool_schema)?;
        let schema = rust_openai::types::JSONSchema(schema_value);
        self.tool = Some(rust_openai::types::Tool {
            description: Some(tool_description.to_string()),
            name: tool_name.to_string(),
            parameters: Some(schema),
        });
        Ok(())
    }
}
