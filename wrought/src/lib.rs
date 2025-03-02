use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context};
use serde::{Deserialize, Serialize};
use xfs::Xfs;

pub mod backend;
pub mod binary16;
pub mod bridge;
pub mod content_store;
pub mod event_log;
pub mod events;
pub mod file_history;
pub mod fs_utils;
pub mod kb;
pub mod llm;
pub mod luau_bridge;
pub mod metadata;
pub mod project_status;
pub mod wasm_bridge;
pub mod package;
