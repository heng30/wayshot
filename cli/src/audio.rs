use anyhow::{Context, Result, anyhow};
use audio_utils::{audio_level, loader, vad};
use clap::Subcommand;
use record_audio::AudioRecorder;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::util;

#[derive(Subcommand)]
pub enum Cmd {
    /// 音频文件信息（采样率、声道、时长、电平）
    Info { input: PathBuf },

    /// 语音活动检测（VAD），输出语音片段的时间区间
    Vad {
        input: PathBuf,
        /// 能量阈值，越低越灵敏
        #[arg(long, default_value_t = 0.01)]
        threshold: f32,
        #[arg(long, default_value_t = 250)]
        min_speech_ms: u32,
        #[arg(long, default_value_t = 200)]
        min_silence_ms: u32,
        /// 片段转成 WAV 输出到该目录
        #[arg(long)]
        output_dir: Option<PathBuf>,
    },

    /// 调整音量（dB）
    Gain {
        input: PathBuf,
        output: PathBuf,
        #[arg(long, allow_hyphen_values = true)]
        db: f32,
    },

    /// 变速不变调
    Speed {
        input: PathBuf,
        output: PathBuf,
        /// 速度倍率，>1 更快
        #[arg(long, default_value_t = 1.0)]
        speed: f32,
    },

    /// 重采样 / 转声道，输出 WAV
    Resample {
        input: PathBuf,
        output: PathBuf,
        #[arg(long)]
        sample_rate: u32,
        #[arg(long, default_value_t = 1)]
        channels: u16,
    },

    /// 列出录音设备
    Devices,

    /// 说话人分离（pyannote 模型），输出说话人时间轴
    Speakers {
        input: PathBuf,
        /// 模型目录
        #[arg(long)]
        models: PathBuf,
        /// 相邻同说话人片段合并的最大间隔（秒）
        #[arg(long, default_value_t = 0.5)]
        merge_gap: f64,
        /// 结果另存为 JSON
        #[arg(long)]
        output: Option<PathBuf>,
    },

    /// 降噪（DeepFilterNet），输出 48kHz 单声道 WAV
    Denoise {
        input: PathBuf,
        output: PathBuf,
        /// DeepFilterNet 模型目录
        #[arg(long)]
        model: PathBuf,
        /// ONNX intra-op 线程数
        #[arg(long)]
        threads: Option<usize>,
    },

    /// 用麦克风录音，输出 WAV
    Record {
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long, default_value_t = 5.0)]
        seconds: f64,
        /// 设备名，缺省为系统默认输入设备
        #[arg(long)]
        device: Option<String>,
        #[arg(long, default_value_t = 0.0, allow_hyphen_values = true)]
        gain_db: f32,
        /// 录成单声道
        #[arg(long)]
        mono: bool,
    },
}

pub fn run(cmd: &Cmd) -> Result<Value> {
    match cmd {
        Cmd::Info { input } => info(input),
        Cmd::Vad {
            input,
            threshold,
            min_speech_ms,
            min_silence_ms,
            output_dir,
        } => detect_vad(
            input,
            *threshold,
            *min_speech_ms,
            *min_silence_ms,
            output_dir.as_deref(),
        ),
        Cmd::Gain { input, output, db } => gain(input, output, *db),
        Cmd::Speed {
            input,
            output,
            speed,
        } => speed_change(input, output, *speed),
        Cmd::Resample {
            input,
            output,
            sample_rate,
            channels,
        } => resample(input, output, *sample_rate, *channels),
        Cmd::Speakers {
            input,
            models,
            merge_gap,
            output,
        } => speakers(input, models, *merge_gap, output.as_deref()),
        Cmd::Denoise {
            input,
            output,
            model,
            threads,
        } => denoise(input, output, model, *threads),
        Cmd::Devices => devices(),
        Cmd::Record {
            output,
            seconds,
            device,
            gain_db,
            mono,
        } => record(output, *seconds, device.as_deref(), *gain_db, *mono),
    }
}

const VAD_SAMPLE_RATE: u32 = 16_000;

