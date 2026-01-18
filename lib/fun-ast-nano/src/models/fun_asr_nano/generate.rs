use std::collections::HashMap;
use std::path::Path;

use crate::error::{FunAsrError, Result};
use candle_core::{DType, Device, Tensor, pickle::read_all_with_key};
use candle_nn::VarBuilder;
use derivative::Derivative;
use derive_setters::Setters;
use log::warn;

use crate::{
    models::fun_asr_nano::{
        config::FunASRNanoConfig, model::FunAsrNanoModel, processor::FunAsrNanoProcessor,
    },
    models::qwen3::config::{Qwen3Config, Qwen3GenerationConfig},
    tokenizer::TokenizerModel,
    utils::{get_device, get_dtype},
};

// Audio processing utilities from audio-utils crate
use audio_utils::audio::normalize_audio;
use audio_utils::vad::{VadConfig, detect_speech_segments};

/// Configuration for FunASR model files
/// All file paths must be explicitly specified
#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct FunASRModelConfig {
    /// Path to model weights file (.pt)
    #[derivative(Default(value = "String::from(\"model.pt\")"))]
    pub model_weights: String,
    /// Path to ASR config file (config.yaml)
    #[derivative(Default(value = "String::from(\"config.yaml\")"))]
    pub asr_config: String,
    /// Path to LLM config file (Qwen3-0.6B/config.json)
    #[derivative(Default(value = "String::from(\"Qwen3-0.6B/config.json\")"))]
    pub llm_config: String,
    /// Path to generation config file (Qwen3-0.6B/generation_config.json)
    #[derivative(Default(value = "String::from(\"Qwen3-0.6B/generation_config.json\")"))]
    pub generation_config: String,
    /// Path to tokenizer directory or file (Qwen3-0.6B/)
    #[derivative(Default(value = "String::from(\"Qwen3-0.6B\")"))]
    pub tokenizer_path: String,
}

/// Simple request structure for audio transcription
#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct TranscriptionRequest {
    #[derivative(Default(value = "String::from(\"\")"))]
    pub audio_path: String,
    pub prompt: Option<String>,
    #[derivative(Default(value = "512"))]
    pub max_tokens: u32,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
}

/// Simple response structure
#[derive(Debug, Clone)]
pub struct TranscriptionResponse {
    pub text: String,
    pub num_tokens: u32,
    /// Timestamp information for each segment
    pub timestamps: Vec<TimestampSegment>,
}

/// Represents a timestamp segment with precise timing information
#[derive(Debug, Clone, Derivative)]
#[derivative(Default)]
pub struct TimestampSegment {
    /// The text content of this segment
    pub text: String,
    /// Start time in milliseconds
    pub start_ms: u32,
    /// End time in milliseconds
    pub end_ms: u32,
    /// Token indices for this segment
    pub token_range: (usize, usize),
}

impl TimestampSegment {
    pub fn new(text: String, start_ms: u32, end_ms: u32, token_range: (usize, usize)) -> Self {
        Self {
            text,
            start_ms,
            end_ms,
            token_range,
        }
    }
}

/// Represents a streaming chunk of generated text
#[derive(Debug, Clone)]
pub struct StreamChunk {
    pub text: String,
    pub is_finished: bool,
    pub num_tokens: u32,
    /// Progress percentage (0-100)
    pub progress: f32,
    /// Current sentence being generated (if available)
    pub current_sentence: Option<String>,
    /// Current segment info (for segmented transcription)
    pub segment_info: Option<SegmentInfo>,
}

