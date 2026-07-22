//! Configuration structs matching the JSON files shipped with VoxCPM2 checkpoints.
//!
//! Field names intentionally mirror the upstream Python/pydantic configs so that
//! `config.json` files from the HuggingFace snapshots can be deserialized with
//! `serde_json::from_str` directly.

use serde::{Deserialize, Serialize};

fn default_true() -> bool {
    true
}

fn default_patch_size() -> usize {
    4
}
fn default_feat_dim() -> usize {
    64
}
fn default_residual_lm_num_layers() -> usize {
    8
}
fn default_sq_latent_dim() -> usize {
    512
}
fn default_sq_scale() -> usize {
    9
}
fn default_max_length() -> usize {
    8192
}
fn default_dtype() -> String {
    "float32".to_string()
}
fn default_device() -> String {
    "cuda".to_string()
}

// ---------------------------------------------------------------------------
// MiniCPM-4 backbone
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RopeScalingConfig {
    #[serde(rename = "type")]
    pub kind: String,
    pub long_factor: Vec<f32>,
    pub short_factor: Vec<f32>,
    pub original_max_position_embeddings: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MiniCpm4Config {
    pub bos_token_id: i64,
    pub eos_token_id: i64,
    pub hidden_size: usize,
    pub intermediate_size: usize,
    pub max_position_embeddings: usize,
    pub num_attention_heads: usize,
    pub num_hidden_layers: usize,
    pub num_key_value_heads: usize,
    pub rms_norm_eps: f32,
    pub rope_scaling: RopeScalingConfig,
    pub vocab_size: usize,

    #[serde(default = "default_true")]
    pub use_mup: bool,
    pub scale_emb: f32,
    pub dim_model_base: f32,
    pub scale_depth: f32,
    pub rope_theta: f32,

    #[serde(default)]
    pub kv_channels: Option<usize>,
    #[serde(default)]
    pub no_rope: bool,
}

impl MiniCpm4Config {
    /// Dimension of a single attention head.
    pub fn head_dim(&self) -> usize {
        self.kv_channels
            .unwrap_or(self.hidden_size / self.num_attention_heads)
    }
}

// ---------------------------------------------------------------------------
// LocDiT / CFM
// ---------------------------------------------------------------------------

fn default_sigma_min() -> f32 {
    1e-6
}
fn default_solver() -> String {
    "euler".to_string()
}
fn default_t_scheduler() -> String {
    "log-norm".to_string()
}
fn default_cfg_rate() -> f32 {
    0.1
}
fn default_inference_cfg_rate() -> f32 {
    1.0
}
fn default_reg_loss_type() -> String {
    "l1".to_string()
}
fn default_rt_range() -> (f32, f32) {
    (0.25, 0.75)
}
fn default_noise_cond_range() -> (f32, f32) {
    (0.0, 0.0)
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CfmConfig {
    #[serde(default = "default_sigma_min")]
    pub sigma_min: f32,
    #[serde(default = "default_solver")]
    pub solver: String,
    #[serde(default = "default_t_scheduler")]
    pub t_scheduler: String,
    #[serde(default = "default_cfg_rate")]
    pub training_cfg_rate: f32,
    #[serde(default = "default_inference_cfg_rate")]
    pub inference_cfg_rate: f32,
    #[serde(default = "default_reg_loss_type")]
    pub reg_loss_type: String,
    #[serde(default = "default_rt_range")]
    pub ratio_r_neq_t_range: (f32, f32),
    #[serde(default = "default_noise_cond_range")]
    pub noise_cond_prob_range: (f32, f32),
    #[serde(default)]
    pub noise_cond_scale: f32,
}

impl Default for CfmConfig {
    fn default() -> Self {
        Self {
            sigma_min: default_sigma_min(),
            solver: default_solver(),
            t_scheduler: default_t_scheduler(),
            training_cfg_rate: default_cfg_rate(),
            inference_cfg_rate: default_inference_cfg_rate(),
            reg_loss_type: default_reg_loss_type(),
            ratio_r_neq_t_range: default_rt_range(),
            noise_cond_prob_range: default_noise_cond_range(),
            noise_cond_scale: 0.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Encoder / DiT sub-configs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VoxCpmEncoderConfig {
    #[serde(default = "VoxCpmEncoderConfig::default_hidden")]
    pub hidden_dim: usize,
    #[serde(default = "VoxCpmEncoderConfig::default_ffn")]
    pub ffn_dim: usize,
    #[serde(default = "VoxCpmEncoderConfig::default_heads")]
    pub num_heads: usize,
    #[serde(default = "VoxCpmEncoderConfig::default_layers")]
    pub num_layers: usize,
    #[serde(default)]
    pub kv_channels: Option<usize>,
}

impl VoxCpmEncoderConfig {
    fn default_hidden() -> usize {
        1024
    }
    fn default_ffn() -> usize {
        4096
    }
    fn default_heads() -> usize {
        16
    }
    fn default_layers() -> usize {
        4
    }
}

impl Default for VoxCpmEncoderConfig {
    fn default() -> Self {
        Self {
            hidden_dim: 1024,
            ffn_dim: 4096,
            num_heads: 16,
            num_layers: 4,
            kv_channels: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VoxCpmDitConfig {
    #[serde(default = "VoxCpmEncoderConfig::default_hidden")]
    pub hidden_dim: usize,
    #[serde(default = "VoxCpmEncoderConfig::default_ffn")]
    pub ffn_dim: usize,
    #[serde(default = "VoxCpmEncoderConfig::default_heads")]
    pub num_heads: usize,
    #[serde(default = "VoxCpmEncoderConfig::default_layers")]
    pub num_layers: usize,
    #[serde(default)]
    pub kv_channels: Option<usize>,
    #[serde(default, alias = "mean_mode")]
    pub dit_mean_mode: bool,
    pub cfm_config: CfmConfig,
}

// ---------------------------------------------------------------------------
// AudioVAE v2
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AudioVaeConfig {
    #[serde(default = "AudioVaeConfig::default_encoder_dim")]
    pub encoder_dim: usize,
    #[serde(default = "AudioVaeConfig::default_encoder_rates")]
    pub encoder_rates: Vec<usize>,
    #[serde(default = "AudioVaeConfig::default_latent_dim")]
    pub latent_dim: usize,
    #[serde(default = "AudioVaeConfig::default_decoder_dim")]
    pub decoder_dim: usize,
    #[serde(default = "AudioVaeConfig::default_decoder_rates")]
    pub decoder_rates: Vec<usize>,
    #[serde(default = "default_true")]
    pub depthwise: bool,
    #[serde(default = "AudioVaeConfig::default_sample_rate")]
    pub sample_rate: usize,
    #[serde(default = "AudioVaeConfig::default_out_sample_rate")]
    pub out_sample_rate: usize,
    #[serde(default)]
    pub use_noise_block: bool,
    #[serde(default = "AudioVaeConfig::default_sr_bin_boundaries")]
    pub sr_bin_boundaries: Option<Vec<i32>>,
    #[serde(default = "AudioVaeConfig::default_cond_type")]
    pub cond_type: String,
    #[serde(default = "AudioVaeConfig::default_cond_dim")]
    pub cond_dim: usize,
    #[serde(default)]
    pub cond_out_layer: bool,
}

impl AudioVaeConfig {
    fn default_encoder_dim() -> usize {
        128
    }
    fn default_encoder_rates() -> Vec<usize> {
        vec![2, 5, 8, 8]
    }
    fn default_latent_dim() -> usize {
        64
    }
    fn default_decoder_dim() -> usize {
        2048
    }
    fn default_decoder_rates() -> Vec<usize> {
        vec![8, 6, 5, 2, 2, 2]
    }
    fn default_sample_rate() -> usize {
        16000
    }
    fn default_out_sample_rate() -> usize {
        48000
    }
    fn default_sr_bin_boundaries() -> Option<Vec<i32>> {
        Some(vec![20000, 30000, 40000])
    }
    fn default_cond_type() -> String {
        "scale_bias".to_string()
    }
    fn default_cond_dim() -> usize {
        128
    }

    /// Audio chunk size in samples covered by a single latent frame at the encoder rate.
    pub fn chunk_size(&self) -> usize {
        self.encoder_rates.iter().product()
    }
    /// Audio chunk size in samples covered by a single latent frame at the decoder rate.
    pub fn decode_chunk_size(&self) -> usize {
        self.decoder_rates.iter().product()
    }
}

impl Default for AudioVaeConfig {
    fn default() -> Self {
        Self {
            encoder_dim: 128,
            encoder_rates: vec![2, 5, 8, 8],
            latent_dim: 64,
            decoder_dim: 2048,
            decoder_rates: vec![8, 6, 5, 2, 2, 2],
            depthwise: true,
            sample_rate: 16000,
            out_sample_rate: 48000,
            use_noise_block: false,
            sr_bin_boundaries: Some(vec![20000, 30000, 40000]),
            cond_type: "scale_bias".to_string(),
            cond_dim: 128,
            cond_out_layer: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Top-level VoxCPM2 config
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VoxCpm2Config {
    pub lm_config: MiniCpm4Config,
    #[serde(default = "default_patch_size")]
    pub patch_size: usize,
    #[serde(default = "default_feat_dim")]
    pub feat_dim: usize,
    #[serde(default = "default_residual_lm_num_layers")]
    pub residual_lm_num_layers: usize,
    #[serde(default)]
    pub residual_lm_no_rope: bool,
    #[serde(default = "default_sq_latent_dim")]
    pub scalar_quantization_latent_dim: usize,
    #[serde(default = "default_sq_scale")]
    pub scalar_quantization_scale: usize,
    pub encoder_config: VoxCpmEncoderConfig,
    pub dit_config: VoxCpmDitConfig,
    #[serde(default)]
    pub audio_vae_config: Option<AudioVaeConfig>,

    #[serde(default = "default_max_length")]
    pub max_length: usize,
    #[serde(default = "default_device")]
    pub device: String,
    #[serde(default = "default_dtype")]
    pub dtype: String,
}

// ---------------------------------------------------------------------------
// LoRA (parsing only — loading is via `VoxCPM::load_lora_weights`)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LoraConfig {
    #[serde(default)]
    pub enable_lm: bool,
    #[serde(default)]
    pub enable_dit: bool,
    #[serde(default)]
    pub enable_proj: bool,
    #[serde(default = "LoraConfig::default_r")]
    pub r: usize,
    #[serde(default = "LoraConfig::default_alpha")]
    pub alpha: usize,
    #[serde(default)]
    pub dropout: f32,
    #[serde(default = "LoraConfig::default_lm_targets")]
    pub target_modules_lm: Vec<String>,
    #[serde(default = "LoraConfig::default_dit_targets")]
    pub target_modules_dit: Vec<String>,
    #[serde(default = "LoraConfig::default_proj_targets")]
    pub target_proj_modules: Vec<String>,
}

impl LoraConfig {
    fn default_r() -> usize {
        8
    }
    fn default_alpha() -> usize {
        16
    }
    fn default_lm_targets() -> Vec<String> {
        ["q_proj", "v_proj", "k_proj", "o_proj"]
            .iter()
            .map(|s| s.to_string())
            .collect()
    }
    fn default_dit_targets() -> Vec<String> {
        Self::default_lm_targets()
    }
    fn default_proj_targets() -> Vec<String> {
        [
            "enc_to_lm_proj",
            "lm_to_dit_proj",
            "res_to_dit_proj",
            "fusion_concat_proj",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect()
    }
}

impl Default for LoraConfig {
    fn default() -> Self {
        Self {
            enable_lm: false,
            enable_dit: false,
            enable_proj: false,
            r: 8,
            alpha: 16,
            dropout: 0.0,
            target_modules_lm: Self::default_lm_targets(),
            target_modules_dit: Self::default_dit_targets(),
            target_proj_modules: Self::default_proj_targets(),
        }
    }
}

// ---------------------------------------------------------------------------
// Derivation helpers
// ---------------------------------------------------------------------------

impl VoxCpm2Config {
    /// Produce a `MiniCpm4Config` for the residual acoustic LM (smaller depth,
    /// no vocab embedding, optionally no RoPE).
    pub fn residual_lm_config(&self) -> MiniCpm4Config {
        let mut cfg = self.lm_config.clone();
        cfg.num_hidden_layers = self.residual_lm_num_layers;
        cfg.vocab_size = 0;
        cfg.no_rope = self.residual_lm_no_rope;
        cfg
    }

    /// `MiniCpm4Config` sized for the local feature encoder.
    pub fn encoder_lm_config(&self) -> MiniCpm4Config {
        let mut cfg = self.lm_config.clone();
        let e = &self.encoder_config;
        cfg.hidden_size = e.hidden_dim;
        cfg.intermediate_size = e.ffn_dim;
        cfg.num_attention_heads = e.num_heads;
        cfg.num_hidden_layers = e.num_layers;
        cfg.kv_channels = e.kv_channels;
        cfg.vocab_size = 0;
        cfg
    }

    /// `MiniCpm4Config` sized for the local DiT estimator.
    pub fn dit_lm_config(&self) -> MiniCpm4Config {
        let mut cfg = self.lm_config.clone();
        let d = &self.dit_config;
        cfg.hidden_size = d.hidden_dim;
        cfg.intermediate_size = d.ffn_dim;
        cfg.num_attention_heads = d.num_heads;
        cfg.num_hidden_layers = d.num_layers;
        cfg.kv_channels = d.kv_channels;
        cfg.vocab_size = 0;
        cfg
    }
}
