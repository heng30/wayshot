use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct FunASRNanoConfig {
    pub audio_encoder_conf: AudioEncoderConf,
    pub llm_conf: LlmConf,
    pub audio_adaptor_conf: AudioAdaptorConf,
    pub detach_ctc_decoder: bool,
    pub ctc_decoder_conf: CtcDecoderConf,
    pub ctc_weight: f64,
    pub ctc_conf: CtcConf,
    pub frontend_conf: FrontendConf,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct AudioEncoderConf {
    pub output_size: usize,
    pub attention_heads: usize,
    pub linear_units: usize,
    pub num_blocks: usize,
    pub tp_blocks: usize,
    pub dropout_rate: f64,
    pub positional_dropout_rate: f64,
    pub attention_dropout_rate: f64,
    pub input_layer: String,
    pub pos_enc_class: String,
    pub normalize_before: bool,
    pub kernel_size: usize,
    pub sanm_shfit: usize,
    pub selfattention_layer_type: String,
    pub freeze: bool,
    pub freeze_layer_num: i32,
    pub feat_permute: bool,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct LlmConf {
    pub hub: String,
    pub freeze: bool,
    pub llm_dtype: String,
    pub init_param_path: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct AudioAdaptorConf {
    pub downsample_rate: usize,
    pub use_low_frame_rate: bool,
    pub ffn_dim: usize,
    pub llm_dim: usize,
    pub encoder_dim: usize,
    pub n_layer: usize,
    pub freeze: bool,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct CtcDecoderConf {
    pub downsample_rate: u32,
    pub ffn_dim: u32,
    pub llm_dim: u32,
    pub encoder_dim: u32,
    pub n_layer: u32,
    pub freeze: bool,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct CtcConf {
    pub dropout_rate: f64,
    pub ctc_type: String,
    pub reduce: bool,
    pub ignore_nan_grad: bool,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct FrontendConf {
    pub fs: usize,
    pub window: String,
    pub n_mels: usize,
    pub frame_length: f32,
    pub frame_shift: f32,
    pub lfr_m: usize,
    pub lfr_n: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cmvn_file: Option<serde_yaml::Value>,
}
