use super::{
    command::with_history_manager,
    export::{
        add_export_task, next_export_task_id, picker_save_file, register_cancellation_token,
        remove_cancellation_token, update_export_task_progress,
    },
};
use crate::{
    global_store,
    logic::tr::tr,
    logic_cb,
    slint_generatedAppWindow::{AppWindow, CropDetail as UICropDetail, MediaType as UIMediaType},
};
use image::imageops;
use screenshot::{Algorithm, StitchConfig, Stitcher};
use slint::{ComponentHandle, SharedPixelBuffer};
use std::sync::atomic::{AtomicBool, Ordering};
use video_editor::{export::progress::CancellationToken, tracks::video_frame_cache::VideoImage};

static CANCEL_FLAG: AtomicBool = AtomicBool::new(false);

pub fn init(ui: &AppWindow) {
    logic_cb!(
        video_editor_long_screenshot_open,
        ui,
        track_index,
        segment_index
    );
    logic_cb!(
        video_editor_long_screenshot_apply,
        ui,
        track_index,
        segment_index,
        crop,
        algorithm
    );
    logic_cb!(video_editor_long_screenshot_cancel, ui);
    logic_cb!(video_editor_long_screenshot_load_image, ui);
}

fn video_editor_long_screenshot_open(ui: &AppWindow, track_index: i32, segment_index: i32) {
    let track_idx = track_index as usize;
    let seg_idx = segment_index as usize;

    global_store!(ui).set_video_editor_long_screenshot_progress(0.0);
    global_store!(ui).set_video_editor_long_screenshot_is_progressing(false);
    global_store!(ui).set_video_editor_long_screenshot_track_index(track_index);
    global_store!(ui).set_video_editor_long_screenshot_segment_index(segment_index);

    let segment_info = with_history_manager(|state| {
        state.tracks_manager.get(track_idx).and_then(|track| {
            if seg_idx < track.segments().len() {
                let seg = track.segments()[seg_idx].clone();
                let video_meta = seg.metadata.first_video().cloned();
                Some((seg, video_meta))
            } else {
                None
            }
        })
    });

    let Some((segment, Some(video_meta))) = segment_info else {
        crate::toast_warn!(ui, tr("Segment has no video"));
        return;
    };

    let source_fps = video_meta.fps;
    let start_frame = (segment.source_offset.as_secs_f64() * source_fps as f64) as usize;

    let frames = match segment.extract_video(start_frame, 1) {
        Ok(frames) => frames,
        Err(e) => {
            crate::toast_warn!(
                ui,
                format!("{}: {}", tr("Failed to extract first frame"), e)
            );
            return;
        }
    };

    let Some(VideoImage::Image { buffer }) = frames.into_iter().next() else {
        crate::toast_warn!(ui, tr("No frames extracted"));
        return;
    };

    let (width, height) = buffer.dimensions();
    let raw_bytes = buffer.as_raw().clone();
    let slint_buffer =
        SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(&raw_bytes, width, height);
    let slint_image = slint::Image::from_rgba8(slint_buffer);

    global_store!(ui).set_video_editor_long_screenshot_first_frame(slint_image);
    global_store!(ui).set_video_editor_is_show_long_screenshot_dialog(true);
}

