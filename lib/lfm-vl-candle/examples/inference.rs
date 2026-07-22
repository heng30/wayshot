//! LFM2.5-VL inference example
//!
//! Usage:
//!   cargo run --example infer -- <model_dir> <image_path> [prompt]
//!
//! Example:
//!   cargo run --example infer -- /home/blue/models/LFM2.5-VL-450M photo.png "Describe this image."
//!
//! Model download:
//!   LFM2.5-VL-450M: https://huggingface.co/Liquid1/LFM2.5-VL-450M
//!   LFM2.5-VL-1.6B: https://huggingface.co/Liquid1/LFM2.5-VL-1.6B

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use image::DynamicImage;
use lfm_vl_candle::lfm2vl::generate::{InferOptions, LFM2VL};

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: cargo run --example infer -- <model_dir> <image_path> [prompt]");
        eprintln!();
        eprintln!("Models:");
        eprintln!("  LFM2.5-VL-450M: https://huggingface.co/Liquid1/LFM2.5-VL-450M");
        eprintln!("  LFM2.5-VL-1.6B: https://huggingface.co/Liquid1/LFM2.5-VL-1.6B");
        std::process::exit(1);
    }
    let model_dir = &args[1];
    let image_path = &args[2];
    let prompt = if args.len() > 3 {
        args[3..].join(" ")
    } else {
        "Describe this image.".to_string()
    };

    // ── Load model ───────────────────────────────────────────────────
    eprintln!("[info] Loading model from {} …", model_dir);
    let t0 = Instant::now();
    let mut model = LFM2VL::load(model_dir, None, None, false, Some(2))?;
    eprintln!("[info] Model loaded in {:.1}s", t0.elapsed().as_secs_f64());

    // ── Load image as RGBA, no pre-resize — let the processor handle it ──
    let dyn_img = {
        let img = image::ImageReader::open(image_path)?
            .with_guessed_format()?
            .decode()?;
        eprintln!("[info] Image size {}x{}", img.width(), img.height());
        DynamicImage::ImageRgba8(img.to_rgba8())
    };

    // ── Set up cancellation (Ctrl-C) ────────────────────────────────
    let cancelled = Arc::new(AtomicBool::new(false));
    let cancelled_clone = cancelled.clone();
    ctrlc::set_handler(move || {
        cancelled_clone.store(true, Ordering::SeqCst);
        eprintln!("\n[cancel] Interrupt requested, stopping generation …");
    })?;

    // ── Run inference with progress callback ─────────────────────────
    eprintln!("[info] Prompt: {}", prompt);
    eprintln!("[info] Generating …");
    let t0 = Instant::now();

    let opts = InferOptions {
        max_tokens: 512,
        temperature: Some(0.3), // low temperature: focused, compact output
        repeat_penalty: 1.1,
        repeat_last_n: 64,
        cancel: Some(Arc::new(move || cancelled.load(Ordering::SeqCst))),
        on_token: Some(Arc::new(|_token_id, step| {
            if step % 50 == 0 {
                eprintln!("[progress] {} tokens generated", step + 1);
            }
        })),
        ..Default::default()
    };

    let result = model.infer_with_options(vec![dyn_img], &prompt, &opts)?;
    let elapsed = t0.elapsed().as_secs_f64();

    // ── Print result ─────────────────────────────────────────────────
    eprintln!();
    if result.cancelled {
        eprintln!("[warn] Generation was cancelled (partial result)");
    }
    eprintln!(
        "[info] {} tokens in {:.1}s ({:.1} tok/s)",
        result.token_count,
        elapsed,
        result.token_count as f64 / elapsed,
    );
    println!("{}", result.text);

    Ok(())
}
