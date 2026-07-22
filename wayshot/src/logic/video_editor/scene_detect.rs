use super::{
    command::{sync_and_refresh, with_history_manager},
    track::is_track_locked,
};
use crate::{
    db::{SceneDetectConfigData, VIDEO_EDITOR_TABLE},
    global_store,
    logic::{tr::tr, video_editor::project::SCENE_DETECT_CONFIG_ID},
    logic_cb,
    slint_generatedAppWindow::{
        AppWindow, SceneDetectConfig as UISceneDetectConfig,
        SceneDetectorAlgorithm as UISceneDetectorAlgorithm,
    },
};
use scenesdetect::{
    adaptive::{Detector as AdaptiveDetector, Options as AdaptiveOptions},
    content::{Detector as ContentDetector, Options as ContentOptions},
    frame::{LumaFrame, RgbFrame, Timebase, Timestamp},
    histogram::{Detector as HistogramDetector, Options as HistogramOptions},
    threshold::{Detector as ThresholdDetector, Options as ThresholdOptions},
};
use slint::{ComponentHandle, Weak};
use std::{
    num::NonZeroU32,
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    },
    time::Duration,
};
use video_editor::{
    Error,
    commands::{AffectedSegment, batch::BatchCommand, segment::SplitSegmentCommand},
    tracks::{segment::Segment, video_frame_cache::VideoImage},
};

static PROCESS_INC_INDEX: AtomicU32 = AtomicU32::new(0);

impl UISceneDetectConfig {
    fn threshold(&self) -> f32 {
        match self.algorithm {
            UISceneDetectorAlgorithm::Content => self.content_threshold,
            UISceneDetectorAlgorithm::Adaptive => self.adaptive_threshold,
            UISceneDetectorAlgorithm::Histogram => self.histogram_threshold,
            UISceneDetectorAlgorithm::Threshold => self.threshold_threshold,
        }
    }
}

pub fn init(ui: &AppWindow) {
    inner_init(ui);

    logic_cb!(
        video_editor_scene_detect_apply,
        ui,
        track_index,
        segment_index,
        config
    );
    logic_cb!(video_editor_scene_detect_cancel, ui);
}

fn inner_init(ui: &AppWindow) {
    let ui_weak = ui.as_weak();

    tokio::spawn(async move {
        let config = match sqldb::entry::select(VIDEO_EDITOR_TABLE, SCENE_DETECT_CONFIG_ID).await {
            Ok(entry) => serde_json::from_str::<SceneDetectConfigData>(&entry.data)
                .unwrap_or_else(|_| SceneDetectConfigData::default()),
            Err(_) => SceneDetectConfigData::default(),
        };

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            global_store!(ui).set_video_editor_scene_detect_config(config.into());
        });
    });
}