fn video_editor_long_screenshot_apply(
    ui: &AppWindow,
    track_index: i32,
    segment_index: i32,
    crop: UICropDetail,
    algorithm: i32,
) {
    let ui_weak = ui.as_weak();
    let track_idx = track_index as usize;
    let seg_idx = segment_index as usize;

    let Some(output_path) = picker_save_file(
        ui_weak.clone(),
        &tr("Export Long Screenshot"),
        &tr("PNG Image"),
        &["png"],
        "long-screenshot.png",
    ) else {
        return;
    };

    global_store!(ui).set_video_editor_long_screenshot_is_progressing(true);
    global_store!(ui).set_video_editor_long_screenshot_progress(0.0);
    CANCEL_FLAG.store(false, Ordering::SeqCst);

    let segment_info = with_history_manager(|state| {
        state.tracks_manager.get(track_idx).and_then(|track| {
            if seg_idx < track.segments().len() {
                let seg = track.segments()[seg_idx].clone();
                let video_meta = seg.metadata.first_video().cloned();
                let source_stem = seg
                    .metadata
                    .path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("segment")
                    .to_string();
                Some((seg, video_meta, source_stem))
            } else {
                None
            }
        })
    });

    let Some((segment, Some(video_meta), source_stem)) = segment_info else {
        crate::toast_warn!(ui, tr("Segment has no video"));
        global_store!(ui).set_video_editor_long_screenshot_is_progressing(false);
        return;
    };

    let crop_left_f = crop.left;
    let crop_top_f = crop.top;
    let crop_width_f = crop.width;
    let crop_height_f = crop.height;

    let source_fps = video_meta.fps as f64;
    let duration_secs = segment.duration.as_secs_f64();
    let start_frame = (segment.source_offset.as_secs_f64() * source_fps as f64) as usize;
    let frames_per_chunk = source_fps.ceil() as usize;
    let total_chunks = duration_secs.ceil() as usize;

    tokio::spawn(async move {
        let task_id = next_export_task_id();
        let task_name = format!("{}-{}", source_stem, seg_idx);
        let cancellation_token = CancellationToken::new();
        add_export_task(&ui_weak, task_id, task_name, UIMediaType::Image).await;
        register_cancellation_token(task_id, cancellation_token.clone());

        let algo = match algorithm {
            0 => Algorithm::Template,
            1 => Algorithm::ColSample,
            _ => Algorithm::Template,
        };

        let mut stitcher = Stitcher::new(StitchConfig {
            algorithm: algo,
            min_overlap: 100,
            accept_diff: 3.5,
            min_append: 10,
            ..StitchConfig::default()
        });

        let mut _processed_count = 0usize;

        for chunk_idx in 0..total_chunks {
            if CANCEL_FLAG.load(Ordering::SeqCst) || cancellation_token.is_cancelled() {
                remove_cancellation_token(task_id);
                _ = ui_weak.upgrade_in_event_loop(move |ui| {
                    global_store!(ui).set_video_editor_long_screenshot_is_progressing(false);
                    crate::toast_warn!(ui, tr("Long screenshot cancelled"));
                });
                return;
            }

            let progress = chunk_idx as f32 / total_chunks as f32;
            update_export_task_progress(&ui_weak, task_id, progress);
            _ = ui_weak.upgrade_in_event_loop(move |ui| {
                global_store!(ui).set_video_editor_long_screenshot_progress(progress);
            });

            let chunk_start_frame = start_frame + chunk_idx * frames_per_chunk;
            let remaining_frames =
                ((duration_secs - chunk_idx as f64) * source_fps).ceil() as usize;
            let chunk_frame_count = frames_per_chunk.min(remaining_frames);

            let chunk_frames = match segment.extract_video(chunk_start_frame, chunk_frame_count) {
                Ok(frames) => frames,
                Err(e) => {
                    remove_cancellation_token(task_id);
                    _ = ui_weak.upgrade_in_event_loop(move |ui| {
                        global_store!(ui).set_video_editor_long_screenshot_is_progressing(false);
                        crate::toast_warn!(
                            ui,
                            format!(
                                "{}. {}: {}",
                                tr("Failed to extract frames"),
                                tr("Reason"),
                                e
                            )
                        );
                    });
                    return;
                }
            };

            for vi in chunk_frames {
                match vi {
                    VideoImage::Image { buffer } => {
                        let (img_w, img_h) = buffer.dimensions();
                        let crop_x = (crop_left_f * img_w as f32) as u32;
                        let crop_y = (crop_top_f * img_h as f32) as u32;
                        let crop_w = (crop_width_f * img_w as f32) as u32;
                        let crop_h = (crop_height_f * img_h as f32) as u32;

                        let clamped_x = crop_x.min(img_w - 1);
                        let clamped_y = crop_y.min(img_h - 1);
                        let clamped_w = crop_w.min(img_w - clamped_x);
                        let clamped_h = crop_h.min(img_h - clamped_y);

                        if clamped_w < 10 || clamped_h < 10 {
                            continue;
                        }

                        let cropped =
                            imageops::crop_imm(&buffer, clamped_x, clamped_y, clamped_w, clamped_h)
                                .to_image();

                        stitcher.push_frame(cropped);
                        _processed_count += 1;
                    }
                    VideoImage::Empty => {}
                }
            }
        }

        let final_image = match stitcher.into_image() {
            Some(img) => img,
            None => {
                remove_cancellation_token(task_id);
                _ = ui_weak.upgrade_in_event_loop(move |ui| {
                    global_store!(ui).set_video_editor_long_screenshot_is_progressing(false);
                    crate::toast_warn!(ui, tr("No frames processed"));
                });
                return;
            }
        };

        if let Err(e) = final_image.save(&output_path) {
            remove_cancellation_token(task_id);
            _ = ui_weak.upgrade_in_event_loop(move |ui| {
                global_store!(ui).set_video_editor_long_screenshot_is_progressing(false);
                crate::toast_warn!(
                    ui,
                    format!("{}. {}: {}", tr("Failed to save image"), tr("Reason"), e)
                );
            });
            return;
        }

        remove_cancellation_token(task_id);
        update_export_task_progress(&ui_weak, task_id, 1.0);

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            global_store!(ui).set_video_editor_long_screenshot_progress(1.0);
            global_store!(ui).set_video_editor_long_screenshot_is_progressing(false);
            crate::toast_success!(ui, tr("Long screenshot exported successfully"));
        });
    });
}

