use std::path::Path;

use anyhow::{Result, anyhow};
use serde_json::{Value, json};
use video_editor::{
    export::{Mp4ExportConfig, Mp4Exporter},
    project::load_project,
};

use super::Preset;

#[allow(clippy::too_many_arguments)]
pub(super) fn export(
    input: &Path,
    output: &Path,
    width: Option<u32>,
    height: Option<u32>,
    fps: Option<u32>,
    crf: Option<u8>,
    preset: Option<Preset>,
    audio_bitrate: Option<u32>,
    burn_subtitles: bool,
    low_memory: bool,
) -> Result<Value> {
    let project = load_project(input)?;
    let manager = project
        .inner
        .ok_or_else(|| anyhow!("工程文件里没有轨道数据: {}", input.display()))?;

    let mut config = Mp4ExportConfig::default();
    config.output_path = output.to_path_buf();
    config.width = width;
    config.height = height;
    config.fps = fps;
    config.crf = crf;
    config.compression_preset = preset.map(|p| p.to_compression());
    config.audio_bitrate = audio_bitrate.unwrap_or(config.audio_bitrate);
    config.burn_subtitles = burn_subtitles;
    config.low_memory_mode = low_memory;

    let result = Mp4Exporter::new(manager, config)
        .export()
        .map_err(|e| anyhow!("导出失败: {e}"))?;

    Ok(json!({
        "path": result.output_path.display().to_string(),
        "duration_secs": result.duration.as_secs_f64(),
        "frames": result.total_frames,
    }))
}

pub(super) fn run(cmd: &super::Cmd) -> Result<Value> {
    let super::Cmd::Export {
        input,
        output,
        width,
        height,
        fps,
        crf,
        preset,
        audio_bitrate,
        no_subtitles,
        low_memory,
    } = cmd
    else {
        unreachable!()
    };

    export(
        input,
        output,
        *width,
        *height,
        *fps,
        *crf,
        *preset,
        *audio_bitrate,
        !no_subtitles,
        *low_memory,
    )
}