fn video_editor_scene_detect_apply(
    ui: &AppWindow,
    track_index: i32,
    segment_index: i32,
    config: UISceneDetectConfig,
) {
    let track_idx = track_index as usize;
    let seg_idx = segment_index as usize;

    if is_track_locked(ui, track_index) {
        crate::toast_warn!(ui, tr("Cannot segment in a locked track"));
        return;
    }

    let segment = with_history_manager(|state| {
        state.tracks_manager.get(track_idx).and_then(|track| {
            if seg_idx < track.segments().len() {
                Some(track.segments()[seg_idx].clone())
            } else {
                None
            }
        })
    });

    let Some(segment) = segment else {
        crate::toast_warn!(ui, tr("Segment not found"));
        return;
    };

    if segment.metadata.videos.is_empty() {
        crate::toast_warn!(ui, tr("Segment has no video"));
        return;
    }

    let ui_weak = ui.as_weak();
    let kind = config.algorithm;
    let threshold = config.threshold();
    let min_duration = Duration::from_secs(config.min_duration.max(1) as u64);
    let inc_index = PROCESS_INC_INDEX.fetch_add(1, Ordering::Relaxed) + 1;

    let config_data = SceneDetectConfigData::from(config);
    tokio::spawn(async move {
        save_config_data(&config_data).await;
    });

    global_store!(ui).set_video_editor_scene_detect_is_progressing(true);
    global_store!(ui).set_video_editor_scene_detect_progress(0.0);

    tokio::task::spawn_blocking(move || {
        let split_points =
            detect_scene_split_points(&ui_weak, &segment, kind, threshold, min_duration, inc_index);

        if inc_index != PROCESS_INC_INDEX.load(Ordering::Relaxed) {
            return;
        }

        let Ok(split_points) = split_points else {
            _ = ui_weak.upgrade_in_event_loop(move |ui| {
                if inc_index != PROCESS_INC_INDEX.load(Ordering::Relaxed) {
                    return;
                }
                global_store!(ui).set_video_editor_scene_detect_is_progressing(false);
                crate::toast_warn!(&ui, tr("Scene detection failed"));
            });
            return;
        };

        if split_points.is_empty() {
            _ = ui_weak.upgrade_in_event_loop(move |ui| {
                if inc_index != PROCESS_INC_INDEX.load(Ordering::Relaxed) {
                    return;
                }
                global_store!(ui).set_video_editor_scene_detect_is_progressing(false);
                global_store!(ui).set_video_editor_is_show_scene_detect_dialog(false);
                crate::toast_info!(&ui, tr("No scene changes detected"));
            });
            return;
        }

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            if inc_index != PROCESS_INC_INDEX.load(Ordering::Relaxed) {
                return;
            }
            apply_split_points(&ui, track_idx, seg_idx, &split_points, min_duration);
            global_store!(ui).set_video_editor_scene_detect_is_progressing(false);
            global_store!(ui).set_video_editor_is_show_scene_detect_dialog(false);
        });
    });
}

fn apply_split_points(
    ui: &AppWindow,
    track_idx: usize,
    seg_idx: usize,
    split_points: &[i32],
    min_duration: Duration,
) {
    let mut removed_indexs = vec![];
    let mut sorted_points = split_points.to_vec();
    sorted_points.sort();

    if let Some(mut splited_point) = sorted_points.get(0).cloned() {
        for index in 1..sorted_points.len() {
            let diff_ms = sorted_points[index].saturating_sub(splited_point);
            if Duration::from_millis(diff_ms as u64) < min_duration {
                removed_indexs.push(index);
            } else {
                splited_point = sorted_points[index];
            }
        }

        removed_indexs.into_iter().rev().for_each(|index| {
            sorted_points.remove(index);
        });
    };

    let result = with_history_manager(|state| {
        if track_idx >= state.tracks_manager.len() {
            return Err(Error::IndexOutOfBounds(
                track_idx,
                state.tracks_manager.len(),
            ));
        }

        let mut batch_command = BatchCommand::new("Intelligent scene segmentation".to_string());
        for &split_ms in sorted_points.iter().rev() {
            let split_duration = Duration::from_millis(split_ms as u64);
            if split_duration > Duration::ZERO {
                batch_command.add_command(Box::new(SplitSegmentCommand::new(
                    track_idx,
                    seg_idx,
                    split_duration,
                )));
            }
        }
        for i in 0..=sorted_points.len() {
            batch_command.add_extra_affected_segment(AffectedSegment::with_both_thumbnails(
                track_idx,
                seg_idx + i,
            ));
        }
        state
            .history_manager
            .execute(&mut state.tracks_manager, Box::new(batch_command))
    });

    match result {
        Ok(execute_result) => {
            sync_and_refresh(ui, execute_result.affected_segments, Some(false));
            crate::toast_success!(ui, tr("Scene segmentation completed"));
        }
        Err(e) => crate::toast_warn!(ui, format!("{}: {}", tr("Failed to segment"), e)),
    }
}

fn video_editor_scene_detect_cancel(ui: &AppWindow) {
    PROCESS_INC_INDEX.fetch_add(1, Ordering::Relaxed);
    global_store!(ui).set_video_editor_scene_detect_is_progressing(false);
}

