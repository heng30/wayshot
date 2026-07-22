//! Voice cloning smoke test: generate speech using a reference and/or prompt wav.
//!
//! Modes (controlled via env vars):
//!   - **Reference** (default): isolates the reference voice via REF_AUDIO tokens.
//!     No transcript needed.
//!   - **Continuation**: set `PROMPT_TEXT` to the transcript of the wav.
//!     Model continues from the prompt audio in the same speaker's voice.
//!     Set `MODE=continuation` to use the wav as a continuation prompt only.
//!   - **Combined**: set `MODE=combined` and `PROMPT_TEXT`. Uses the wav as
//!     both a reference prefix and a continuation prompt.
//!
//! Usage:
//!   cargo run --release --example clone --no-default-features --features wgpu -- \
//!       <model_dir> <wav> "target text" /tmp/out.wav
//!
//!   PROMPT_TEXT="..." MODE=continuation cargo run ... -- ...
//!   PROMPT_TEXT="..." MODE=combined     cargo run ... -- ...

#![recursion_limit = "256"]

use std::env;
use std::path::PathBuf;
use std::time::Instant;

use voxcpm_rs::{audio, GenerateOptions, Prompt, VoxCPM};

#[cfg(feature = "wgpu")]
type B = burn::backend::Wgpu<f32, i32>;
#[cfg(all(not(feature = "wgpu"), feature = "cpu"))]
type B = burn::backend::NdArray<f32>;

fn main() {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or(
            "info,wgpu_hal=error,wgpu_core=error,naga=error,cubecl_wgpu=warn",
        ),
    )
    .init();
    let mut args = env::args().skip(1);
    let model_dir = args
        .next()
        .unwrap_or_else(|| "/home/nipah/dev/ai_space/VoxCPM2".to_string());
    let wav = args
        .next()
        .unwrap_or_else(|| "vendor/VoxCPM/examples/reference_speaker.wav".to_string());
    let text = args
        .next()
        .unwrap_or_else(|| "Hello world, this is a voice cloning test.".to_string());
    let out = args.next().unwrap_or_else(|| "/tmp/clone.wav".to_string());

    let prompt_text = env::var("PROMPT_TEXT").ok();
    let mode = env::var("MODE").unwrap_or_else(|_| "reference".to_string());

    let prompt = match mode.as_str() {
        "reference" => Prompt::Reference {
            audio: PathBuf::from(&wav).into(),
        },
        "continuation" => Prompt::Continuation {
            audio: PathBuf::from(&wav).into(),
            text: prompt_text.expect("MODE=continuation requires PROMPT_TEXT env var"),
        },
        "combined" => Prompt::Combined {
            reference_audio: PathBuf::from(&wav).into(),
            prompt_audio: PathBuf::from(&wav).into(),
            prompt_text: prompt_text.expect("MODE=combined requires PROMPT_TEXT env var"),
        },
        other => panic!("unknown MODE={other:?} (expected reference|continuation|combined)"),
    };

    let device = Default::default();
    let t0 = Instant::now();
    eprintln!("loading model from {model_dir} ...");
    let model: VoxCPM<B> = VoxCPM::from_local(&model_dir, &device).expect("load model");
    eprintln!("loaded in {:.2?}", t0.elapsed());
    eprintln!("mode: {mode}");
    eprintln!("wav:  {wav}");
    eprintln!("synthesizing: {text:?}");

    let timesteps = env::var("VOXCPM_TIMESTEPS")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(10);
    let opts = GenerateOptions::builder()
        .timesteps(timesteps)
        .cfg(2.0)
        .max_len(500)
        .prompt(prompt)
        .build();
    let t1 = Instant::now();
    let wav_out = model.generate(&text, opts).expect("generate");
    let elapsed = t1.elapsed();
    let sr = model.sample_rate();
    let audio_sec = wav_out.len() as f32 / sr as f32;
    eprintln!(
        "got {} samples @ {} Hz ({:.2}s of audio) in {:.2?} (RTF = {:.2})",
        wav_out.len(),
        sr,
        audio_sec,
        elapsed,
        elapsed.as_secs_f32() / audio_sec
    );
    eprintln!("writing {out}");
    audio::write_wav(&out, &wav_out, sr).expect("write wav");
    eprintln!("done");
}