fn info(input: &Path) -> Result<Value> {
    let config = loader::load_audio_file(input)
        .map_err(|e| anyhow!("无法读取音频 {}: {e}", input.display()))?;

    Ok(json!({
        "path": input.display().to_string(),
        "sample_rate": config.sample_rate,
        "channels": config.channel,
        "duration_secs": config.duration.as_secs_f64(),
        "samples": config.samples.len() / config.channel.max(1) as usize,
        "rms_db": audio_level::calc_rms_level(&config.samples),
        "peak_db": audio_level::calc_peak_level(&config.samples),
        "lufs_db": audio_level::calc_lufs_style(&config.samples),
    }))
}

fn detect_vad(
    input: &Path,
    threshold: f32,
    min_speech_ms: u32,
    min_silence_ms: u32,
    output_dir: Option<&Path>,
) -> Result<Value> {
    let config = loader::load_audio_file_and_convert(input, 1, VAD_SAMPLE_RATE)
        .map_err(|e| anyhow!("无法读取音频 {}: {e}", input.display()))?;

    let vad_config = vad::VadConfig::default()
        .with_sample_rate(VAD_SAMPLE_RATE)
        .with_speech_threshold(threshold)
        .with_min_speech_duration_ms(min_speech_ms)
        .with_min_silence_duration_ms(min_silence_ms);

    let segments = vad::detect_speech_segments(&config.samples, &vad_config);

    if let Some(dir) = output_dir {
        std::fs::create_dir_all(dir)?;
    }

    let mut items = Vec::with_capacity(segments.len());
    for (i, segment) in segments.iter().enumerate() {
        let start = segment.start_sample as f64 / VAD_SAMPLE_RATE as f64;
        let end = segment.end_sample as f64 / VAD_SAMPLE_RATE as f64;
        let mut item = json!({
            "start_secs": start,
            "end_secs": end,
            "duration_secs": end - start,
        });
        if let Some(dir) = output_dir {
            let path = dir.join(format!("speech_{:05}.wav", i));
            util::write_wav(&path, &segment.audio_data, VAD_SAMPLE_RATE, 1)?;
            item["path"] = json!(path.display().to_string());
        }
        items.push(item);
    }

    Ok(json!({
        "count": items.len(),
        "speech_secs": items.iter().map(|v| v["duration_secs"].as_f64().unwrap_or(0.0)).sum::<f64>(),
        "segments": items,
    }))
}

fn gain(input: &Path, output: &Path, db: f32) -> Result<Value> {
    let mut config = loader::load_audio_file(input)
        .map_err(|e| anyhow!("无法读取音频 {}: {e}", input.display()))?;

    audio_level::apply_gain(&mut config.samples, db);
    util::write_wav(output, &config.samples, config.sample_rate, config.channel)?;

    Ok(json!({
        "path": output.display().to_string(),
        "db": db,
        "rms_db": audio_level::calc_rms_level(&config.samples),
        "peak_db": audio_level::calc_peak_level(&config.samples),
    }))
}

fn speed_change(input: &Path, output: &Path, speed: f32) -> Result<Value> {
    if speed <= 0.0 {
        return Err(anyhow!("speed 必须大于 0"));
    }

    let config = loader::load_audio_file(input)
        .map_err(|e| anyhow!("无法读取音频 {}: {e}", input.display()))?;

    // stretch_ratio 2.0 表示放慢一倍，所以变速倍率取倒数
    let samples = audio_utils::time_stretch_preserving_pitch(
        &config.samples,
        1.0 / speed,
        config.channel,
        config.sample_rate,
    );

    util::write_wav(output, &samples, config.sample_rate, config.channel)?;

    Ok(json!({
        "path": output.display().to_string(),
        "speed": speed,
        "duration_secs": samples.len() as f64 / (config.channel as f64 * config.sample_rate as f64),
    }))
}

fn resample(input: &Path, output: &Path, sample_rate: u32, channels: u16) -> Result<Value> {
    let samples = loader::load_audio_file_and_convert(input, channels, sample_rate)
        .map_err(|e| anyhow!("无法读取音频 {}: {e}", input.display()))?;

    util::write_wav(output, &samples.samples, sample_rate, channels)?;

    Ok(json!({
        "path": output.display().to_string(),
        "sample_rate": sample_rate,
        "channels": channels,
        "duration_secs": samples.duration.as_secs_f64(),
    }))
}