fn detect_scene_split_points(
    ui_weak: &Weak<AppWindow>,
    segment: &Arc<Segment>,
    kind: UISceneDetectorAlgorithm,
    threshold: f32,
    min_duration: Duration,
    inc_index: u32,
) -> Result<Vec<i32>, String> {
    let video_meta = segment.metadata.first_video().ok_or("No video metadata")?;
    let fps = video_meta.fps as f64;
    let duration_secs = segment.duration.as_secs_f64();
    let total_frames = (fps * duration_secs).ceil() as usize;
    if total_frames == 0 {
        return Ok(vec![]);
    }

    let tb = Timebase::new(fps.round() as u32, NonZeroU32::new(1).unwrap());
    let frames_per_chunk = fps.ceil() as usize;
    let total_chunks = duration_secs.ceil() as usize;
    let start_frame = (segment.source_offset.as_secs_f64() * fps) as usize;

    let mut detected_frame_numbers: Vec<u64> = Vec::new();

    match kind {
        UISceneDetectorAlgorithm::Content => {
            let opts = if threshold > 0.0 {
                ContentOptions::new().with_threshold(threshold as f64)
            } else {
                ContentOptions::new()
            }
            .with_min_duration(min_duration);
            let mut det = ContentDetector::new(opts);
            process_bgr_detector(
                ui_weak,
                segment,
                &mut det,
                start_frame,
                total_frames,
                frames_per_chunk,
                total_chunks,
                tb,
                &mut detected_frame_numbers,
                inc_index,
            )?;
        }
        UISceneDetectorAlgorithm::Adaptive => {
            let opts = if threshold > 0.0 {
                AdaptiveOptions::new().with_adaptive_threshold(threshold as f64)
            } else {
                AdaptiveOptions::new()
            }
            .with_min_duration(min_duration);

            let mut det = AdaptiveDetector::new(opts);
            process_bgr_detector(
                ui_weak,
                segment,
                &mut det,
                start_frame,
                total_frames,
                frames_per_chunk,
                total_chunks,
                tb,
                &mut detected_frame_numbers,
                inc_index,
            )?;
        }
        UISceneDetectorAlgorithm::Histogram => {
            let opts = if threshold > 0.0 {
                HistogramOptions::new().with_threshold(threshold as f64)
            } else {
                HistogramOptions::new()
            }
            .with_min_duration(min_duration);
            let mut det = HistogramDetector::new(opts);
            process_luma_detector(
                segment,
                &mut det,
                start_frame,
                total_frames,
                frames_per_chunk,
                total_chunks,
                tb,
                &mut detected_frame_numbers,
                inc_index,
                ui_weak,
            )?;
        }
        UISceneDetectorAlgorithm::Threshold => {
            let opts = if threshold > 0.0 {
                ThresholdOptions::new().with_threshold(threshold as u8)
            } else {
                ThresholdOptions::new()
            }
            .with_min_duration(min_duration);
            let mut det = ThresholdDetector::new(opts);
            process_rgb_detector(
                segment,
                &mut det,
                start_frame,
                total_frames,
                frames_per_chunk,
                total_chunks,
                tb,
                &mut detected_frame_numbers,
                inc_index,
                ui_weak,
            )?;
        }
    }

    let split_points_ms: Vec<i32> = detected_frame_numbers
        .into_iter()
        .map(|frame_num| {
            let ms = (frame_num as f64 * 1000.0 / fps).round() as i32;
            ms.clamp(100, segment.duration.as_millis() as i32 - 100)
        })
        .filter(|ms| *ms > 0 && *ms < segment.duration.as_millis() as i32)
        .collect();

    Ok(split_points_ms)
}

