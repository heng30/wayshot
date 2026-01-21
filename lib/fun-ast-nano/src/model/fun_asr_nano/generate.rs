use crate::{
    ENGLISH_PUNCTUATIONS, FunAsrError, INPUT_AUDIO_CHANNELS, INPUT_AUDIO_SAMPLE_RATE, Result,
    device::{get_device, get_dtype},
    model::fun_asr_nano::{
        config::FunASRNanoConfig, model::FunAsrNanoModel, processor::FunAsrNanoProcessor,
    },
    model::qwen3::{Qwen3Config, Qwen3GenerationConfig},
    tokenizer::TokenizerModel,
};
use audio_utils::{
    audio::{AudioConfig, load_audio_file_and_convert},
    vad::{VadConfig, detect_speech_segments},
};
use candle_core::{DType, Device, Tensor, pickle::read_all_with_key};
use candle_nn::VarBuilder;
use derivative::Derivative;
use derive_setters::Setters;
use rand::{Rng, SeedableRng};
use std::{collections::HashMap, path::Path};

const ASR_CONFIG_YAML: &str = include_str!("../../../asset/config.yaml");
const QWEN3_0_6B_LLM_CONFIG_JSON: &str = include_str!("../../../asset/qwen3_0.6b_config.json");
const QWEN3_0_6B_GENERATION_CONFIG: &str =
    include_str!("../../../asset/qwen3_0.6b_generation_config.json");

#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct FunASRModelConfig {
    #[derivative(Default(value = "String::from(\"model.pt\")"))]
    pub model_weights: String,

    #[derivative(Default(value = "String::from(\"qwen3_0.6B_tokenizer.json\")"))]
    pub tokenizer_path: String,
}

#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct TranscriptionRequest {
    pub audio_config: AudioConfig,
    pub prompt: Option<String>,
    #[derivative(Default(value = "512"))]
    pub max_tokens: u32,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
}

#[derive(Debug, Clone)]
pub struct TranscriptionResponse {
    pub text: String,
    pub num_tokens: u32,
}

#[derive(Debug, Clone)]
pub struct StreamChunk {
    pub text: String,
    pub is_finished: bool,
    pub num_tokens: u32,
    pub progress: f32, // [0-1]
    pub segment_info: Option<SegmentInfo>,
}

