//! High-level inference interface for LFM2.5-VL
//!
//! # Model locations
//!
//! | Model | Source |
//! |-------|--------|
//! | LFM2.5-VL-450M | <https://huggingface.co/Liquid1/LFM2.5-VL-450M> |
//! | LFM2.5-VL-1.6B | <https://huggingface.co/Liquid1/LFM2.5-VL-1.6B> |
//!
//! Download all model files (config.json, tokenizer.json, model.safetensors,
//! processor_config.json, generation_config.json, tokenizer_config.json,
//! chat_template.jinja) into a local directory.
//!
//! # Cancellation & progress
//!
//! The [`InferOptions`] struct supports:
//! - **cancellation** — set a `cancel` callback (`Fn() -> bool`); if it returns
//!   `true` the generation loop stops and the partial result is returned.
//! - **progress callback** — set an `on_token` callback (`Fn(u32, usize)`);
//!   called after every generated token with `(token_id, step_index)`.

use std::sync::Arc;

use crate::chat_template::ChatTemplate;
use crate::lfm2::config::Lfm2GenerateConfig;
use crate::lfm2vl::config::Lfm2VLConfig;
use crate::lfm2vl::model::LFM2VLModel;
use crate::lfm2vl::processor::Processor;
use crate::tokenizer::TokenizerModel;
use crate::error::Result;
use candle_core::{DType, Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::generation::LogitsProcessor;
use image::DynamicImage;

/// Convenience helper: convert `image::RgbaImage` → `DynamicImage`.
pub fn rgba_to_dynamic(rgba: image::RgbaImage) -> DynamicImage {
    DynamicImage::ImageRgba8(rgba)
}

/// Inference options including cancellation and progress callbacks.
pub struct InferOptions {
    /// Maximum tokens to generate.
    pub max_tokens: u32,
    /// Sampling temperature (`None` → greedy / argmax).
    pub temperature: Option<f32>,
    /// Top-p nucleus sampling threshold.
    pub top_p: Option<f32>,
    /// Top-k sampling.
    pub top_k: Option<usize>,
    /// Repeat penalty applied to recently generated tokens.
    /// Values > 1.0 discourage repetition (e.g. 1.1 = mild, 1.5 = strong).
    /// Default is 1.0 (no penalty).
    pub repeat_penalty: f32,
    /// How many of the most recent tokens to consider for repeat penalty.
    pub repeat_last_n: usize,
    /// Cancellation callback. Called before each token generation step.
    /// Return `true` to stop generation early (partial text is returned).
    pub cancel: Option<Arc<dyn Fn() -> bool + Send + Sync>>,
    /// Progress callback. Called after each token is generated with
    /// `(token_id, step_index)`. `step_index` starts at 0.
    pub on_token: Option<Arc<dyn Fn(u32, usize) + Send + Sync>>,
    /// Force F32 dtype even on GPU (for maximum accuracy).
    /// On CPU, BF16/F16 models automatically fall back to F32
    /// unless this is set to `true`.
    pub force_f32: bool,
    /// Override maximum number of image tiles (default: from model config).
    /// Smaller values = faster prefill but less image detail.
    /// E.g. `Some(2)` instead of default `10` can reduce prefill time 5x+.
    pub max_tiles: Option<usize>,
}

impl Default for InferOptions {
    fn default() -> Self {
        Self {
            max_tokens: 1024,
            temperature: None,
            top_p: None,
            top_k: None,
            repeat_penalty: 1.0,
            repeat_last_n: 64,
            cancel: None,
            on_token: None,
            force_f32: false,
            max_tiles: None,
        }
    }
}

/// The result of an inference call.
#[derive(Debug)]
pub struct InferResult {
    /// The generated text (may be partial if cancelled).
    pub text: String,
    /// Number of tokens generated.
    pub token_count: usize,
    /// Whether generation was cancelled via the callback.
    pub cancelled: bool,
}

/// A loaded LFM2.5-VL model ready for inference.
pub struct LFM2VL {
    chat_template: ChatTemplate,
    tokenizer: TokenizerModel,
    device: Device,
    model: LFM2VLModel,
    processor: Processor,
    model_name: String,
}

impl LFM2VL {
    /// Load the model from a local directory containing all weight/config files.
    ///
    /// `path` is the directory that holds the downloaded model files.
    /// Pass `device = None` for auto-detection (CUDA → Metal → CPU).
    /// Pass `dtype = None` to follow the model's default dtype
    /// (BF16/F16 models on CPU automatically use F32 for correct matmul).
    /// Pass `force_f32 = true` to override and always use F32.
    /// Pass `max_tiles = Some(N)` to cap image tiles (smaller = faster prefill).
    pub fn load(
        path: &str,
        device: Option<&Device>,
        dtype: Option<DType>,
        force_f32: bool,
        max_tiles: Option<usize>,
    ) -> Result<Self> {
        let chat_template = ChatTemplate::init(path)?;
        let tokenizer = TokenizerModel::init(path)?;
        let device = get_device(device);
        let gen_cfg_path = path.to_string() + "/generation_config.json";
        let gen_cfg: Lfm2GenerateConfig = serde_json::from_slice(&std::fs::read(gen_cfg_path)?)?;
        let cfg_path = path.to_string() + "/config.json";
        let cfg: Lfm2VLConfig = serde_json::from_slice(&std::fs::read(cfg_path)?)?;

        let model_path = find_type_files(path, "safetensors")?;
        let dtype = get_dtype(dtype, &cfg.dtype, &device, force_f32);
        let vb = unsafe { VarBuilder::from_mmaped_safetensors(&model_path, dtype, &device)? };
        let eos_ids = vec![gen_cfg.eos_token_id];
        let model = LFM2VLModel::new(vb, &cfg, eos_ids)?;
        let processor = Processor::new(path, dtype, &device, max_tiles)?;
        let model_name = std::path::Path::new(path)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("lfm2.5-vl")
            .to_string();
        Ok(Self {
            chat_template,
            tokenizer,
            device,
            model,
            processor,
            model_name,
        })
    }

    /// Run inference — simple API without callbacks.
    ///
    /// `images` — one or more `DynamicImage`s (use [`rgba_to_dynamic`] to
    ///            convert from `image::RgbaImage`).
    /// `prompt` — user text; the `<image>` placeholder is auto-inserted.
    pub fn infer(
        &mut self,
        images: Vec<DynamicImage>,
        prompt: &str,
        max_tokens: u32,
        temperature: Option<f32>,
    ) -> Result<String> {
        let opts = InferOptions {
            max_tokens,
            temperature,
            ..Default::default()
        };
        let result = self.infer_with_options(images, prompt, &opts)?;
        Ok(result.text)
    }

    /// Run inference with full options (cancellation & progress callbacks).
    ///
    /// See [`InferOptions`] for available settings.
    pub fn infer_with_options(
        &mut self,
        images: Vec<DynamicImage>,
        prompt: &str,
        opts: &InferOptions,
    ) -> Result<InferResult> {
        // Build chat message with <image> placeholders + text
        let image_placeholders = "<image>".repeat(images.len());
        let user_content = format!("{}{}", image_placeholders, prompt);

        // Apply chat template
        let template_text = self.chat_template.render(&user_content)?;

        // Process images + expand text
        let (pixel_values, pixel_attention_mask, spatial_shapes, text) =
            self.processor.process_info(images, &template_text)?;

        // Tokenize — the chat template already includes the BOS token
        // ({{- bos_token -}} renders to <|startoftext|>), so we pass
        // add_special_tokens=false to avoid a doubled BOS prefix.
        // HF's processor also defaults to add_special_tokens=false.
        let input_ids = self.tokenizer.encode(&text, &self.device, false)?;

        // Autoregressive generation
        let eos_ids = self.model.stop_token_ids().to_vec();
        let seed = 299792458u64;
        let mut logit_processor = get_logit_processor(opts.temperature, opts.top_p, opts.top_k, seed);

        let mut generated: Vec<u32> = Vec::new();
        let mut seqlen_offset = 0usize;
        let mut cancelled = false;

        // Check cancel before starting
        if let Some(ref cancel) = opts.cancel {
            if cancel() {
                self.model.clear_cache();
                return Ok(InferResult {
                    text: String::new(),
                    token_count: 0,
                    cancelled: true,
                });
            }
        }

        // Initial forward with image data
        let logits = self.model.forward(
            &input_ids,
            Some(&pixel_values),
            Some(&pixel_attention_mask),
            Some(&spatial_shapes),
            seqlen_offset,
        )?;

        let next_token = sample_token(
            &logits,
            &mut logit_processor,
            &mut generated,
            opts.repeat_penalty,
            opts.repeat_last_n,
        )?;
        if let Some(ref on_token) = opts.on_token {
            on_token(next_token, 0);
        }
        let mut cur_ids = Tensor::from_vec(vec![next_token], (1, 1), &self.device)?;
        seqlen_offset += input_ids.dim(1)?;

        for step in 1..opts.max_tokens {
            // Check cancellation
            if let Some(ref cancel) = opts.cancel {
                if cancel() {
                    cancelled = true;
                    break;
                }
            }

            let logits = self.model.forward(&cur_ids, None, None, None, seqlen_offset)?;
            let next_token = sample_token(
                &logits,
                &mut logit_processor,
                &mut generated,
                opts.repeat_penalty,
                opts.repeat_last_n,
            )?;
            if let Some(ref on_token) = opts.on_token {
                on_token(next_token, step as usize);
            }
            if eos_ids.contains(&next_token) {
                break;
            }
            cur_ids = Tensor::from_vec(vec![next_token], (1, 1), &self.device)?;
            seqlen_offset += 1;
        }

        self.model.clear_cache();
        let token_count = generated.len();
        let text = self.tokenizer.decode(generated)?;

        Ok(InferResult {
            text,
            token_count,
            cancelled,
        })
    }

    /// Reference to the model name (derived from the directory path).
    pub fn model_name(&self) -> &str {
        &self.model_name
    }
}

// ── helpers ──────────────────────────────────────────────────────────

fn get_device(device: Option<&Device>) -> Device {
    match device {
        Some(d) => d.clone(),
        None => {
            #[cfg(feature = "cuda")]
            {
                Device::new_cuda(0).unwrap_or(Device::Cpu)
            }
            #[cfg(all(not(feature = "cuda"), feature = "metal"))]
            {
                Device::new_metal(0).unwrap_or(Device::Cpu)
            }
            #[cfg(all(not(feature = "cuda"), not(feature = "metal")))]
            {
                Device::Cpu
            }
        }
    }
}

fn get_dtype(dtype: Option<DType>, cfg_dtype: &str, device: &Device, force_f32: bool) -> DType {
    if force_f32 {
        return DType::F32;
    }
    match dtype {
        Some(d) => d,
        None => match cfg_dtype {
            "float32" | "float" => DType::F32,
            // CPU doesn't support BF16/F16 matmul natively — both are
            // implemented as F32 under the hood with extra conversion
            // overhead.  F32 is actually faster on CPU.
            // On GPU (CUDA/Metal), use the model's native dtype.
            "bfloat16" => {
                if device.is_cpu() {
                    DType::F32
                } else {
                    DType::BF16
                }
            }
            "float16" => {
                if device.is_cpu() {
                    DType::F32
                } else {
                    DType::F16
                }
            }
            _ => DType::F32,
        },
    }
}

fn find_type_files(path: &str, extension_type: &str) -> Result<Vec<String>> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let file_path = entry.path();
        if file_path.is_file()
            && let Some(extension) = file_path.extension()
            && extension == extension_type
        {
            files.push(file_path.to_string_lossy().to_string());
        }
    }
    Ok(files)
}

