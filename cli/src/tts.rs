use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::path::PathBuf;
use voxcpm_rs::{GenerateOptions, Prompt, PromptAudio, VoxCPM};

type Backend = burn::backend::NdArray<f32>;

#[derive(clap::Args)]
pub struct Args {
    /// 要合成的文本
    #[arg(long)]
    pub text: String,

    #[arg(short, long)]
    pub output: PathBuf,

    /// VoxCPM2 模型目录（含 config.json 与 tokenizer 文件）
    #[arg(long)]
    pub model: PathBuf,

    /// 参考音频文件，用于克隆音色
    #[arg(long)]
    pub reference: Option<PathBuf>,

    /// 引导强度
    #[arg(long, default_value_t = 2.0)]
    pub cfg: f32,

    /// 扩散采样步数
    #[arg(long, default_value_t = 10)]
    pub steps: usize,

    /// 生成潜变量上限，越大可生成越长语音
    #[arg(long, default_value_t = 2000)]
    pub max_len: usize,
}

pub fn run(args: &Args) -> Result<Value> {
    let device = Default::default();
    let model = VoxCPM::<Backend>::from_local(&args.model, &device)
        .map_err(|e| anyhow::anyhow!("加载模型失败: {e}"))?;

    let mut options = GenerateOptions {
        cfg_value: args.cfg,
        inference_timesteps: args.steps,
        max_len: args.max_len,
        ..Default::default()
    };
    if let Some(reference) = &args.reference {
        options.prompt = Prompt::Reference {
            audio: PromptAudio::File(reference.clone()),
        };
    }

    let samples = model
        .generate(&args.text, options)
        .map_err(|e| anyhow::anyhow!("合成失败: {e}"))?;

    let sample_rate = model.sample_rate();
    voxcpm_rs::audio::write_wav(&args.output, &samples, sample_rate)
        .with_context(|| format!("无法写入 WAV: {}", args.output.display()))?;

    Ok(json!({
        "path": args.output.display().to_string(),
        "sample_rate": sample_rate,
        "duration_secs": samples.len() as f64 / sample_rate as f64,
    }))
}