trait BgrDetector {
    fn process_bgr_frame(&mut self, frame: RgbFrame<'_>) -> Option<Timestamp>;
}

impl BgrDetector for ContentDetector {
    fn process_bgr_frame(&mut self, frame: RgbFrame<'_>) -> Option<Timestamp> {
        self.process_bgr(frame)
    }
}

impl BgrDetector for AdaptiveDetector {
    fn process_bgr_frame(&mut self, frame: RgbFrame<'_>) -> Option<Timestamp> {
        self.process_bgr(frame)
    }
}

trait LumaDetector {
    fn process_luma_frame(&mut self, frame: LumaFrame<'_>) -> Option<Timestamp>;
}

impl LumaDetector for HistogramDetector {
    fn process_luma_frame(&mut self, frame: LumaFrame<'_>) -> Option<Timestamp> {
        self.process(frame)
    }
}

trait RgbDetector {
    fn process_rgb_frame(&mut self, frame: RgbFrame<'_>) -> Option<Timestamp>;
}

impl RgbDetector for ThresholdDetector {
    fn process_rgb_frame(&mut self, frame: RgbFrame<'_>) -> Option<Timestamp> {
        self.process_rgb(frame)
    }
}

fn process_bgr_detector<D: BgrDetector>(
    ui_weak: &Weak<AppWindow>,
    segment: &Arc<Segment>,
    detector: &mut D,
    start_frame: usize,
    total_frames: usize,
    frames_per_chunk: usize,
    total_chunks: usize,
    tb: Timebase,
    detected: &mut Vec<u64>,
    inc_index: u32,
) -> Result<(), String> {
    let mut global_frame_idx: u64 = 0;

    for chunk_idx in 0..total_chunks {
        if inc_index != PROCESS_INC_INDEX.load(Ordering::Relaxed) {
            return Err("Cancelled".to_string());
        }

        let progress = chunk_idx as f32 / total_chunks as f32;
        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            if inc_index != PROCESS_INC_INDEX.load(Ordering::Relaxed) {
                return;
            }
            global_store!(ui).set_video_editor_scene_detect_progress(progress);
        });

        let chunk_start_frame = start_frame + chunk_idx * frames_per_chunk;
        let remaining = ((total_frames as f64 - chunk_idx as f64 * frames_per_chunk as f64).ceil()
            as usize)
            .max(1);
        let chunk_frame_count = frames_per_chunk.min(remaining);

        let chunk_frames = segment
            .extract_video(chunk_start_frame, chunk_frame_count)
            .map_err(|e| format!("Frame extraction failed: {e}"))?;

        for vi in chunk_frames {
            match vi {
                VideoImage::Image { buffer } => {
                    let rgba = buffer.as_raw();
                    let (w, h) = (buffer.width(), buffer.height());
                    let bgr = rgba_to_bgr(rgba);
                    let stride = w * RgbFrame::BYTES_PER_PIXEL;
                    let ts = Timestamp::new(global_frame_idx as i64, tb);
                    let frame = RgbFrame::new(&bgr, w, h, stride, ts);

                    if detector.process_bgr_frame(frame).is_some() {
                        detected.push(global_frame_idx);
                    }
                    global_frame_idx += 1;
                }
                VideoImage::Empty => {
                    global_frame_idx += 1;
                }
            }
        }
    }