/// Information about current audio segment
#[derive(Debug, Clone)]
pub struct SegmentInfo {
    /// Current segment number (1-based)
    pub current_segment: usize,
    /// Total number of segments
    pub total_segments: usize,
    /// Start time of current segment in milliseconds
    pub segment_start_ms: u32,
    /// End time of current segment in milliseconds
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
    /// Initialize model with explicit file paths
    pub fn init(
        config: FunASRModelConfig,
        device: Option<&Device>,
        dtype: Option<DType>,
    ) -> Result<Self> {
        // Validate all files exist
        Self::validate_files(&config)?;

        // Load tokenizer
        let tokenizer = TokenizerModel::init(&config.tokenizer_path)?;

        // Load generation config
        let generation_config: Qwen3GenerationConfig =
            serde_json::from_slice(&std::fs::read(&config.generation_config)?)?;

        // Load LLM config
        let llm_cfg: Qwen3Config = serde_json::from_slice(&std::fs::read(&config.llm_config)?)?;

        // Get device
        let device = get_device(device);

        // Load ASR config
        let cfg: FunASRNanoConfig = serde_yaml::from_slice(&std::fs::read(&config.asr_config)?)?;
        let cfg_dtype = cfg.llm_conf.llm_dtype.as_str();
        let dtype = get_dtype(dtype, cfg_dtype)?;

        // Create processor
        let processor = FunAsrNanoProcessor::new(&cfg.frontend_conf, &device)?;

        // Load model weights
        let tensor_vec: Vec<(String, Tensor)> =
            match read_all_with_key(&config.model_weights, Some("state_dict")) {
                Ok(dict) => dict,
                Err(e) => {
                    warn!(
                        "model read_all_with_key {} get state_dict err: {}, use None try again",
                        &config.model_weights, e
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

    /// Validate that all required files exist
    fn validate_files(config: &FunASRModelConfig) -> Result<()> {
        if !Path::new(&config.model_weights).exists() {
            return Err(FunAsrError::NotFound(format!(
                "Model weights file not found: {}",
                config.model_weights
            )));
        }
        if !Path::new(&config.asr_config).exists() {
            return Err(FunAsrError::NotFound(format!(
                "ASR config file not found: {}",
                config.asr_config
            )));
        }
        if !Path::new(&config.llm_config).exists() {
            return Err(FunAsrError::NotFound(format!(
                "LLM config file not found: {}",
                config.llm_config
            )));
        }
        if !Path::new(&config.generation_config).exists() {
            return Err(FunAsrError::NotFound(format!(
                "Generation config file not found: {}",
                config.generation_config
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

    pub fn generate(&mut self, request: TranscriptionRequest) -> Result<TranscriptionResponse> {
        let temperature = request
            .temperature
            .unwrap_or(self.generation_config.temperature);
        let top_p = request.top_p.unwrap_or(self.generation_config.top_p);
        let top_k = self.generation_config.top_k;
        let seed = 34562u64;
        let sample_len = request.max_tokens;

        // Create a simple logit processor
        let mut logit_processor = SimpleLogitProcessor::new(temperature, top_p, top_k, seed);

        // Process the audio file
        let (audio_data, sample_rate) = self.load_audio_with_sample_rate(&request.audio_path)?;

        // Calculate audio duration in milliseconds
        let audio_duration_ms = (audio_data.len() as u32 * 1000 / sample_rate) as u32;

        let (speech, fbank_mask, mut input_ids) = self.processor.process_audio(
            &audio_data,
            request.prompt.as_deref(),
            &self.tokenizer,
        )?;

        let mut speech = Some(speech.to_dtype(self.dtype)?);
        let mut fbank_mask = Some(&fbank_mask);
        let mut seq_len = input_ids.dim(1)?;
        let mut seqlen_offset = 0;
        let mut generate = Vec::new();

        for _ in 0..sample_len {
            let logits = self.fun_asr_nano.forward(
                &input_ids,
                speech.as_ref(),
                fbank_mask,
                seqlen_offset,
            )?;
            let logits = logits.squeeze(0)?.squeeze(0)?.to_dtype(DType::F32)?;
            let next_token = logit_processor.sample(&logits)?;
            generate.push(next_token);
            if next_token == self.eos_token_id1 || next_token == self.eos_token_id2 {
                break;
            }
            seqlen_offset += seq_len;
            seq_len = 1;
            input_ids = Tensor::from_vec(vec![next_token], (1, 1), &self.device)?;
            speech = None;
            fbank_mask = None;
        }
        let num_token = generate.len() as u32;
        let res = self.tokenizer.token_decode(generate.clone())?;

        // Generate timestamps
        let timestamps = self.generate_timestamps(&generate, audio_duration_ms)?;

        self.fun_asr_nano.clear_kv_cache();
        Ok(TranscriptionResponse {
            text: res.clone(),
            num_tokens: num_token,
            timestamps,
        })
    }

    /// Generate timestamp segments from generated tokens
    fn generate_timestamps(
        &self,
        tokens: &[u32],
        audio_duration_ms: u32,
    ) -> Result<Vec<TimestampSegment>> {
        if tokens.is_empty() {
            return Ok(Vec::new());
        }

        // Decode tokens to text
        let text = self.tokenizer.token_decode(tokens.to_vec())?;

        // Split text into sentences using common Chinese and English punctuation
        let sentences: Vec<&str> = text
            .split(&['，', '。', '！', '？', ',', '.', '!', '?'][..])
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();

        if sentences.is_empty() {
            // Fallback: single segment
            let segment = TimestampSegment::new(text, 0, audio_duration_ms, (0, tokens.len()));
            return Ok(vec![segment]);
        }

        // Calculate approximate time per sentence
        // Assume even distribution of tokens across sentences
        let total_chars: usize = sentences.iter().map(|s| s.chars().count()).sum();
        let mut current_time = 0u32;
        let mut segments = Vec::new();
        let mut token_offset = 0;

        for &sentence in sentences.iter() {
            let sentence_chars = sentence.chars().count();
            // Estimate time proportion based on character count
            let time_proportion = sentence_chars as f32 / total_chars as f32;
            let duration_ms = (audio_duration_ms as f32 * time_proportion) as u32;

            // Estimate token proportion
            let token_proportion = sentence_chars as f32 / total_chars as f32;
            let token_count = (tokens.len() as f32 * token_proportion) as usize;
            let token_end = token_offset + token_count.max(1);

            let segment = TimestampSegment::new(
                sentence.to_string(),
                current_time,
                current_time + duration_ms,
                (token_offset, token_end.min(tokens.len())),
            );

            segments.push(segment);
            current_time += duration_ms;
            token_offset = token_end;
        }

        // Adjust the last segment to end at audio duration
        if let Some(last) = segments.last_mut() {
            last.end_ms = audio_duration_ms;
            last.token_range.1 = tokens.len();
        }

        Ok(segments)
    }

    /// Generate transcription with streaming output using callback
    pub fn generate_stream_callback<F>(
        &mut self,
        request: TranscriptionRequest,
        mut callback: F,
    ) -> Result<TranscriptionResponse>
    where
        F: FnMut(StreamChunk) -> Result<()>,
    {
        let temperature = request
            .temperature
            .unwrap_or(self.generation_config.temperature);
        let top_p = request.top_p.unwrap_or(self.generation_config.top_p);
        let top_k = self.generation_config.top_k;
        let seed = 34562u64;
        let sample_len = request.max_tokens;

        // Create a simple logit processor
        let mut logit_processor = SimpleLogitProcessor::new(temperature, top_p, top_k, seed);

        // Process the audio file
        let (audio_data, sample_rate) = self.load_audio_with_sample_rate(&request.audio_path)?;

        // Calculate audio duration in milliseconds
        let audio_duration_ms = (audio_data.len() as u32 * 1000 / sample_rate) as u32;

        let (speech, fbank_mask, mut input_ids) = self.processor.process_audio(
            &audio_data,
            request.prompt.as_deref(),
            &self.tokenizer,
        )?;

        let mut speech = Some(speech.to_dtype(self.dtype)?);
        let mut fbank_mask = Some(&fbank_mask);
        let mut seq_len = input_ids.dim(1)?;
        let mut seqlen_offset = 0;
        let mut generate = Vec::new();
        let mut final_text = String::new();

        for _i in 0..sample_len {
            let logits = self.fun_asr_nano.forward(
                &input_ids,
                speech.as_ref(),
                fbank_mask,
                seqlen_offset,
            )?;
            let logits = logits.squeeze(0)?.squeeze(0)?.to_dtype(DType::F32)?;
            let next_token = logit_processor.sample(&logits)?;
            generate.push(next_token);

            // Try to decode - only decode recent tokens to avoid capacity issues
            let recent_tokens: Vec<u32> = generate.iter().rev().take(100).cloned().collect();
            let recent_tokens: Vec<u32> = recent_tokens.into_iter().rev().collect();

            let decoded_text = self.tokenizer.token_decode(recent_tokens)?;

            // Check for decoding errors (replacement character)
            if !decoded_text.contains('�') {
                final_text = decoded_text.clone();

                // Extract current sentence (text after last punctuation)
                let current_sentence = decoded_text
                    .split(&['，', '。', '！', '？', ',', '.', '!', '?'][..])
                    .last()
                    .unwrap_or("")
                    .trim()
                    .to_string();

                let progress = (generate.len() as f32 / sample_len as f32 * 100.0).min(100.0);

                // Send streaming chunk
                let chunk = StreamChunk {
                    text: decoded_text,
                    is_finished: false,
                    num_tokens: generate.len() as u32,
                    progress,
                    current_sentence: if current_sentence.is_empty() {
                        None
                    } else {
                        Some(current_sentence)
                    },
                    segment_info: None,
                };
                callback(chunk)?;
            }

            if next_token == self.eos_token_id1 || next_token == self.eos_token_id2 {
                break;
            }
            seqlen_offset += seq_len;
            seq_len = 1;
            input_ids = Tensor::from_vec(vec![next_token], (1, 1), &self.device)?;
            speech = None;
            fbank_mask = None;
        }

        let num_token = generate.len() as u32;

        // Generate timestamps with sentence-level granularity
        let timestamps = self.generate_timestamps(&generate, audio_duration_ms)?;

        // Send final chunk
        let final_chunk = StreamChunk {
            text: final_text.clone(),
            is_finished: true,
            num_tokens: num_token,
            progress: 100.0,
            current_sentence: None,
            segment_info: None,
        };
        callback(final_chunk)?;

        self.fun_asr_nano.clear_kv_cache();
        Ok(TranscriptionResponse {
            text: final_text,
            num_tokens: num_token,
            timestamps,
        })
    }

    fn load_audio_with_sample_rate(&self, path: &str) -> Result<(Vec<f32>, u32)> {
        // Only support WAV files for now
        let reader = hound::WavReader::open(path)
            .map_err(|e| FunAsrError::Audio(format!("Failed to open WAV file: {}", e)))?;
        let original_sample_rate = reader.spec().sample_rate;
        let channels = reader.spec().channels as u32;

        let mut audio_data = Vec::new();
        reader
            .into_samples::<i16>()
            .filter_map(|s| s.ok())
            .for_each(|s| audio_data.push(s as f32 / 32768.0));

        // Normalize audio to 16000 Hz mono (expected by ASR models)
        let target_sample_rate = 16000u32;
        let target_channels = 1u32;

        let audio_data =
            if original_sample_rate != target_sample_rate || channels != target_channels {
                log::info!(
                    "Audio format: {} Hz, {} channels -> Target: {} Hz, {} channels",
                    original_sample_rate,
                    channels,
                    target_sample_rate,
                    target_channels
                );

                normalize_audio(
                    &audio_data,
                    original_sample_rate,
                    channels,
                    target_sample_rate,
                    target_channels,
                )?
            } else {
                audio_data
            };

        Ok((audio_data, target_sample_rate))
    }

    /// Generate transcription using VAD-based audio segmentation
    /// This method splits audio into speech segments and transcribes each segment separately
    /// which can improve efficiency and accuracy for longer audio files
    pub fn generate_by_segments(
        &mut self,
        request: TranscriptionRequest,
        vad_config: Option<VadConfig>,
    ) -> Result<TranscriptionResponse> {
        // Load audio
        let (audio_data, sample_rate) = self.load_audio_with_sample_rate(&request.audio_path)?;

        // Use default or custom VAD config
        let mut vad_config = vad_config.unwrap_or_default();
        vad_config.sample_rate = sample_rate;

        // Detect speech segments
        let segments = detect_speech_segments(&audio_data, &vad_config);

        if segments.is_empty() {
            // No speech detected, return empty response
            return Ok(TranscriptionResponse {
                text: String::new(),
                num_tokens: 0,
                timestamps: Vec::new(),
            });
        }

        let mut all_text = String::new();
        let mut all_timestamps = Vec::new();
        let mut total_tokens = 0;
        let mut current_offset_ms = 0u32;

        // Process each segment
        for (segment_idx, segment) in segments.iter().enumerate() {
            log::info!(
                "Processing segment {}/{} ({} samples)",
                segment_idx + 1,
                segments.len(),
                segment.audio_data.len()
            );

            // Calculate segment duration
            let segment_duration_ms =
                (segment.end_sample - segment.start_sample) as u32 * 1000 / sample_rate;

            // Process this segment
            let segment_result = self.transcribe_segment(
                &segment.audio_data,
                sample_rate,
                request.prompt.as_deref(),
                request.max_tokens,
                request.temperature,
                request.top_p,
            )?;

            // Accumulate results
            if !segment_result.text.is_empty() {
                // Add separator between segments (except first)
                if segment_idx > 0 && !all_text.is_empty() {
                    all_text.push(' ');
                }
                all_text.push_str(&segment_result.text);
            }

            total_tokens += segment_result.num_tokens;

            // Adjust timestamps to global timeline
            for mut ts in segment_result.timestamps {
                ts.start_ms += current_offset_ms;
                ts.end_ms += current_offset_ms;
                all_timestamps.push(ts);
            }

            current_offset_ms += segment_duration_ms;
        }

        // Clear KV cache after processing all segments
        self.fun_asr_nano.clear_kv_cache();

        Ok(TranscriptionResponse {
            text: all_text,
            num_tokens: total_tokens,
            timestamps: all_timestamps,
        })
    }

    /// Generate transcription using VAD-based audio segmentation with streaming output
    /// Provides real-time progress for each segment being transcribed
    pub fn generate_by_segments_stream_callback<F>(
        &mut self,
        request: TranscriptionRequest,
        vad_config: Option<VadConfig>,
        mut callback: F,
    ) -> Result<TranscriptionResponse>
    where
        F: FnMut(StreamChunk) -> Result<()>,
    {
        // Load audio
        let (audio_data, sample_rate) = self.load_audio_with_sample_rate(&request.audio_path)?;

        // Use default or custom VAD config
        let mut vad_config = vad_config.unwrap_or_default();
        vad_config.sample_rate = sample_rate;

        // Detect speech segments
        let segments = detect_speech_segments(&audio_data, &vad_config);

        if segments.is_empty() {
            // No speech detected, send empty finished chunk
            let chunk = StreamChunk {
                text: String::new(),
                is_finished: true,
                num_tokens: 0,
                progress: 100.0,
                current_sentence: None,
                segment_info: None,
            };
            callback(chunk)?;

            return Ok(TranscriptionResponse {
                text: String::new(),
                num_tokens: 0,
                timestamps: Vec::new(),
            });
        }

        let total_segments = segments.len();
        let mut all_text = String::new();
        let mut all_timestamps = Vec::new();
        let mut total_tokens = 0;
        let mut current_offset_ms = 0u32;

        // Process each segment with streaming output
        for (segment_idx, segment) in segments.iter().enumerate() {
            let segment_num = segment_idx + 1;

            // Calculate segment duration and time positions
            let segment_start_ms = (segment.start_sample * 1000 / sample_rate as usize) as u32;
            let segment_end_ms = (segment.end_sample * 1000 / sample_rate as usize) as u32;

            log::info!(
                "Processing segment {}/{} ({} samples, {}ms -> {}ms)",
                segment_num,
                total_segments,
                segment.audio_data.len(),
                segment_start_ms,
                segment_end_ms
            );

            // Create segment info for this chunk
            let segment_info = SegmentInfo {
                current_segment: segment_num,
                total_segments,
                segment_start_ms,
                segment_end_ms,
            };

            // Transcribe this segment with streaming
            let segment_result = self.transcribe_segment_stream(
                &segment.audio_data,
                sample_rate,
                request.prompt.as_deref(),
                request.max_tokens,
                request.temperature,
                request.top_p,
                &segment_info,
                &mut callback,
                segment_idx,
                &all_text,
            )?;

            // Accumulate results
            if !segment_result.text.is_empty() {
                // Add separator between segments (except first)
                if segment_idx > 0 && !all_text.is_empty() {
                    all_text.push(' ');
                }
                all_text.push_str(&segment_result.text);
            }

            total_tokens += segment_result.num_tokens;

            // Adjust timestamps to global timeline
            for mut ts in segment_result.timestamps {
                ts.start_ms += current_offset_ms;
                ts.end_ms += current_offset_ms;
                all_timestamps.push(ts);
            }

            current_offset_ms += segment_end_ms - segment_start_ms;
        }

        // Clear KV cache after processing all segments
        self.fun_asr_nano.clear_kv_cache();

        // Send final completion chunk
        let final_chunk = StreamChunk {
            text: all_text.clone(),
            is_finished: true,
            num_tokens: total_tokens,
            progress: 100.0,
            current_sentence: None,
            segment_info: None,
        };
        callback(final_chunk)?;

        Ok(TranscriptionResponse {
            text: all_text,
            num_tokens: total_tokens,
            timestamps: all_timestamps,
        })
    }

    /// Transcribe a single audio segment with streaming output
    fn transcribe_segment_stream<F>(
        &mut self,
        audio_data: &[f32],
        sample_rate: u32,
        prompt: Option<&str>,
        max_tokens: u32,
        temperature: Option<f32>,
        top_p: Option<f32>,
        segment_info: &SegmentInfo,
        callback: &mut F,
        segment_idx: usize,
        previous_text: &str,
    ) -> Result<TranscriptionResponse>
    where
        F: FnMut(StreamChunk) -> Result<()>,
    {
        let temperature = temperature.unwrap_or(self.generation_config.temperature);
        let top_p = top_p.unwrap_or(self.generation_config.top_p);
        let top_k = self.generation_config.top_k;
        let seed = 34562u64;
        let sample_len = max_tokens.min(512); // Limit segment tokens

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

        for _i in 0..sample_len {
            let logits = self.fun_asr_nano.forward(
                &input_ids,
                speech.as_ref(),
                fbank_mask,
                seqlen_offset,
            )?;
            let logits = logits.squeeze(0)?.squeeze(0)?.to_dtype(DType::F32)?;
            let next_token = logit_processor.sample(&logits)?;
            generate.push(next_token);

            // Try to decode - only decode recent tokens to avoid capacity issues
            let recent_tokens: Vec<u32> = generate.iter().rev().take(100).cloned().collect();
            let recent_tokens: Vec<u32> = recent_tokens.into_iter().rev().collect();

            let decoded_text = self.tokenizer.token_decode(recent_tokens)?;

            // Check for decoding errors (replacement character)
            if !decoded_text.contains('�') {
                segment_text = decoded_text.clone();

                // Extract current sentence (text after last punctuation)
                let current_sentence = decoded_text
                    .split(&['，', '。', '！', '？', ',', '.', '!', '?'][..])
                    .last()
                    .unwrap_or("")
                    .trim()
                    .to_string();

                // Calculate progress: combine segment progress and overall progress
                let segment_progress =
                    (generate.len() as f32 / sample_len as f32 * 100.0).min(100.0);
                let overall_progress = ((segment_idx as f32 + segment_progress / 100.0)
                    / segment_info.total_segments as f32
                    * 100.0)
                    .min(100.0);

                // Prefix with previous text for continuity
                let full_text = if segment_idx == 0 {
                    decoded_text.clone()
                } else {
                    format!("{} {}", previous_text, decoded_text)
                };

                // Send streaming chunk with segment info
                let chunk = StreamChunk {
                    text: full_text,
                    is_finished: false,
                    num_tokens: generate.len() as u32,
                    progress: overall_progress,
                    current_sentence: if current_sentence.is_empty() {
                        None
                    } else {
                        Some(current_sentence)
                    },
                    segment_info: Some(segment_info.clone()),
                };
                callback(chunk)?;
            }

            if next_token == self.eos_token_id1 || next_token == self.eos_token_id2 {
                break;
            }

            seqlen_offset += seq_len;
            seq_len = 1;
            input_ids = Tensor::from_vec(vec![next_token], (1, 1), &self.device)?;
            speech = None;
            fbank_mask = None;
        }

        let num_token = generate.len() as u32;

        // Calculate segment duration in milliseconds
        let segment_duration_ms = (audio_data.len() as u32 * 1000 / sample_rate) as u32;

        // Generate timestamps
        let timestamps = self.generate_timestamps(&generate, segment_duration_ms)?;

        self.fun_asr_nano.clear_kv_cache();

        Ok(TranscriptionResponse {
            text: segment_text,
            num_tokens: num_token,
            timestamps,
        })
    }

    /// Transcribe a single audio segment
    fn transcribe_segment(
        &mut self,
        audio_data: &[f32],
        sample_rate: u32,
        prompt: Option<&str>,
        max_tokens: u32,
        temperature: Option<f32>,
        top_p: Option<f32>,
    ) -> Result<TranscriptionResponse> {
        let temperature = temperature.unwrap_or(self.generation_config.temperature);
        let top_p = top_p.unwrap_or(self.generation_config.top_p);
        let top_k = self.generation_config.top_k;
        let seed = 34562u64;
        let sample_len = max_tokens.min(512); // Limit segment tokens

        let mut logit_processor = SimpleLogitProcessor::new(temperature, top_p, top_k, seed);

        let (speech, fbank_mask, mut input_ids) =
            self.processor
                .process_audio(audio_data, prompt, &self.tokenizer)?;

        let mut speech = Some(speech.to_dtype(self.dtype)?);
        let mut fbank_mask = Some(&fbank_mask);
        let mut seq_len = input_ids.dim(1)?;
        let mut seqlen_offset = 0;
        let mut generate = Vec::new();

        for _i in 0..sample_len {
            let logits = self.fun_asr_nano.forward(
                &input_ids,
                speech.as_ref(),
                fbank_mask,
                seqlen_offset,
            )?;
            let logits = logits.squeeze(0)?.squeeze(0)?.to_dtype(DType::F32)?;
            let next_token = logit_processor.sample(&logits)?;
            generate.push(next_token);

            if next_token == self.eos_token_id1 || next_token == self.eos_token_id2 {
                break;
            }

            seqlen_offset += seq_len;
            seq_len = 1;
            input_ids = Tensor::from_vec(vec![next_token], (1, 1), &self.device)?;
            speech = None;
            fbank_mask = None;
        }

        let num_token = generate.len() as u32;
        let text = self.tokenizer.token_decode(generate.clone())?;

        // Calculate segment duration in milliseconds
        let segment_duration_ms = (audio_data.len() as u32 * 1000 / sample_rate) as u32;

        // Generate timestamps
        let timestamps = self.generate_timestamps(&generate, segment_duration_ms)?;

        self.fun_asr_nano.clear_kv_cache();

        Ok(TranscriptionResponse {
            text,
            num_tokens: num_token,
            timestamps,
        })
    }
}

struct SimpleLogitProcessor {
    temperature: f32,
    rng: rand::rngs::StdRng,
}

impl SimpleLogitProcessor {
    fn new(temperature: f32, _top_p: f32, _top_k: usize, seed: u64) -> Self {
        use rand::SeedableRng;
        Self {
            temperature,
            rng: rand::rngs::StdRng::seed_from_u64(seed),
        }
    }

    fn sample(&mut self, logits: &Tensor) -> Result<u32> {
        use rand::Rng;

        let logits = logits.to_vec1::<f32>()?;

        // Apply temperature
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

        // Fallback to last index if rounding errors
        Ok((probs.len() - 1) as u32)
    }
}
