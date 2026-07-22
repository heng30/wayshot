//! LFM2.5-VL inference library
//!
//! Vision-language model inference using candle.
//!
//! # Supported models
//!
//! | Model | Size | Source |
//! |-------|------|--------|
//! | LFM2.5-VL-450M | 450M params | <https://huggingface.co/Liquid1/LFM2.5-VL-450M> |
//! | LFM2.5-VL-1.6B | 1.6B params | <https://huggingface.co/Liquid1/LFM2.5-VL-1.6B> |
//!
//! Both models use the same architecture; just point [`LFM2VL::load`] at the
//! appropriate directory. Download all model files (config.json, tokenizer.json,
//! model.safetensors, processor_config.json, generation_config.json,
//! tokenizer_config.json, chat_template.jinja) into a local directory.
//!
//! # Quick start
//!
//! ```no_run
//! use lfm_vl_rs::lfm2vl::generate::{LFM2VL, rgba_to_dynamic};
//! use lfm_vl_rs::error::LfmError;
//!
//! // Load model (CPU auto-detect, default dtype)
//! let mut model = LFM2VL::load("/path/to/LFM2.5-VL-450M", None, None, false, None)?;
//!
//! // Load an image as RgbaImage and convert
//! let img = image::open("photo.png")?;
//! let rgba: image::RgbaImage = img.to_rgba8();
//! let dyn_img = rgba_to_dynamic(rgba);
//!
//! // Run inference
//! let text = model.infer(
//!     vec![dyn_img],          // images (DynamicImage)
//!     "Describe this image.", // prompt
//!     512,                    // max tokens
//!     Some(0.3),              // temperature (None = greedy)
//! )?;
//! println!("{}", text);
//! # Ok::<(), LfmError>(())
//! ```
//!
//! # Cancellation & progress callbacks
//!
//! Use [`InferOptions`] to cancel generation early or receive per-token
//! progress:
//!
//! ```no_run
//! use std::sync::atomic::{AtomicBool, Ordering};
//! use std::sync::Arc;
//! use lfm_vl_rs::lfm2vl::generate::{LFM2VL, InferOptions, InferResult, rgba_to_dynamic};
//! use lfm_vl_rs::error::LfmError;
//!
//! let mut model = LFM2VL::load("/path/to/LFM2.5-VL-450M", None, None, false, None)?;
//! let dyn_img = rgba_to_dynamic(image::open("photo.png")?.to_rgba8());
//!
//! // Cancellation flag (e.g. set by Ctrl-C handler)
//! let cancel_flag = Arc::new(AtomicBool::new(false));
//! let cancel_clone = cancel_flag.clone();
//!
//! let opts = InferOptions {
//!     max_tokens: 512,
//!     temperature: Some(0.3),
//!     // Return true to stop generation early
//!     cancel: Some(Arc::new(move || cancel_clone.load(Ordering::SeqCst))),
//!     // Called after each token: (token_id, step_index)
//!     on_token: Some(Arc::new(|_tid, step| {
//!         if step % 50 == 0 { eprintln!("{} tokens", step + 1); }
//!     })),
//!     ..Default::default()
//! };
//!
//! let result: InferResult = model.infer_with_options(
//!     vec![dyn_img],
//!     "请描述这张图片",
//!     &opts,
//! )?;
//!
//! if result.cancelled {
//!     eprintln!("Generation was cancelled");
//! }
//! println!("{}", result.text);
//! # Ok::<(), LfmError>(())
//! ```

pub mod error;
pub mod lfm2vl;
pub mod lfm2;
pub mod rope;
pub mod util;
pub mod chat_template;
pub mod tokenizer;
