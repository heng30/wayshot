use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Default, Clone, Debug)]
pub struct StreamTextItem {
    pub id: u64,
    pub text: Option<String>,
    pub reasoning_text: Option<String>,
    pub etext: Option<String>,
    pub finished: bool,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct ChunkChoice {
    pub index: usize,
    pub delta: HashMap<String, Option<String>>,
    pub finish_reason: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct ChatCompletionChunk {
    pub id: String,

    #[serde(default)]
    pub object: String,

    pub created: i64,
    pub model: String,
    pub choices: Vec<ChunkChoice>,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct Error {
    pub error: HashMap<String, String>,
}
