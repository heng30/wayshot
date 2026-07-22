//! Download MusicGen ONNX model files from HuggingFace.
//!
//! This is a one-time setup step. The original MusicGPT project exports
//! MusicGen models to ONNX format using HuggingFace's `optimum` library
//! and hosts them at `https://huggingface.co/gabotechs/music_gen`.
//!
//! # Usage
//!
//! ```sh
//! cargo run --example download_model -- /path/to/model-dir
//! # Optionally select a model variant:
//! cargo run --example download_model -- /path/to/model-dir --model small-fp16
//! ```

use anyhow::{Context, Result};
use clap::Parser;
use musicgen_rs::Model;
use std::fs;
use std::path::PathBuf;

#[derive(Clone, Copy, clap::ValueEnum)]
enum ModelVariant {
    Small,
    SmallFp16,
    SmallQuant,
    Medium,
    MediumFp16,
    MediumQuant,
    Large,
}

impl ModelVariant {
    fn to_model(&self) -> Model {
        match self {
            ModelVariant::Small => Model::Small,
            ModelVariant::SmallFp16 => Model::SmallFp16,
            ModelVariant::SmallQuant => Model::SmallQuant,
            ModelVariant::Medium => Model::Medium,
            ModelVariant::MediumFp16 => Model::MediumFp16,
            ModelVariant::MediumQuant => Model::MediumQuant,
            ModelVariant::Large => Model::Large,
        }
    }
}

#[derive(Parser)]
#[command(
    name = "download_model",
    about = "Download MusicGen ONNX models from HuggingFace"
)]
struct Args {
    /// Directory to save model files to.
    output_dir: PathBuf,

    /// Model variant to download.
    #[arg(long, default_value = "small")]
    model: ModelVariant,

    /// Force re-download even if files exist.
    #[arg(long)]
    force: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let model = args.model.to_model();

    fs::create_dir_all(&args.output_dir)
        .with_context(|| format!("Failed to create directory {:?}", args.output_dir))?;

    let files = model.file_spec();
    let total = files.len();

    for (i, (remote, local)) in files.iter().enumerate() {
        let local_path = args.output_dir.join(local);

        if local_path.exists() && !args.force {
            println!("[{}/{}] {} already exists, skipping", i + 1, total, local);
            continue;
        }

        let url = model.download_url(&(remote, local));
        println!("[{}/{}] Downloading {}...", i + 1, total, remote);

        // Use curl for downloading — simple and reliable
        let status = std::process::Command::new("curl")
            .args(["-L", "-o"])
            .arg(&local_path)
            .arg(&url)
            .status()
            .context("Failed to run curl. Is it installed?")?;

        if !status.success() {
            anyhow::bail!("Failed to download {}", url);
        }
    }

    println!("\nAll files downloaded to {:?}", args.output_dir);
    println!("You can now run the CLI example:");
    println!(
        "  cargo run --example cli -- --model-dir {:?} --prompt \"your prompt\" --secs 10",
        args.output_dir
    );

    Ok(())
}