fn video_editor_long_screenshot_cancel(ui: &AppWindow) {
    CANCEL_FLAG.store(true, Ordering::SeqCst);
    global_store!(ui).set_video_editor_long_screenshot_is_progressing(false);
}

fn video_editor_long_screenshot_load_image(ui: &AppWindow) {
    let track_index = global_store!(ui).get_video_editor_long_screenshot_track_index();
    let segment_index = global_store!(ui).get_video_editor_long_screenshot_segment_index();
    let track_idx = track_index as usize;
    let seg_idx = segment_index as usize;

    let segment_info = with_history_manager(|state| {
        state.tracks_manager.get(track_idx).and_then(|track| {
            if seg_idx < track.segments().len() {
                let seg = track.segments()[seg_idx].clone();
                let video_meta = seg.metadata.first_video().cloned();
                Some((seg, video_meta))
            } else {
                None
            }
        })
    });

    let Some((segment, Some(video_meta))) = segment_info else {
        crate::toast_warn!(ui, tr("Segment has no video"));
        return;
    };

    let source_fps = video_meta.fps;
    let start_frame = (segment.source_offset.as_secs_f64() * source_fps as f64) as usize;

    let frames = match segment.extract_video(start_frame, 1) {
        Ok(frames) => frames,
        Err(e) => {
            crate::toast_warn!(
                ui,
                format!("{}: {}", tr("Failed to extract first frame"), e)
            );
            return;
        }
    };

    let Some(VideoImage::Image { buffer }) = frames.into_iter().next() else {
        crate::toast_warn!(ui, tr("No frames extracted"));
        return;
    };

    let (width, height) = buffer.dimensions();
    let raw_bytes = buffer.as_raw().clone();
    let slint_buffer =
        SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(&raw_bytes, width, height);
    let slint_image = slint::Image::from_rgba8(slint_buffer);
    global_store!(ui).set_video_editor_long_screenshot_first_frame(slint_image);
}
