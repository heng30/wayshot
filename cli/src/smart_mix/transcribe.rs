use std::path::Path;
use std::time::Duration;

use anyhow::{Result, anyhow};
use video_editor::tracks::Manager;

use super::AudioEntry;

pub(super) fn transcribe_track(
    manager: &Manager,
    audio_track: usize,
    weights: &Path,
    tokenizer: &Path,
) -> Result<Vec<AudioEntry>> {
    let track = manager
        .tracks
        .get(audio_track)
        .ok_or_else(|| anyhow!("轨道索引越界: {audio_track}"))?;
    let segment = track
        .segments()
        .first()
        .ok_or_else(|| anyhow!("音频轨道没有片段"))?;

    let (channels, rate, samples) = crate::video::decode_audio(&segment.metadata.path)?;
    let samples =
        audio_utils::audio::resample_audio_with_channel(&samples, rate, channels, 16_000, 1)
            .map_err(|e| anyhow!("重采样失败: {e}"))?;

    let vad = audio_utils::vad::VadConfig::default().with_sample_rate(16_000);
    let segments = audio_utils::vad::detect_speech_segments(&samples, &vad);
    if segments.is_empty() {
        return Err(anyhow!("音频里没有检测到语音"));
    }

    let model_config = fun_ast_nano::FunASRModelConfig::default()
        .with_model_weights(
            weights
                .to_str()
                .ok_or_else(|| anyhow!("ASR 权重路径不是有效 UTF-8"))?
                .to_string(),
        )
        .with_tokenizer_path(
            tokenizer
                .to_str()
                .ok_or_else(|| anyhow!("ASR tokenizer 路径不是有效 UTF-8"))?
                .to_string(),
        );
    let mut model = fun_ast_nano::FunAsrNanoGenerateModel::new(model_config, None, None)
        .map_err(|e| anyhow!("加载 ASR 模型失败: {e}"))?;

    let mut entries = Vec::new();
    for segment in &segments {
        let audio = audio_utils::loader::AudioConfig::default()
            .with_sample_rate(16_000)
            .with_channel(1)
            .with_duration(Duration::from_secs_f64(
                segment.audio_data.len() as f64 / 16_000.0,
            ))
            .with_samples(segment.audio_data.clone());
        let request = fun_ast_nano::TranscriptionRequest::default()
            .with_audio_config(audio)
            .with_max_tokens(256);
        let text = model
            .generate(request, None, |_| Ok(()))
            .map(|response| fast2s::convert(&response.text))
            .unwrap_or_default();
        entries.push(AudioEntry {
            start_ms: (segment.start_sample as f64 / 16_000.0 * 1000.0) as u64,
            end_ms: (segment.end_sample as f64 / 16_000.0 * 1000.0) as u64,
            text,
        });
    }

    Ok(entries)
}