    Ok(())
}

fn process_luma_detector<D: LumaDetector>(
    segment: &std::sync::Arc<Segment>,
    detector: &mut D,
    start_frame: usize,
    total_frames: usize,
    frames_per_chunk: usize,
    total_chunks: usize,
    tb: Timebase,
    detected: &mut Vec<u64>,
    inc_index: u32,
    ui_weak: &Weak<AppWindow>,
) -> Result<(), String> {
    let mut global_frame_idx: u64 = 0;

    for chunk_idx in 0..total_chunks {
        if inc_index != PROCESS_INC_INDEX.load(Ordering::Relaxed) {
            return Err("Cancelled".to_string());
        }

        let progress = chunk_idx as f32 / total_chunks as f32;
        let ui_w = ui_weak.clone();
        _ = ui_w.upgrade_in_event_loop(move |ui| {
            if inc_index != PROCESS_INC_INDEX.load(Ordering::Relaxed) {
                return;
            }
            global_store!(ui).set_video_editor_scene_detect_progress(progress);
        });

        let chunk_start_frame = start_frame + chunk_idx * frames_per_chunk;
        let remaining = ((total_frames as f64 - chunk_idx as f64 * frames_per_chunk as f64).ceil()
            as usize)
            .max(1);
        let chunk_frame_count = frames_per_chunk.min(remaining);

        let chunk_frames = segment
            .extract_video(chunk_start_frame, chunk_frame_count)
            .map_err(|e| format!("Frame extraction failed: {e}"))?;

        for vi in chunk_frames {
            match vi {
                VideoImage::Image { buffer } => {
                    let rgba = buffer.as_raw();
                    let (w, h) = (buffer.width(), buffer.height());
                    let luma = rgba_to_luma(rgba);
                    let ts = Timestamp::new(global_frame_idx as i64, tb);
                    let frame = LumaFrame::new(&luma, w, h, w, ts);

                    if detector.process_luma_frame(frame).is_some() {
                        detected.push(global_frame_idx);
                    }
                    global_frame_idx += 1;
                }
                VideoImage::Empty => {
                    global_frame_idx += 1;
                }
            }
        }
    }

    Ok(())
}

fn process_rgb_detector<D: RgbDetector>(
    segment: &std::sync::Arc<Segment>,
    detector: &mut D,
    start_frame: usize,
    total_frames: usize,
    frames_per_chunk: usize,
    total_chunks: usize,
    tb: Timebase,
    detected: &mut Vec<u64>,
    inc_index: u32,
    ui_weak: &Weak<AppWindow>,
) -> Result<(), String> {
    let mut global_frame_idx: u64 = 0;

    for chunk_idx in 0..total_chunks {
        if inc_index != PROCESS_INC_INDEX.load(Ordering::Relaxed) {
            return Err("Cancelled".to_string());
        }

        let progress = chunk_idx as f32 / total_chunks as f32;
        let ui_w = ui_weak.clone();
        _ = ui_w.upgrade_in_event_loop(move |ui| {
            if inc_index != PROCESS_INC_INDEX.load(Ordering::Relaxed) {
                return;
            }
            global_store!(ui).set_video_editor_scene_detect_progress(progress);
        });

        let chunk_start_frame = start_frame + chunk_idx * frames_per_chunk;
        let remaining = ((total_frames as f64 - chunk_idx as f64 * frames_per_chunk as f64).ceil()
            as usize)
            .max(1);
        let chunk_frame_count = frames_per_chunk.min(remaining);

        let chunk_frames = segment
            .extract_video(chunk_start_frame, chunk_frame_count)
            .map_err(|e| format!("Frame extraction failed: {e}"))?;

        for vi in chunk_frames {
            match vi {
                VideoImage::Image { buffer } => {
                    let rgba = buffer.as_raw();
                    let (w, h) = (buffer.width(), buffer.height());
                    let rgb = rgba_to_rgb(rgba);
                    let stride = w * RgbFrame::BYTES_PER_PIXEL;
                    let ts = Timestamp::new(global_frame_idx as i64, tb);
                    let frame = RgbFrame::new(&rgb, w, h, stride, ts);

                    if detector.process_rgb_frame(frame).is_some() {
                        detected.push(global_frame_idx);
                    }
                    global_frame_idx += 1;
                }
                VideoImage::Empty => {
                    global_frame_idx += 1;
                }
            }
        }
    }

    Ok(())
}

fn rgba_to_bgr(rgba: &[u8]) -> Vec<u8> {
    let mut bgr = Vec::with_capacity(rgba.len() / 4 * 3);
    for pixel in rgba.chunks_exact(4) {
        bgr.push(pixel[2]); // B
        bgr.push(pixel[1]); // G
        bgr.push(pixel[0]); // R
    }
    bgr
}

fn rgba_to_rgb(rgba: &[u8]) -> Vec<u8> {
    let mut rgb = Vec::with_capacity(rgba.len() / 4 * 3);
    for pixel in rgba.chunks_exact(4) {
        rgb.push(pixel[0]); // R
        rgb.push(pixel[1]); // G
        rgb.push(pixel[2]); // B
    }
    rgb
}

fn rgba_to_luma(rgba: &[u8]) -> Vec<u8> {
    let mut luma = Vec::with_capacity(rgba.len() / 4);
    for pixel in rgba.chunks_exact(4) {
        let y = (0.299 * pixel[0] as f32 + 0.587 * pixel[1] as f32 + 0.114 * pixel[2] as f32)
            .round() as u8;
        luma.push(y);
    }
    luma
}

async fn save_config_data(data: &SceneDetectConfigData) {
    let json = serde_json::to_string(data).expect("serialize scene detect config failed");
    if sqldb::entry::insert(VIDEO_EDITOR_TABLE, SCENE_DETECT_CONFIG_ID, &json)
        .await
        .is_err()
    {
        if let Err(e) =
            sqldb::entry::update(VIDEO_EDITOR_TABLE, SCENE_DETECT_CONFIG_ID, &json).await
        {
            log::warn!("Failed to save scene detect config: {:?}", e);
        }
    }
}
