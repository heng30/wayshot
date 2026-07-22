//! Configuration structures for LFM2.5-VL

use candle_nn::Activation;

// ── LFM2 text model config ──────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
pub struct Lfm2Config {
    pub block_auto_adjust_ff_dim: bool,
    pub block_dim: usize,
    pub block_ff_dim: usize,
    pub block_ffn_dim_multiplier: f64,
    pub block_mlp_init_scale: f64,
    pub block_multiple_of: usize,
    pub block_norm_eps: f64,
    pub block_out_init_scale: f64,
    pub block_use_swiglu: bool,
    pub block_use_xavier_init: bool,
    pub bos_token_id: Option<u32>,
    #[serde(rename = "conv_L_cache")]
    pub conv_l_cache: usize,
    pub conv_bias: bool,
    pub conv_dim: usize,
    pub conv_dim_out: Option<usize>,
    pub conv_use_xavier_init: bool,
    pub eos_token_id: u32,
    pub full_attn_idxs: Option<Vec<usize>>,
    pub layer_types: Option<Vec<String>>,
    pub hidden_size: usize,
    pub initializer_range: f64,
    pub intermediate_size: Option<usize>,
    pub max_position_embeddings: usize,
    pub model_type: String,
    pub norm_eps: f64,
    pub num_attention_heads: usize,
    pub num_heads: usize,
    pub num_hidden_layers: usize,
    pub num_key_value_heads: usize,
    pub pad_token_id: Option<u32>,
    pub rope_theta: Option<f32>,
    pub rope_parameters: Option<RopeParameters>,
    pub torch_dtype: Option<String>,
    pub dtype: Option<String>,
    pub use_cache: bool,
    pub use_pos_enc: bool,
    pub vocab_size: usize,
    pub tie_embedding: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
pub struct RopeParameters {
    pub rope_theta: f32,
    pub rope_type: String,
}

impl Lfm2Config {
    pub fn get_layer_types(&self) -> crate::error::Result<Vec<String>> {
        if let Some(types) = &self.layer_types {
            Ok(types.clone())
        } else if let Some(idx) = &self.full_attn_idxs {
            let mut layer_types = vec![];
            for i in 0..self.num_hidden_layers {
                if idx.contains(&i) {
                    layer_types.push("full_attention".to_string());
                } else {
                    layer_types.push("conv".to_string());
                }
            }
            Ok(layer_types)
        } else {
            return Err(crate::error::LfmError::Config("layer_types and full_attn_idxs cannot both be None".into()));
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
pub struct Lfm2GenerateConfig {
    pub bos_token_id: u32,
    pub eos_token_id: u32,
    pub pad_token_id: u32,
}

// ── LFM2.5-VL config ────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
pub struct Lfm2VLConfig {
    pub do_image_splitting: bool,
    pub downsample_factor: usize,
    pub dtype: String,
    pub encoder_patch_size: usize,
    pub image_token_id: u32,
    pub max_image_tokens: usize,
    pub max_pixels_tolerance: f64,
    pub max_tiles: usize,
    pub min_image_tokens: usize,
    pub min_tiles: usize,
    pub model_type: String,
    pub projector_bias: bool,
    pub projector_hidden_act: Activation,
    pub projector_hidden_size: usize,
    pub projector_use_layernorm: Option<bool>,
    pub text_config: Lfm2Config,
    pub tile_size: usize,
    pub use_image_special_tokens: bool,
    pub use_thumbnail: bool,
    pub vision_config: Lfm2VLVisionConfig,
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
pub struct Lfm2VLVisionConfig {
    pub attention_dropout: f64,
    pub dtype: String,
    pub hidden_act: Activation,
    pub hidden_size: usize,
    pub intermediate_size: usize,
    pub layer_norm_eps: f64,
    pub model_type: String,
    pub num_attention_heads: usize,
    pub num_channels: usize,
    pub num_hidden_layers: usize,
    pub num_patches: usize,
    pub patch_size: usize,
    pub vision_use_head: bool,
}

// ── Processor config ────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
pub struct Lfm2ImageConfig {
    pub do_image_splitting: bool,
    pub do_normalize: bool,
    pub do_pad: bool,
    pub do_rescale: bool,
    pub do_resize: bool,
    pub downsample_factor: usize,
    pub encoder_patch_size: usize,
    pub image_mean: Vec<f64>,
    pub image_std: Vec<f64>,
    pub max_image_tokens: usize,
    pub max_num_patches: usize,
    pub max_pixels_tolerance: f64,
    pub max_tiles: usize,
    pub min_image_tokens: usize,
    pub min_tiles: usize,
    pub resample: usize,
    pub rescale_factor: f64,
    pub size: Size,
    pub tile_size: usize,
    pub use_thumbnail: bool,
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
pub struct Size {
    pub height: usize,
    pub width: usize,
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
pub struct Lfm2ProcessorConfig {
    pub image_processor: Lfm2ImageConfig,
}
