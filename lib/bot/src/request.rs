use serde::{Deserialize, Serialize};

#[derive(Default, Clone, Debug)]
pub struct HistoryChat {
    pub utext: String,
    pub btext: String,
}

#[derive(Serialize, Deserialize, Default, Debug, Clone)]
pub struct APIConfig {
    pub api_base_url: String,
    pub api_model: String,
    pub api_key: String,
    pub temperature: Option<f32>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct ChatCompletion {
    pub messages: Vec<Message>,
    pub model: String,
    pub stream: bool,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct Message {
    pub role: String,
    pub content: String,
}