fn speakers(input: &Path, models: &Path, merge_gap: f64, output: Option<&Path>) -> Result<Value> {
    let audio = loader::load_audio_file_and_convert(input, 1, 16_000)
        .map_err(|e| anyhow!("无法读取音频 {}: {e}", input.display()))?;

    let mut pipeline =
        speakrs::OwnedDiarizationPipeline::from_dir(models, speakrs::ExecutionMode::Cpu)
            .map_err(|e| anyhow!("加载说话人模型失败: {e}"))?;

    let result = pipeline
        .run(&audio.samples)
        .map_err(|e| anyhow!("说话人分离失败: {e}"))?;

    let segments = if merge_gap > 0.0 {
        speakrs::segment::merge_segments(&result.segments, merge_gap)
    } else {
        result.segments.clone()
    };

    let mut speakers: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    for segment in &segments {
        speakers
            .entry(segment.speaker.clone())
            .or_default()
            .push(json!({
                "start_secs": segment.start,
                "end_secs": segment.end,
                "duration_secs": segment.end - segment.start,
            }));
    }

    let items: Vec<Value> = speakers
        .iter()
        .map(|(speaker, segments)| {
            json!({
                "speaker": speaker,
                "segments": segments,
                "total_secs": segments
                    .iter()
                    .map(|s| s["duration_secs"].as_f64().unwrap_or(0.0))
                    .sum::<f64>(),
            })
        })
        .collect();

    let value = json!({
        "path": input.display().to_string(),
        "speakers": items,
        "segments": segments.len(),
    });

    if let Some(path) = output {
        std::fs::write(path, serde_json::to_string_pretty(&value)?)?;
    }

    Ok(value)
}

#[allow(clippy::too_many_arguments)]
fn denoise(input: &Path, output: &Path, model: &Path, threads: Option<usize>) -> Result<Value> {
    const DENOISE_RATE: usize = deepfilter_rs::SAMPLE_RATE;

    let config = loader::load_audio_file_and_convert(input, 1, DENOISE_RATE as u32)
        .map_err(|e| anyhow!("无法读取音频 {}: {e}", input.display()))?;

    let mut stream = match threads {
        Some(threads) => deepfilter_rs::DeepFilterStream::with_threads(model, threads.max(1)),
        None => deepfilter_rs::DeepFilterStream::new(model),
    }
    .map_err(|e| anyhow!("加载降噪模型失败: {e}"))?;

    let mut samples = Vec::with_capacity(config.samples.len());
    for chunk in config.samples.chunks(4800) {
        samples.extend(
            stream
                .process(chunk)
                .map_err(|e| anyhow!("降噪失败: {e}"))?,
        );
    }
    samples.extend(stream.flush().map_err(|e| anyhow!("降噪失败: {e}"))?);

    util::write_wav(output, &samples, DENOISE_RATE as u32, 1)?;

    Ok(json!({
        "path": output.display().to_string(),
        "sample_rate": DENOISE_RATE,
        "channels": 1,
        "duration_secs": samples.len() as f64 / DENOISE_RATE as f64,
        "rms_db": audio_level::calc_rms_level(&samples),
    }))
}

fn devices() -> Result<Value> {
    let recorder = AudioRecorder::new();
    let devices = recorder
        .get_input_devices()
        .map_err(|e| anyhow!("无法枚举录音设备: {e}"))?;

    let devices: Vec<Value> = devices
        .iter()
        .map(|d| {
            json!({
                "name": d.name,
                "channels": d.channels,
                "sample_rate": d.sample_rate,
            })
        })
        .collect();

    Ok(json!({ "devices": devices }))
}

fn record(
    output: &Path,
    seconds: f64,
    device: Option<&str>,
    gain_db: f32,
    mono: bool,
) -> Result<Value> {
    let mut recorder = AudioRecorder::new().with_gain(gain_db).with_mono(mono);
    recorder
        .start_recording(device)
        .with_context(|| format!("开始录音失败: {device:?}"))?;

    std::thread::sleep(Duration::from_secs_f64(seconds.max(0.0)));

    let audio = recorder
        .stop_recording()
        .map_err(|e| anyhow!("录音失败: {e}"))?;
    audio
        .save_to_file(output)
        .map_err(|e| anyhow!("写入 WAV 失败: {e}"))?;

    Ok(json!({
        "path": output.display().to_string(),
        "sample_rate": audio.sample_rate,
        "channels": audio.channels,
        "duration_secs": audio.duration_secs(),
        "rms_db": audio.rms_level_db(),
    }))
}
