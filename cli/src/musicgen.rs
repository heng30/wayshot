use anyhow::{Result, anyhow};
use clap::{Subcommand, ValueEnum};
use musicgen_rs::{DecoderMode, Model, MusicGen};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

use crate::util;

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum Variant {
    Small,
    SmallFp16,
    SmallQuant,
    Medium,
    MediumFp16,
    MediumQuant,
    Large,
}

impl Variant {
    fn to_model(self) -> Model {
        match self {
            Variant::Small => Model::Small,
            Variant::SmallFp16 => Model::SmallFp16,
            Variant::SmallQuant => Model::SmallQuant,
            Variant::Medium => Model::Medium,
            Variant::MediumFp16 => Model::MediumFp16,
            Variant::MediumQuant => Model::MediumQuant,
            Variant::Large => Model::Large,
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
pub enum Decoder {
    Merged,
    Split,
}

#[derive(Subcommand)]
pub enum Cmd {
    /// 文本生成音乐（MusicGen）
    Generate {
        /// 音乐描述提示词
        #[arg(long)]
        prompt: String,
        #[arg(short, long)]
        output: PathBuf,
        /// 模型目录（含 config.json / tokenizer.json / *.onnx）
        #[arg(long)]
        model: PathBuf,
        #[arg(long, value_enum, default_value_t = Variant::Small)]
        variant: Variant,
        #[arg(long, value_enum, default_value_t = Decoder::Merged)]
        decoder: Decoder,
        /// 生成时长（1-30 秒）
        #[arg(long, default_value_t = 10)]
        seconds: usize,
    },
}

pub fn run(cmd: &Cmd) -> Result<Value> {
    match cmd {
        Cmd::Generate {
            prompt,
            output,
            model,
            variant,
            decoder,
            seconds,
        } => generate(prompt, output, model, *variant, *decoder, *seconds),
    }
}

fn generate(
    prompt: &str,
    output: &Path,
    model_dir: &Path,
    variant: Variant,
    decoder: Decoder,
    seconds: usize,
) -> Result<Value> {
    let mut model = MusicGen::load(
        model_dir,
        variant.to_model(),
        match decoder {
            Decoder::Merged => DecoderMode::Merged,
            Decoder::Split => DecoderMode::Split,
        },
    )
    .map_err(|e| anyhow!("加载模型失败: {e}"))?;

    let audio = model
        .generate_simple(prompt, seconds)
        .map_err(|e| anyhow!("生成失败: {e}"))?;

    util::write_wav(output, &audio.samples, audio.sample_rate, audio.channels)?;

    Ok(json!({
        "path": output.display().to_string(),
        "seconds": seconds,
        "sample_rate": audio.sample_rate,
        "channels": audio.channels,
        "samples": audio.samples.len(),
    }))
}
