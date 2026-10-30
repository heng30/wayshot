use std::path::Path;

use anyhow::{Result, anyhow};
use serde_json::{Value, json};

pub(crate) fn clip(
    input: &Path,
    output: &Path,
    start: f64,
    duration: Option<f64>,
    keep_audio: bool,
) -> Result<Value> {
    use std::sync::Arc;
    use video_editor::{
        export::{SegmentExportConfig, SegmentExporter},
        metadata::get_metadata,
        tracks::segment::Segment,
    };

    let metadata = Arc::new(get_metadata(input).map_err(|e| anyhow!("读取媒体信息失败: {e}"))?);
    if metadata.first_video().is_none() {
        return Err(anyhow!("文件中没有视频流: {}", input.display()));
    }

    let source_duration = metadata.duration.as_secs_f64();
    let duration = duration.unwrap_or(source_duration - start);
    if duration <= 0.0 {
        return Err(anyhow!("片段时长必须大于 0"));
    }

    let mut segment = Segment::new(
        std::time::Duration::ZERO,
        std::time::Duration::from_secs_f64(duration),
        metadata,
        1.0,
    );
    segment.source_offset = std::time::Duration::from_secs_f64(start.max(0.0));
    segment.audio_muted = !keep_audio;

    let result = SegmentExporter::export(
        &Arc::new(segment),
        SegmentExportConfig::new(output.to_path_buf()),
    )
    .map_err(|e| anyhow!("截取失败: {e}"))?;

    Ok(json!({
        "path": result.output_path.display().to_string(),
        "frames": result.total_frames,
        "start_secs": start,
        "duration_secs": duration,
    }))
}
