use anyhow::{Result, anyhow};
use audio_utils::loader;
use fun_ast_nano::{FunASRModelConfig, FunAsrNanoGenerateModel, TranscriptionRequest};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use video_utils::subtitle::{Subtitle, ms_to_srt_timestamp, save_as_srt};

const DEFAULT_PROMPT: &str = "请将这段音频转写为文字。";

#[derive(clap::Args)]
pub struct Args {
    /// 音频（视频也可以，会先解码音轨）
    pub input: PathBuf,

    /// 模型权重文件（model.pt / safetensors）
    #[arg(long)]
    pub weights: PathBuf,

    /// tokenizer 文件
    #[arg(long)]
    pub tokenizer: PathBuf,

    /// 纯文本结果写到该文件（不填则只输出 JSON）
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// 输出带时间轴的 SRT
    #[arg(long)]
    pub srt: Option<PathBuf>,

    #[arg(long, default_value_t = 512)]
    pub max_tokens: u32,

    #[arg(long)]
    pub prompt: Option<String>,

    #[arg(long, default_value_t = 0.01)]
    pub vad_threshold: f32,
}

pub fn run(args: &Args) -> Result<Value> {
    let audio = loader::load_audio_file_and_convert(
        &args.input,
        fun_ast_nano::INPUT_AUDIO_CHANNELS as u16,
        fun_ast_nano::INPUT_AUDIO_SAMPLE_RATE,
    )
    .map_err(|e| anyhow!("无法读取音频 {}: {e}", args.input.display()))?;

    let model_config = FunASRModelConfig::default()
        .with_model_weights(
            args.weights
                .to_str()
                .ok_or_else(|| anyhow!("权重路径不是有效 UTF-8"))?
                .to_string(),
        )
        .with_tokenizer_path(
            args.tokenizer
                .to_str()
                .ok_or_else(|| anyhow!("tokenizer 路径不是有效 UTF-8"))?
                .to_string(),
        );

    let mut model = FunAsrNanoGenerateModel::new(model_config, None, None)
        .map_err(|e| anyhow!("加载模型失败: {e}"))?;

    let prompt = args
        .prompt
        .clone()
        .unwrap_or_else(|| DEFAULT_PROMPT.to_string());

    if let Some(srt_path) = &args.srt {
        return transcribe_to_srt(
            &mut model,
            &audio,
            &prompt,
            args.max_tokens,
            args.vad_threshold,
            srt_path,
        );
    }

    let request = TranscriptionRequest::default()
        .with_audio_config(audio)
        .with_prompt(Some(prompt))
        .with_max_tokens(args.max_tokens);

    let response = model
        .generate(request, None, |_| Ok(()))
        .map_err(|e| anyhow!("转写失败: {e}"))?;

    let text = fast2s::convert(&response.text);
    if let Some(path) = &args.output {
        std::fs::write(path, &text)?;
    }

    Ok(json!({
        "text": text,
        "tokens": response.num_tokens,
        "path": args.output.as_ref().map(|p| p.display().to_string()),
    }))
}

fn transcribe_to_srt(
    model: &mut FunAsrNanoGenerateModel,
    audio: &audio_utils::loader::AudioConfig,
    prompt: &str,
    max_tokens: u32,
    threshold: f32,
    srt_path: &Path,
) -> Result<Value> {
    let sample_rate = fun_ast_nano::INPUT_AUDIO_SAMPLE_RATE;
    let vad_config = audio_utils::vad::VadConfig::default()
        .with_sample_rate(sample_rate)
        .with_speech_threshold(threshold);
    let segments = audio_utils::vad::detect_speech_segments(&audio.samples, &vad_config);

    let mut subtitles = Vec::with_capacity(segments.len());
    for (index, segment) in segments.iter().enumerate() {
        let seg_audio = audio_utils::loader::AudioConfig::default()
            .with_sample_rate(sample_rate)
            .with_channel(fun_ast_nano::INPUT_AUDIO_CHANNELS as u16)
            .with_duration(std::time::Duration::from_secs_f64(
                segment.audio_data.len() as f64 / sample_rate as f64,
            ))
            .with_samples(segment.audio_data.clone());

        let request = TranscriptionRequest::default()
            .with_audio_config(seg_audio)
            .with_prompt(Some(prompt.to_string()))
            .with_max_tokens(max_tokens);

        let text = match model.generate(request, None, |_| Ok(())) {
            Ok(response) => fast2s::convert(&response.text),
            Err(e) => return Err(anyhow!("第 {} 段转写失败: {e}", index + 1)),
        };
        if text.trim().is_empty() {
            continue;
        }

        let start_ms = segment.start_sample as f64 / sample_rate as f64 * 1000.0;
        let end_ms = segment.end_sample as f64 / sample_rate as f64 * 1000.0;
        subtitles.push(Subtitle {
            index: 0,
            start_timestamp: start_ms as u64,
            end_timestamp: end_ms as u64,
            text,
        });
    }

    for (index, subtitle) in subtitles.iter_mut().enumerate() {
        subtitle.index = index as u32 + 1;
    }

    save_as_srt(&subtitles, srt_path).map_err(|e| anyhow!("写入 SRT 失败: {e}"))?;

    Ok(json!({
        "path": srt_path.display().to_string(),
        "count": subtitles.len(),
        "first": subtitles
            .first()
            .map(|s| ms_to_srt_timestamp(s.start_timestamp)),
    }))
}