impl StreamChunk {
    pub fn finished(text: String, num_tokens: u32) -> Self {
        StreamChunk {
            text,
            is_finished: true,
            num_tokens,
            progress: 1.0,
            segment_info: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SegmentInfo {
    pub current_segment: usize, // (1-based)
    pub total_segments: usize,
    pub segment_start_ms: u32,
    pub segment_end_ms: u32,
}

pub struct FunAsrNanoGenerateModel {
    tokenizer: TokenizerModel,
    processor: FunAsrNanoProcessor,
    fun_asr_nano: FunAsrNanoModel,
    device: Device,
    dtype: DType,
    eos_token_id1: u32,
    eos_token_id2: u32,
    generation_config: Qwen3GenerationConfig,
}

impl FunAsrNanoGenerateModel {
    pub fn new(
        config: FunASRModelConfig,
        device: Option<&Device>,
        dtype: Option<DType>,
    ) -> Result<Self> {
        Self::validate_files(&config)?;
        let tokenizer = TokenizerModel::new(&config.tokenizer_path)?;
        let generation_config: Qwen3GenerationConfig =
            serde_json::from_slice(&QWEN3_0_6B_GENERATION_CONFIG.as_bytes())?;
        let llm_cfg: Qwen3Config = serde_json::from_slice(&QWEN3_0_6B_LLM_CONFIG_JSON.as_bytes())?;
        let cfg: FunASRNanoConfig = serde_yaml::from_slice(&ASR_CONFIG_YAML.as_bytes())?;

        let cfg_dtype = cfg.llm_conf.llm_dtype.as_str();
        let dtype = get_dtype(dtype, cfg_dtype)?;
        let device = get_device(device);
        let processor = FunAsrNanoProcessor::new(&cfg.frontend_conf, &device)?;

        let tensor_vec: Vec<(String, Tensor)> =
            match read_all_with_key(&config.model_weights, Some("state_dict")) {
                Ok(dict) => dict,
                Err(e) => {
                    log::warn!(
                        "model read_all_with_key {} get state_dict err: {}, use None try again",
                        &config.model_weights,
                        e
                    );
                    read_all_with_key(&config.model_weights, None)?
                }
            };

        let dict: HashMap<String, Tensor> = tensor_vec.into_iter().collect();
        let vb = VarBuilder::from_tensors(dict, dtype, &device);
        let fun_asr_nano = FunAsrNanoModel::new(vb, &cfg, &llm_cfg)?;

        Ok(Self {
            tokenizer,
            processor,
            fun_asr_nano,
            device,
            dtype,
            eos_token_id1: generation_config.eos_token_id[0] as u32,
            eos_token_id2: generation_config.eos_token_id[1] as u32,
            generation_config,
        })
    }

    pub fn generate(
        &mut self,
        request: TranscriptionRequest,
        vad_config: Option<VadConfig>,
        mut callback: impl FnMut(StreamChunk) -> Result<()>,
    ) -> Result<TranscriptionResponse> {
        let audio_data = request.audio_config.samples;
        let sample_rate = request.audio_config.sample_rate;

        let mut vad_config = vad_config.unwrap_or_default();
        vad_config.sample_rate = sample_rate;

        let segments = detect_speech_segments(&audio_data, &vad_config);
        if segments.is_empty() {
            callback(StreamChunk::finished(String::new(), 0))?;

            return Ok(TranscriptionResponse {
                text: String::new(),
                num_tokens: 0,
            });
        }

        let total_segments = segments.len();
        let mut all_text = String::new();
        let mut total_tokens = 0;

        for (segment_idx, segment) in segments.iter().enumerate() {
            let segment_num = segment_idx + 1;
            let segment_start_ms = (segment.start_sample * 1000 / sample_rate as usize) as u32;
            let segment_end_ms = (segment.end_sample * 1000 / sample_rate as usize) as u32;

            log::debug!(
                "Processing segment {}/{} ({} samples, {}ms -> {}ms)",
                segment_num,
                total_segments,
                segment.audio_data.len(),
                segment_start_ms,
                segment_end_ms
            );

            let segment_info = SegmentInfo {
                current_segment: segment_num,
                total_segments,
                segment_start_ms,
                segment_end_ms,
            };

            let segment_result = self.transcribe_segment(
                &segment.audio_data,
                request.prompt.as_deref(),
                request.max_tokens,
                request.temperature,
                request.top_p,
            )?;

            if !segment_result.text.is_empty() {
                let chunk = StreamChunk {
                    text: segment_result.text.clone(),
                    is_finished: false,
                    num_tokens: segment_result.num_tokens,
                    progress: (segment_idx + 1) as f32 / total_segments as f32,
                    segment_info: Some(segment_info.clone()),
                };
                callback(chunk)?;

                if segment_idx > 0
                    && !all_text.is_empty()
                    && all_text.ends_with(ENGLISH_PUNCTUATIONS)
                {
                    all_text.push(' ');
                }
                all_text.push_str(&segment_result.text);
            }

            total_tokens += segment_result.num_tokens;
        }

        self.fun_asr_nano.clear_kv_cache();
        callback(StreamChunk::finished(all_text.clone(), total_tokens))?;

        Ok(TranscriptionResponse {
            text: all_text,
            num_tokens: total_tokens,
        })
    }

    fn transcribe_segment(
        &mut self,
        audio_data: &[f32],
        prompt: Option<&str>,
        max_tokens: u32,
        temperature: Option<f32>,
        top_p: Option<f32>,
    ) -> Result<TranscriptionResponse> {
        let temperature = temperature.unwrap_or(self.generation_config.temperature);
        let top_p = top_p.unwrap_or(self.generation_config.top_p);
        let top_k = self.generation_config.top_k;
        let seed = 34562u64;
        let max_tokens = max_tokens.min(512); // Limit segment tokens

        let mut logit_processor = SimpleLogitProcessor::new(temperature, top_p, top_k, seed);

        let (speech, fbank_mask, mut input_ids) =
            self.processor
                .process_audio(audio_data, prompt, &self.tokenizer)?;

        let mut speech = Some(speech.to_dtype(self.dtype)?);
        let mut fbank_mask = Some(&fbank_mask);
        let mut seq_len = input_ids.dim(1)?;
        let mut seqlen_offset = 0;
        let mut generate = Vec::new();
        let mut segment_text = String::new();

        for _ in 0..max_tokens {
            let logits = self.fun_asr_nano.forward(
                &input_ids,
                speech.as_ref(),
                fbank_mask,
                seqlen_offset,
            )?;
            let logits = logits.squeeze(0)?.squeeze(0)?.to_dtype(DType::F32)?;
            let next_token = logit_processor.sample(&logits)?;
            generate.push(next_token);

            let recent_tokens: Vec<u32> = generate.iter().rev().take(100).cloned().collect();
            let recent_tokens: Vec<u32> = recent_tokens.into_iter().rev().collect();
            let decoded_text = self.tokenizer.token_decode(recent_tokens)?;

            if next_token == self.eos_token_id1 || next_token == self.eos_token_id2 {
                segment_text = decoded_text.trim().to_string();
                break;
            }

            seqlen_offset += seq_len;
            seq_len = 1;
            input_ids = Tensor::from_vec(vec![next_token], (1, 1), &self.device)?;
            speech = None;
            fbank_mask = None;
        }

        self.fun_asr_nano.clear_kv_cache();

        Ok(TranscriptionResponse {
            text: segment_text,
            num_tokens: generate.len() as u32,
        })
    }

    fn validate_files(config: &FunASRModelConfig) -> Result<()> {
        if !Path::new(&config.model_weights).exists() {
            return Err(FunAsrError::NotFound(format!(
                "Model weights file not found: {}",
                config.model_weights
            )));
        }

        if !Path::new(&config.tokenizer_path).exists() {
            return Err(FunAsrError::NotFound(format!(
                "Tokenizer path not found: {}",
                config.tokenizer_path
            )));
        }
        Ok(())
    }
}

struct SimpleLogitProcessor {
    temperature: f32,
    rng: rand::rngs::StdRng,
}

impl SimpleLogitProcessor {
    fn new(temperature: f32, _top_p: f32, _top_k: usize, seed: u64) -> Self {
        Self {
            temperature,
            rng: rand::rngs::StdRng::seed_from_u64(seed),
        }
    }

    fn sample(&mut self, logits: &Tensor) -> Result<u32> {
        let logits = logits.to_vec1::<f32>()?;
        let logits: Vec<f32> = logits.iter().map(|x| x / self.temperature).collect();

        // Compute softmax
        let max_logit = logits.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));
        let exp_logits: Vec<f32> = logits.iter().map(|x| (x - max_logit).exp()).collect();
        let sum: f32 = exp_logits.iter().sum();
        let probs: Vec<f32> = exp_logits.iter().map(|x| x / sum).collect();

        // Sample using custom weighted sampling
        let rand_val: f32 = self.rng.random();
        let mut cumulative = 0.0f32;
        for (idx, &prob) in probs.iter().enumerate() {
            cumulative += prob;
            if rand_val < cumulative {
                return Ok(idx as u32);
            }
        }

        Ok((probs.len() - 1) as u32)
    }
}

pub fn load_audio_file(path: impl AsRef<Path>) -> Result<AudioConfig> {
    let config =
        load_audio_file_and_convert(path, INPUT_AUDIO_CHANNELS as u16, INPUT_AUDIO_SAMPLE_RATE)?;
    Ok(config)
}