fn get_logit_processor(
    temperature: Option<f32>,
    top_p: Option<f32>,
    top_k: Option<usize>,
    seed: u64,
) -> LogitsProcessor {
    let temperature = temperature.and_then(|v| if v < 1e-7 { None } else { Some(v) });
    match top_k {
        None => LogitsProcessor::new(
            seed,
            temperature.map(|t| t as f64),
            top_p.map(|p| p as f64),
        ),
        Some(k) => {
            use candle_transformers::generation::Sampling;
            let sampling = match temperature {
                None => Sampling::ArgMax,
                Some(t) => match top_p {
                    None => Sampling::TopK {
                        k,
                        temperature: t as f64,
                    },
                    Some(p) => Sampling::TopKThenTopP {
                        k,
                        p: p as f64,
                        temperature: t as f64,
                    },
                },
            };
            LogitsProcessor::from_sampling(seed, sampling)
        }
    }
}

fn sample_token(
    logits: &Tensor,
    processor: &mut LogitsProcessor,
    generated: &mut Vec<u32>,
    repeat_penalty: f32,
    repeat_last_n: usize,
) -> Result<u32> {
    let mut logits = logits.squeeze(0)?.squeeze(0)?.to_dtype(DType::F32)?;
    if repeat_penalty != 1.0 && repeat_last_n > 0 && !generated.is_empty() {
        let start_at = generated.len().saturating_sub(repeat_last_n);
        logits = candle_transformers::utils::apply_repeat_penalty(
            &logits,
            repeat_penalty,
            &generated[start_at..],
        )?;
    }
    let token = processor.sample(&logits)?;
    generated.push(token);
    Ok(token)
}
