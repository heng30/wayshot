//! CLI example for musicgen-rs.
//!
//! Generates music from a text prompt and saves it as a WAV file.
//!
//! # Usage
//!
//! ```sh
//! # First, download the ONNX model files (one-time setup):
//! cargo run --example download_model -- /path/to/model-dir
//!
//! # Then generate audio:
//! cargo run --example cli -- --model-dir /path/to/model-dir --prompt "80s pop song with synth" --secs 10
//! ```

use anyhow::{Context, Result};
use clap::Parser;
use musicgen_rs::{DecoderMode, Model, MusicGen};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "musicgen-cli", about = "Generate music from text using MusicGen")]
struct Args {
    /// Path to the directory containing ONNX model files.
    #[arg(long)]
    model_dir: PathBuf,

    /// The model variant to use.
    #[arg(long, default_value = "small")]
    model: ModelArg,

    /// Whether the decoder model is split (two files) or merged (one file).
    #[arg(long, default_value = "merged")]
    decoder: DecoderArg,

    /// Text prompt describing the music to generate.
    #[arg(short, long)]
    prompt: String,

    /// Duration of audio to generate in seconds (1–30).
    #[arg(short, long, default_value = "10")]
    secs: usize,

    /// Output WAV file path.
    #[arg(short, long, default_value = "output.wav")]
    output: PathBuf,
}

#[derive(Clone, Copy, clap::ValueEnum)]
enum ModelArg {
    Small,
    SmallFp16,
    SmallQuant,
    Medium,
    MediumFp16,
    MediumQuant,
    Large,
}

impl From<ModelArg> for Model {
    fn from(v: ModelArg) -> Self {
        match v {
            ModelArg::Small => Model::Small,
            ModelArg::SmallFp16 => Model::SmallFp16,
            ModelArg::SmallQuant => Model::SmallQuant,
            ModelArg::Medium => Model::Medium,
            ModelArg::MediumFp16 => Model::MediumFp16,
            ModelArg::MediumQuant => Model::MediumQuant,
            ModelArg::Large => Model::Large,
        }
    }
}

#[derive(Clone, Copy, clap::ValueEnum)]
enum DecoderArg {
    Merged,
    Split,
}

impl From<DecoderArg> for DecoderMode {
    fn from(v: DecoderArg) -> Self {
        match v {
            DecoderArg::Merged => DecoderMode::Merged,
            DecoderArg::Split => DecoderMode::Split,
        }
    }
}

fn main() -> Result<()> {
    env_logger::init();

    let args = Args::parse();

    println!("Loading model from {:?}...", args.model_dir);
    let mut musicgen = MusicGen::load(&args.model_dir, args.model.into(), args.decoder.into())
        .context("Failed to load model")?;

    println!("Generating {} seconds of audio...", args.secs);
    let output = musicgen
        .generate(
            &args.prompt,
            args.secs,
            Box::new(|current, total| {
                let pct = (current / total * 100.0) as u32;
                eprint!("\rProgress: {pct}%  ");
                false
            }),
        )
        .context("Generation failed")?;
    eprintln!();

    // Write WAV file (16-bit PCM for maximum compatibility)
    let spec = hound::WavSpec {
        channels: output.channels,
        sample_rate: output.sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer = hound::WavWriter::create(&args.output, spec)
        .with_context(|| format!("Failed to create WAV file at {:?}", args.output))?;
    for sample in &output.samples {
        // Clamp to [-1.0, 1.0] and convert to i16
        let clamped = sample.clamp(-1.0, 1.0);
        let i16_sample = (clamped * 32767.0) as i16;
        writer.write_sample(i16_sample)?;
    }
    writer.finalize()?;

    println!(
        "Saved {} seconds of audio ({} Hz, {} ch) to {:?}",
        args.secs, output.sample_rate, output.channels, args.output
    );

    Ok(())
}
