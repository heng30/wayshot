use std::path::Path;

use anyhow::Result;
use ffmpeg_next as ffmpeg;
use serde_json::{Value, json};

use super::args::SceneAlgorithm;
use super::probe::{decode_video, open_video, source_duration, source_fps};

#[allow(clippy::too_many_arguments)]
pub(crate) fn scenes(
    input: &Path,
    algorithm: SceneAlgorithm,
    threshold: f64,
    min_duration: f64,
    start: f64,
    duration: Option<f64>,
) -> Result<Value> {
    let mut source = open_video(input)?;
    let fps = source_fps(&source);
    let end = duration
        .map(|d| start + d)
        .unwrap_or_else(|| source_duration(&source).max(start));

    let mut detector = SceneDetector::new(algorithm, threshold, min_duration);
    let mut changes: Vec<f64> = Vec::new();
    let mut frame_index: i64 = 0;

    decode_video(&mut source, |rgb_frame, _ts| {
        let index = frame_index;
        frame_index += 1;
        let seconds = index as f64 / fps;
        if seconds < start {
            return Ok(true);
        }
        if seconds > end {
            return Ok(false);
        }

        if detector.hit(rgb_frame, index, start, fps) && changes.len() < MAX_SCENE_CHANGES {
            changes.push(seconds);
        }
        Ok(true)
    })?;

    Ok(json!({
        "path": input.display().to_string(),
        "algorithm": format!("{algorithm:?}").to_lowercase(),
        "count": changes.len(),
        "scenes": changes,
    }))
}
const MAX_SCENE_CHANGES: usize = 10_000;
enum SceneDetector {
    Content(scenesdetect::content::Detector),
    Adaptive(scenesdetect::adaptive::Detector),
    Histogram(scenesdetect::histogram::Detector),
    Threshold(scenesdetect::threshold::Detector),
}
fn timestamp_of(frame_index: i64, start: f64, fps: f64) -> scenesdetect::frame::Timestamp {
    use std::num::NonZeroI32;
    let timebase =
        scenesdetect::frame::Timebase::new(fps.round() as i32, NonZeroI32::new(1).expect("非零"));
    scenesdetect::frame::Timestamp::new(frame_index - (start * fps).round() as i64, timebase)
}
fn rgb_to_luma(data: &[u8], width: u32, height: u32, stride: u32) -> Vec<u8> {
    let mut luma = vec![0u8; (width * height) as usize];
    for y in 0..height as usize {
        for x in 0..width as usize {
            let idx = y * stride as usize + x * 3;
            let (r, g, b) = (data[idx] as u32, data[idx + 1] as u32, data[idx + 2] as u32);
            luma[y * width as usize + x] = ((r * 299 + g * 587 + b * 114) / 1000) as u8;
        }
    }
    luma
}

impl SceneDetector {
    fn new(algorithm: SceneAlgorithm, threshold: f64, min_duration: f64) -> Self {
        use scenesdetect::{
            adaptive::Options as AdaptiveOptions, content::Options as ContentOptions,
            histogram::Options as HistogramOptions, threshold::Options as ThresholdOptions,
        };

        let min_duration = std::time::Duration::from_secs_f64(min_duration.max(0.0));

        match algorithm {
            SceneAlgorithm::Content => {
                let opts = if threshold > 0.0 {
                    ContentOptions::new().with_threshold(threshold)
                } else {
                    ContentOptions::new()
                }
                .with_min_duration(min_duration);
                Self::Content(scenesdetect::content::Detector::new(opts))
            }
            SceneAlgorithm::Adaptive => {
                let opts = if threshold > 0.0 {
                    AdaptiveOptions::new().with_adaptive_threshold(threshold)
                } else {
                    AdaptiveOptions::new()
                }
                .with_min_duration(min_duration);
                Self::Adaptive(scenesdetect::adaptive::Detector::new(opts))
            }
            SceneAlgorithm::Histogram => {
                let opts = if threshold > 0.0 {
                    HistogramOptions::new().with_threshold(threshold)
                } else {
                    HistogramOptions::new()
                }
                .with_min_duration(min_duration);
                Self::Histogram(scenesdetect::histogram::Detector::new(opts))
            }
            SceneAlgorithm::Threshold => {
                let opts = if threshold > 0.0 {
                    ThresholdOptions::new().with_threshold(threshold as u8)
                } else {
                    ThresholdOptions::new()
                }
                .with_min_duration(min_duration);
                Self::Threshold(scenesdetect::threshold::Detector::new(opts))
            }
        }
    }

    fn hit(
        &mut self,
        frame: &ffmpeg::frame::Video,
        frame_index: i64,
        start: f64,
        fps: f64,
    ) -> bool {
        use scenesdetect::frame::{LumaFrame, RgbFrame};

        let width = frame.width();
        let height = frame.height();
        let stride = frame.stride(0) as u32;
        let data = frame.data(0);
        let timestamp = timestamp_of(frame_index, start, fps);

        match self {
            Self::Content(detector) => detector
                .process_bgr(RgbFrame::new(data, width, height, stride, timestamp))
                .is_some(),
            Self::Adaptive(detector) => detector
                .process_bgr(RgbFrame::new(data, width, height, stride, timestamp))
                .is_some(),
            Self::Threshold(detector) => detector
                .process_rgb(RgbFrame::new(data, width, height, stride, timestamp))
                .is_some(),
            Self::Histogram(detector) => {
                let luma = rgb_to_luma(data, width, height, stride);
                detector
                    .process(LumaFrame::new(&luma, width, height, width, timestamp))
                    .is_some()
            }
        }
    }
}
