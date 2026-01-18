use candle_nn::Activation;
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Qwen3Config {
    pub attention_bias: bool,
    pub attention_dropout: f64,
    pub bos_token_id: u32,
    pub eos_token_id: u32,
    pub head_dim: usize,
    pub hidden_act: Activation,
    pub hidden_size: usize,
    pub initializer_range: f64,
    pub intermediate_size: usize,
    pub max_position_embeddings: usize,
    pub max_window_layers: usize,
    pub num_attention_heads: usize,
    pub num_hidden_layers: usize,
    pub num_key_value_heads: usize,
    pub rms_norm_eps: f64,
    pub rope_theta: f32,
    pub tie_word_embeddings: bool,
    pub torch_dtype: String,
    pub use_cache: bool,
    pub use_sliding_window: bool,
    pub vocab_size: usize,
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
pub struct Qwen3GenerationConfig {
    pub bos_token_id: usize,
    pub pad_token_id: usize,
    pub do_sample: bool,
    pub eos_token_id: Vec<usize>,
    pub top_p: f32,
    pub top_k: usize,
    pub temperature: f32,
    #[serde(default = "default_repetition_penalty")]
    pub repetition_penalty: f32,
}

fn default_repetition_penalty() -> f32 {
    1.0
}
