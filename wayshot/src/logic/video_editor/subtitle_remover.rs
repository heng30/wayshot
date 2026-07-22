use crate::{
    db::{SubtitleRemoverConfigData, VIDEO_EDITOR_TABLE},
    global_store,
    logic::{
        downloader::{downloader_cancel, downloader_start},
        recorder::picker_directory,
        share_screen::picker_file,
        toast::async_toast_warn,
        tr::tr,
        video_editor::{playlist::import_file_to_playlist, project::SUBTITLE_REMOVER_CONFIG_ID},
    },
    logic_cb, logic_cb_pure,
    slint_generatedAppWindow::{
        AppWindow, Downloader as UIDownloader, DownloaderState as UIDownloaderState,
        VideoEditorSubtitleRemoverConfig as UISubtitleRemoverConfig,
    },
};
use anyhow::{Context, Result, bail};
use downloader::DownloadState;
use image::RgbImage;
use slint::{
    ComponentHandle, Model as SlintModel, SharedPixelBuffer, SharedString, VecModel, Weak,
};
use std::{
    path::PathBuf,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use subtitle_remover::{
    InpaintArea, Inpainter, LAMA_DOWNLOAD_URL, LamaInpainter, Mask,
    mask::{BBox, create_mask},
};
use video_editor::metadata::get_metadata;

use super::video_helper::{
    SegmentEncoderState, extract_all_audio_samples, extract_frame_at_time,
    extract_frames_for_duration,
};

static CANCEL_FLAG: AtomicBool = AtomicBool::new(false);
static PROCESSING_FLAG: AtomicBool = AtomicBool::new(false);
static RECT_STATE: Mutex<Option<RectState>> = Mutex::new(None);
static VIDEO_STATE: Mutex<Option<VideoState>> = Mutex::new(None);

#[macro_export]
macro_rules! store_video_editor_subtitle_remover_models_downloader {
    ($ui:expr) => {
        crate::global_store!($ui)
            .get_video_editor_subtitle_remover_models_downloader()
            .as_any()
            .downcast_ref::<VecModel<UIDownloader>>()
            .expect("We know we set a VecModel<UIDownloader> earlier for video editor subtitle remover models")
    };
}

#[derive(Clone)]
struct VideoState {
    video_path: PathBuf,
    fps: f32,
    duration: Duration,
    width: u32,
    height: u32,
    video_stream_index: usize,
    audio_stream_index: Option<usize>,
    audio_sample_rate: Option<u32>,
    audio_channels: Option<u16>,
}

#[derive(Clone)]
struct RectState {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

pub fn init(ui: &AppWindow) {
    inner_init(ui);

    logic_cb!(video_editor_subtitle_remover_import_video, ui);
    logic_cb!(video_editor_subtitle_remover_process, ui);
    logic_cb!(video_editor_subtitle_remover_remove_all, ui);
    logic_cb!(video_editor_subtitle_remover_choose_export_dir, ui);
    logic_cb!(video_editor_subtitle_remover_choose_model_path, ui, index);
    logic_cb!(video_editor_subtitle_remover_update_config, ui, config);
    logic_cb!(
        video_editor_subtitle_remover_model_start_download,
        ui,
        index,
        url
    );
    logic_cb!(
        video_editor_subtitle_remover_model_cancel_download,
        ui,
        index,
        url
    );
    logic_cb_pure!(video_editor_subtitle_remover_setting_is_valid, ui);
    logic_cb!(video_editor_subtitle_remover_seek, ui, position);
    logic_cb!(video_editor_subtitle_remover_rect_drawn, ui, x, y, w, h);
    logic_cb!(
        video_editor_subtitle_remover_overlay_refresh,
        ui,
        start_x,
        start_y,
        current_x,
        current_y
    );
    logic_cb!(video_editor_subtitle_remover_cancel, ui);
}

fn inner_init(ui: &AppWindow) {
    let downloaders = vec![UIDownloader {
        url: LAMA_DOWNLOAD_URL.to_string().into(),
        filename: "lama_fp32.onnx".into(),
        state: UIDownloaderState::UnStart,
        progress: 0.0,
    }];
    store_video_editor_subtitle_remover_models_downloader!(ui).set_vec(downloaders);

    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        let config = load_config().await.unwrap_or_else(|| {
            let mut config = SubtitleRemoverConfigData::default();
            config.id = SUBTITLE_REMOVER_CONFIG_ID.to_string();
            config
        });

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            global_store!(ui).set_video_editor_subtitle_remover_config(config.into());
        });
    });
}

fn save_config(config: SubtitleRemoverConfigData) {
    tokio::spawn(async move {
        let data =
            serde_json::to_string(&config).expect("serialize subtitle remover config failed");
        if sqldb::entry::insert(VIDEO_EDITOR_TABLE, SUBTITLE_REMOVER_CONFIG_ID, &data)
            .await
            .is_err()
        {
            if let Err(e) =
                sqldb::entry::update(VIDEO_EDITOR_TABLE, SUBTITLE_REMOVER_CONFIG_ID, &data).await
            {
                log::warn!("Failed to save subtitle remover config: {:?}", e);
            }
        }
    });
}

async fn load_config() -> Option<SubtitleRemoverConfigData> {
    match sqldb::entry::select(VIDEO_EDITOR_TABLE, SUBTITLE_REMOVER_CONFIG_ID).await {
        Ok(entry) => serde_json::from_str(&entry.data).ok(),
        Err(_) => None,
    }
}

fn video_editor_subtitle_remover_import_video(ui: &AppWindow) {
    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        let Some(filepath) = picker_file(
            ui_weak.clone(),
            &tr("Select video"),
            &tr("Video Files"),
            &["mp4", "mkv", "avi", "mov", "webm", "flv", "wmv", "ts"],
        ) else {
            return;
        };

        let metadata = match get_metadata(&filepath) {
            Ok(m) => m,
            Err(e) => {
                async_toast_warn(
                    ui_weak.clone(),
                    format!("{}: {e}", tr("Failed to read video metadata")),
                );
                return;
            }
        };

        let video_meta = match metadata.first_video() {
            Some(v) => v,
            None => {
                async_toast_warn(ui_weak.clone(), tr("No video stream found"));
                return;
            }
        };

        let audio_meta = metadata.audios.first();
        let fps = video_meta.fps;
        let duration = metadata.duration;
        let width = video_meta.width;
        let height = video_meta.height;
        let video_stream_index = video_meta.index;
        let audio_stream_index = audio_meta.map(|a| a.index);
        let audio_sample_rate = audio_meta.map(|a| a.sample_rate as u32);
        let audio_channels = audio_meta.map(|a| a.channels as u16);

        *VIDEO_STATE.lock().unwrap() = Some(VideoState {
            video_path: filepath.clone(),
            fps,
            duration,
            width,
            height,
            video_stream_index,
            audio_stream_index,
            audio_sample_rate,
            audio_channels,
        });
        *RECT_STATE.lock().unwrap() = None;

        let first_frame = extract_frame_at_time(&filepath, video_stream_index, Duration::ZERO);

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            if let Some(frame) = first_frame {
                let slint_img = rgba_to_slint_image(&frame);
                global_store!(ui).set_video_editor_subtitle_remover_image(slint_img);
            }

            global_store!(ui).set_video_editor_subtitle_remover_slider_position(0.0);
            global_store!(ui).set_video_editor_subtitle_remover_current_time("00:00".into());
            global_store!(ui)
                .set_video_editor_subtitle_remover_total_time(format_duration(duration).into());
            global_store!(ui)
                .set_video_editor_subtitle_remover_rectangle_overlay(slint::Image::default());
            global_store!(ui).set_video_editor_subtitle_remover_rect_x(0);
            global_store!(ui).set_video_editor_subtitle_remover_rect_y(0);
            global_store!(ui).set_video_editor_subtitle_remover_rect_width(0);
            global_store!(ui).set_video_editor_subtitle_remover_rect_height(0);
        });
    });
}

fn video_editor_subtitle_remover_seek(ui: &AppWindow, position: f32) {
    let state_guard = VIDEO_STATE.lock().unwrap();
    let Some(state) = state_guard.as_ref() else {
        return;
    };

    let target_time = Duration::from_secs_f64(position as f64 * state.duration.as_secs_f64());
    let frame = extract_frame_at_time(&state.video_path, state.video_stream_index, target_time);

    if let Some(frame) = &frame {
        let slint_img = rgba_to_slint_image(frame);
        global_store!(ui).set_video_editor_subtitle_remover_image(slint_img);
    }

    global_store!(ui).set_video_editor_subtitle_remover_slider_position(position);
    global_store!(ui)
        .set_video_editor_subtitle_remover_current_time(format_duration(target_time).into());

    let rect_guard = RECT_STATE.lock().unwrap();
    if let Some(rect) = rect_guard.as_ref() {
        let overlay = create_rect_overlay(
            state.width,
            state.height,
            rect.x,
            rect.y,
            rect.width,
            rect.height,
        );
        let slint_overlay = rgba_to_slint_image(&overlay);
        global_store!(ui).set_video_editor_subtitle_remover_rectangle_overlay(slint_overlay);
    }
}

fn video_editor_subtitle_remover_rect_drawn(ui: &AppWindow, x: f32, y: f32, w: f32, h: f32) {
    let state_guard = VIDEO_STATE.lock().unwrap();
    let Some(state) = state_guard.as_ref() else {
        return;
    };

    let rx = x as i32;
    let ry = y as i32;
    let rw = w as i32;
    let rh = h as i32;

    if rw < 5 || rh < 5 {
        return;
    }

    *RECT_STATE.lock().unwrap() = Some(RectState {
        x: rx,
        y: ry,
        width: rw,
        height: rh,
    });

    global_store!(ui).set_video_editor_subtitle_remover_rect_x(rx);
    global_store!(ui).set_video_editor_subtitle_remover_rect_y(ry);
    global_store!(ui).set_video_editor_subtitle_remover_rect_width(rw);
    global_store!(ui).set_video_editor_subtitle_remover_rect_height(rh);

    let overlay = create_rect_overlay(state.width, state.height, rx, ry, rw, rh);
    global_store!(ui)
        .set_video_editor_subtitle_remover_rectangle_overlay(rgba_to_slint_image(&overlay));
}

fn video_editor_subtitle_remover_overlay_refresh(
    ui: &AppWindow,
    img_x1: f32,
    img_y1: f32,
    img_x2: f32,
    img_y2: f32,
) {
    let state_guard = VIDEO_STATE.lock().unwrap();
    let Some(state) = state_guard.as_ref() else {
        return;
    };

    let rx = img_x1.min(img_x2) as i32;
    let ry = img_y1.min(img_y2) as i32;
    let rw = (img_x2 - img_x1).abs() as i32;
    let rh = (img_y2 - img_y1).abs() as i32;

    if rw < 2 || rh < 2 {
        return;
    }

    let overlay = create_rect_overlay(state.width, state.height, rx, ry, rw, rh);
    global_store!(ui)
        .set_video_editor_subtitle_remover_rectangle_overlay(rgba_to_slint_image(&overlay));
}

fn video_editor_subtitle_remover_process(ui: &AppWindow) {
    if PROCESSING_FLAG.load(Ordering::SeqCst) {
        return;
    }

    let model_path = match get_and_check_model_setting(ui) {
        Ok(path) => path,
        Err(e) => {
            global_store!(ui).set_video_editor_is_show_subtitle_remover_setting_dialog(true);
            crate::toast_warn!(ui, format!("{e}"));
            return;
        }
    };

    let state_guard = VIDEO_STATE.lock().unwrap();
    let Some(state) = state_guard.clone() else {
        crate::toast_warn!(ui, tr("No video imported"));
        return;
    };
    drop(state_guard);

    let rect_guard = RECT_STATE.lock().unwrap();
    let Some(rect) = rect_guard.clone() else {
        crate::toast_warn!(ui, tr("No subtitle region drawn"));
        return;
    };
    drop(rect_guard);

    let config = global_store!(ui).get_video_editor_subtitle_remover_config();
    let export_dir = config.export_dir.to_string();
    if export_dir.is_empty() {
        crate::toast_warn!(ui, tr("Please set export directory"));
        global_store!(ui).set_video_editor_is_show_subtitle_remover_setting_dialog(true);
        return;
    }

    let segment_duration_secs = if config.is_segment_save && config.segment_duration > 0 {
        config.segment_duration as u64
    } else {
        0 // 0 means no segment saving, save entire video
    };

    CANCEL_FLAG.store(false, Ordering::SeqCst);
    PROCESSING_FLAG.store(true, Ordering::SeqCst);
    global_store!(ui).set_video_editor_subtitle_remover_is_processing(true);
    global_store!(ui).set_video_editor_subtitle_remover_progress(0.0);

    let ui_weak = ui.as_weak();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Failed to create tokio runtime");

        let result = rt.block_on(process_video(
            &state,
            &rect,
            &model_path,
            &export_dir,
            segment_duration_secs,
            &ui_weak,
        ));

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            PROCESSING_FLAG.store(false, Ordering::SeqCst);
            global_store!(ui).set_video_editor_subtitle_remover_is_processing(false);

            if let Err(e) = result {
                if !CANCEL_FLAG.load(Ordering::SeqCst) {
                    global_store!(ui).set_video_editor_subtitle_remover_progress(0.0);
                    crate::toast_warn!(ui, format!("{}: {e}", tr("Processing failed")));
                }
            } else {
                global_store!(ui).set_video_editor_subtitle_remover_progress(1.0);
                crate::toast_success!(ui, tr("Processing complete"));
            }
        });
    });
}

fn video_editor_subtitle_remover_remove_all(ui: &AppWindow) {
    CANCEL_FLAG.store(true, Ordering::SeqCst);
    PROCESSING_FLAG.store(false, Ordering::SeqCst);
    global_store!(ui).set_video_editor_subtitle_remover_is_processing(false);
    global_store!(ui).set_video_editor_subtitle_remover_image(slint::Image::default());
    global_store!(ui).set_video_editor_subtitle_remover_rectangle_overlay(slint::Image::default());
    global_store!(ui).set_video_editor_subtitle_remover_progress(0.0);
    global_store!(ui).set_video_editor_subtitle_remover_slider_position(0.0);
    global_store!(ui).set_video_editor_subtitle_remover_current_time("".into());
    global_store!(ui).set_video_editor_subtitle_remover_total_time("".into());
    global_store!(ui).set_video_editor_subtitle_remover_rect_x(0);
    global_store!(ui).set_video_editor_subtitle_remover_rect_y(0);
    global_store!(ui).set_video_editor_subtitle_remover_rect_width(0);
    global_store!(ui).set_video_editor_subtitle_remover_rect_height(0);
    *VIDEO_STATE.lock().unwrap() = None;
    *RECT_STATE.lock().unwrap() = None;
}

fn video_editor_subtitle_remover_choose_export_dir(ui: &AppWindow) {
    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        let Some(dirpath) = picker_directory(ui_weak.clone(), &tr("Choose export directory"))
        else {
            return;
        };

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            let mut config = global_store!(ui).get_video_editor_subtitle_remover_config();
            config.export_dir = dirpath.to_string_lossy().to_string().into();
            global_store!(ui).set_video_editor_subtitle_remover_config(config.clone());
            save_config(config.into());
        });
    });
}

fn video_editor_subtitle_remover_choose_model_path(ui: &AppWindow, _index: i32) {
    let ui_weak = ui.as_weak();
    let title = tr("Choose LaMa model");

    tokio::spawn(async move {
        let Some(filepath) = picker_file(ui_weak.clone(), &title, &tr("ONNX Model"), &["onnx"])
        else {
            return;
        };

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            let filepath_str = filepath.to_string_lossy().to_string().into();
            let mut config = global_store!(ui).get_video_editor_subtitle_remover_config();
            config.lama_path = filepath_str;
            global_store!(ui).set_video_editor_subtitle_remover_config(config.clone());
            save_config(config.into());
        });
    });
}

fn video_editor_subtitle_remover_model_start_download(
    ui: &AppWindow,
    index: i32,
    url: SharedString,
) {
    let index = index as usize;
    let filename = "lama_fp32.onnx".into();

    downloader_start(
        ui,
        url,
        filename,
        move |ui: &AppWindow, _downloaded: u64, _total: u64, progress: f32| {
            if let Some(mut item) =
                store_video_editor_subtitle_remover_models_downloader!(ui).row_data(index)
            {
                item.progress = progress;
                store_video_editor_subtitle_remover_models_downloader!(ui)
                    .set_row_data(index, item);
            }
        },
        move |ui: &AppWindow, filepath: PathBuf| {
            if let Some(mut item) =
                store_video_editor_subtitle_remover_models_downloader!(ui).row_data(index)
            {
                item.state = UIDownloaderState::Downloading;
                store_video_editor_subtitle_remover_models_downloader!(ui)
                    .set_row_data(index, item);
            }

            let filepath_str = filepath.to_string_lossy().to_string().into();
            let mut config = global_store!(ui).get_video_editor_subtitle_remover_config();
            config.lama_path = filepath_str;
            global_store!(ui).set_video_editor_subtitle_remover_config(config.clone());
            save_config(config.into());
        },
        move |ui: &AppWindow, result: downloader::Result<downloader::DownloadState>| {
            if let Some(mut item) =
                store_video_editor_subtitle_remover_models_downloader!(ui).row_data(index)
            {
                match result {
                    Ok(DownloadState::Cancelled) => item.state = UIDownloaderState::Cancelled,
                    Ok(DownloadState::Incompleted) => item.state = UIDownloaderState::Failed,
                    Ok(DownloadState::Finsished) => item.state = UIDownloaderState::Finished,
                    Err(_) => item.state = UIDownloaderState::Failed,
                }
                store_video_editor_subtitle_remover_models_downloader!(ui)
                    .set_row_data(index, item);
            }
        },
    );
}

fn video_editor_subtitle_remover_model_cancel_download(
    ui: &AppWindow,
    index: i32,
    url: SharedString,
) {
    let index = index as usize;

    downloader_cancel(ui, url, move |ui: &AppWindow| {
        if let Some(mut item) =
            store_video_editor_subtitle_remover_models_downloader!(ui).row_data(index)
        {
            item.state = UIDownloaderState::Cancelled;
            store_video_editor_subtitle_remover_models_downloader!(ui).set_row_data(index, item);
        }
    });
}

fn video_editor_subtitle_remover_setting_is_valid(ui: &AppWindow) -> bool {
    get_and_check_model_setting(ui).is_ok()
}

fn video_editor_subtitle_remover_cancel(ui: &AppWindow) {
    CANCEL_FLAG.store(true, Ordering::SeqCst);
    global_store!(ui).set_video_editor_subtitle_remover_is_processing(false);
    global_store!(ui).set_video_editor_subtitle_remover_progress(0.0);
}

fn video_editor_subtitle_remover_update_config(ui: &AppWindow, config: UISubtitleRemoverConfig) {
    global_store!(ui).set_video_editor_subtitle_remover_config(config.clone());
    save_config(config.into());
}

fn get_and_check_model_setting(ui: &AppWindow) -> Result<PathBuf> {
    let config = global_store!(ui).get_video_editor_subtitle_remover_config();
    let model_path = config.lama_path.to_string();

    if model_path.is_empty() {
        bail!(tr("Please select a model file").to_string());
    }

    let path = PathBuf::from(&model_path);
    if !path.exists() {
        bail!(tr("Model file not found").to_string());
    }

    Ok(path)
}

async fn process_video(
    state: &VideoState,
    rect: &RectState,
    model_path: &PathBuf,
    export_dir: &str,
    segment_duration_secs: u64,
    ui_weak: &Weak<AppWindow>,
) -> Result<()> {
    let mut inpainter = LamaInpainter::new(model_path.to_str().unwrap_or(""), 1)
        .context(tr("Failed to load model").to_string())?;

    let sr_config = subtitle_remover::Config::default();
    let bbox: BBox = (rect.x, rect.x + rect.width, rect.y, rect.y + rect.height);
    let mask: Mask = create_mask(
        state.height as usize,
        state.width as usize,
        &[bbox],
        sr_config.subtitle_area_deviation_pixel,
    );

    // Use the exact mask bounds as the inpaint area (like the image_inpaint_demo example),
    // aligned to multiples of 8 (required by LaMa model).
    // Do NOT use get_inpaint_area_by_mask which creates fixed-height strips
    // centered on mask island centers — that can shift the area position
    // and is designed for auto-detection, not user-drawn rectangles.
    let inpaint_areas =
        compute_inpaint_areas_from_mask(&mask, state.width as i32, state.height as i32);

    if inpaint_areas.is_empty() {
        bail!(tr("No inpaint area found for the drawn region").to_string());
    }

    let fps = state.fps;
    let total_duration = state.duration;
    let total_secs = total_duration.as_secs_f64();
    let frames_per_second = (fps.ceil() as u64).max(1);
    let output_width = state.width;
    let output_height = state.height;

    // Extract all audio samples upfront so we can send them alongside video frames
    let audio_data = state
        .audio_stream_index
        .and_then(|idx| extract_all_audio_samples(&state.video_path, idx));

    // Audio cursor: position in the interleaved f32 sample buffer.
    // We send audio samples corresponding to each video frame's duration (1/fps).
    // mp4m internally handles AAC framing (caching partial frames, sending 1024
    // samples/channel at a time), so we just send raw interleaved samples per frame.
    let mut audio_cursor: usize = 0;

    let filename_stem = state
        .video_path
        .file_stem()
        .and_then(|n| n.to_str())
        .unwrap_or("output");

    let mut segment_index: u64 = 0;
    let mut frames_in_segment: u64 = 0;
    let mut segment_start_time: f64 = 0.0;

    // Initialize first segment encoder
    let mut encoder_state = SegmentEncoderState::new(
        output_width,
        output_height,
        fps,
        state.audio_sample_rate,
        state.audio_channels,
        &PathBuf::from(export_dir).join(format!("{}_part{}.mp4", filename_stem, segment_index)),
    )?;

    let mut current_time: f64 = 0.0;
    let total_frames = (total_secs * frames_per_second as f64).ceil() as u64;
    let mut processed_frames_count: u64 = 0;

    while current_time < total_secs {
        if CANCEL_FLAG.load(Ordering::SeqCst) {
            // Finalize current segment before stopping
            if frames_in_segment > 0 {
                let path = encoder_state.output_path.clone();
                encoder_state.finalize()?;
                import_file_to_playlist(ui_weak.clone(), path, None).await;
            }
            return Ok(());
        }

        // Decode 1 second of frames
        let start_time = Duration::from_secs_f64(current_time);
        let frames = extract_frames_for_duration(
            &state.video_path,
            state.video_stream_index,
            start_time,
            frames_per_second,
            output_width,
            output_height,
        );

        if frames.is_empty() {
            break;
        }

        for rgb_frame in &frames {
            if CANCEL_FLAG.load(Ordering::SeqCst) {
                if frames_in_segment > 0 {
                    let path = encoder_state.output_path.clone();
                    encoder_state.finalize()?;
                    import_file_to_playlist(ui_weak.clone(), path, None).await;
                }
                return Ok(());
            }

            // Check if we need to start a new segment BEFORE encoding this frame.
            // This ensures the frame goes into the correct segment.
            if segment_duration_secs > 0 && frames_in_segment > 0 {
                let segment_elapsed = current_time - segment_start_time;
                if segment_elapsed as u64 >= segment_duration_secs {
                    let path = encoder_state.output_path.clone();
                    encoder_state.finalize()?;
                    import_file_to_playlist(ui_weak.clone(), path, None).await;

                    segment_index += 1;
                    segment_start_time = current_time;
                    frames_in_segment = 0;

                    encoder_state = SegmentEncoderState::new(
                        output_width,
                        output_height,
                        fps,
                        state.audio_sample_rate,
                        state.audio_channels,
                        &PathBuf::from(export_dir)
                            .join(format!("{}_part{}.mp4", filename_stem, segment_index)),
                    )?;
                }
            }

            // Inpaint: start from the original full-size frame and composite
            // each inpaint area result back into it.
            // inpaint() returns frames cropped to the InpaintArea size,
            // so we must composite only the mask=255 pixels back into the full-size frame.
            // We use masked compositing (like the image_inpaint_demo example) to only
            // replace pixels where the mask indicates inpainting is needed, with
            // feathered blending at mask boundaries to avoid visible seams.
            let mut composite_frame = rgb_frame.clone();
            for area in &inpaint_areas {
                let result = inpainter
                    .inpaint(&[composite_frame.clone()], &mask, area)
                    .context("Inpainting failed")?;
                if let Some(inpainted) = result.into_iter().next() {
                    masked_composite(&mut composite_frame, &inpainted, &mask, area);
                }
            }

            encoder_state.encode_frame(&composite_frame)?;

            // Send corresponding audio samples for this video frame.
            // Each frame covers 1/fps seconds of audio.
            // We send the raw interleaved samples and let mp4m handle AAC framing
            // (it caches partial frames internally and sends 1024 samples/channel).
            if let Some((channels, sample_rate, audio_samples)) = &audio_data {
                let channels = *channels as usize;
                // Number of interleaved samples for one frame duration (1/fps seconds)
                let samples_per_frame =
                    (*sample_rate as f64 / fps as f64 * channels as f64).round() as usize;

                let end = (audio_cursor + samples_per_frame).min(audio_samples.len());
                if audio_cursor < end {
                    let chunk: Vec<f32> = audio_samples[audio_cursor..end].to_vec();
                    encoder_state.send_audio_chunk(chunk)?;
                }
                audio_cursor = end;
            }

            frames_in_segment += 1;
            processed_frames_count += 1;

            let progress = if total_frames > 0 {
                (processed_frames_count as f32 / total_frames as f32).min(1.0)
            } else {
                0.0
            };
            let current_time_str = format_duration(Duration::from_secs_f64(current_time));
            _ = ui_weak.clone().upgrade_in_event_loop(move |ui| {
                global_store!(ui).set_video_editor_subtitle_remover_progress(progress);
                global_store!(ui)
                    .set_video_editor_subtitle_remover_current_time(current_time_str.into());
            });
        }

        current_time += 1.0;
    }

    // Finalize last segment
    if frames_in_segment > 0 {
        let path = encoder_state.output_path.clone();
        encoder_state.finalize()?;
        import_file_to_playlist(ui_weak.clone(), path, None).await;
    }

    Ok(())
}

fn rgba_to_slint_image(img: &image::RgbaImage) -> slint::Image {
    let buffer = SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
        img.as_raw(),
        img.width(),
        img.height(),
    );
    slint::Image::from_rgba8(buffer)
}

fn create_rect_overlay(
    width: u32,
    height: u32,
    rx: i32,
    ry: i32,
    rw: i32,
    rh: i32,
) -> image::RgbaImage {
    let mut overlay = image::RgbaImage::new(width, height);
    let x_start = rx.max(0) as u32;
    let y_start = ry.max(0) as u32;
    let x_end = ((rx + rw) as u32).min(width);
    let y_end = ((ry + rh) as u32).min(height);

    // Draw semi-transparent red fill
    for y in y_start..y_end {
        for x in x_start..x_end {
            overlay.put_pixel(x, y, image::Rgba([255, 0, 0, 60]));
        }
    }

    // Draw border
    let border_color = image::Rgba([255, 0, 0, 200]);
    for x in x_start..x_end {
        if y_start < height {
            overlay.put_pixel(x, y_start, border_color);
        }
        if y_end > 0 && y_end - 1 < height {
            overlay.put_pixel(x, y_end - 1, border_color);
        }
    }
    for y in y_start..y_end {
        if x_start < width {
            overlay.put_pixel(x_start, y, border_color);
        }
        if x_end > 0 && x_end - 1 < width {
            overlay.put_pixel(x_end - 1, y, border_color);
        }
    }

    overlay
}

fn format_duration(d: Duration) -> String {
    let total_secs = d.as_secs();
    let mins = total_secs / 60;
    let secs = total_secs % 60;
    format!("{:02}:{:02}", mins, secs)
}

/// Compute inpaint areas from the mask by finding the exact bounds of mask=255
/// pixels and aligning them to multiples of 8 (required by the LaMa model).
/// This preserves the user's drawn rectangle position precisely, unlike
/// get_inpaint_area_by_mask which creates fixed-height strips that can shift
/// the area position.
fn compute_inpaint_areas_from_mask(mask: &Mask, width: i32, height: i32) -> Vec<InpaintArea> {
    if mask.iter().all(|&v| v == 0) {
        return vec![];
    }

    let rows = mask.nrows();
    let cols = mask.ncols();

    let mut mask_ymin = rows as i32;
    let mut mask_ymax = 0i32;
    let mut mask_xmin = cols as i32;
    let mut mask_xmax = 0i32;

    for r in 0..rows {
        for c in 0..cols {
            if mask[[r, c]] == 255 {
                mask_ymin = mask_ymin.min(r as i32);
                mask_ymax = mask_ymax.max(r as i32 + 1);
                mask_xmin = mask_xmin.min(c as i32);
                mask_xmax = mask_xmax.max(c as i32 + 1);
            }
        }
    }

    // Align to 8-pixel multiples (LaMa model requirement)
    let align = 8;
    let area_ymin = (mask_ymin / align) * align;
    let area_ymax = ((mask_ymax + align - 1) / align) * align;
    let area_xmin = (mask_xmin / align) * align;
    let area_xmax = ((mask_xmax + align - 1) / align) * align;

    // Clamp to image bounds
    let area_ymin = area_ymin.max(0);
    let area_ymax = area_ymax.min(height);
    let area_xmin = area_xmin.max(0);
    let area_xmax = area_xmax.min(width);

    vec![(area_ymin, area_ymax, area_xmin, area_xmax)]
}

/// Composite inpainted region back into the full-size frame, only at mask=255
/// pixels. Pixels near the mask boundary are blended with a feathered transition
/// to avoid visible seams (same approach as the image_inpaint_demo example).
fn masked_composite(target: &mut RgbImage, inpainted: &RgbImage, mask: &Mask, area: &InpaintArea) {
    let (ymin, _ymax, xmin, _xmax) = *area;
    let iw = inpainted.width() as i32;
    let ih = inpainted.height() as i32;
    let feather_radius = 5; // pixels for feathering at mask boundary

    let mask_rows = mask.nrows();
    let mask_cols = mask.ncols();

    for y in 0..ih {
        let ty = y + ymin;
        if ty < 0 || ty >= target.height() as i32 {
            continue;
        }
        for x in 0..iw {
            let tx = x + xmin;
            if tx < 0 || tx >= target.width() as i32 {
                continue;
            }
            let my = ty as usize;
            let mx = tx as usize;
            if my >= mask_rows || mx >= mask_cols {
                continue;
            }
            if mask[[my, mx]] == 0 {
                continue;
            }

            let dist = min_mask_boundary_dist(mask, my, mx);

            if dist >= feather_radius {
                // Far from boundary: use inpainted pixel
                target.put_pixel(
                    tx as u32,
                    ty as u32,
                    *inpainted.get_pixel(x as u32, y as u32),
                );
            } else {
                // Near boundary: blend inpainted and original
                let alpha = dist as f32 / feather_radius as f32;
                let inp = inpainted.get_pixel(x as u32, y as u32);
                let orig = target.get_pixel(tx as u32, ty as u32);
                let blended = image::Rgb([
                    (orig[0] as f32 * (1.0 - alpha) + inp[0] as f32 * alpha) as u8,
                    (orig[1] as f32 * (1.0 - alpha) + inp[1] as f32 * alpha) as u8,
                    (orig[2] as f32 * (1.0 - alpha) + inp[2] as f32 * alpha) as u8,
                ]);
                target.put_pixel(tx as u32, ty as u32, blended);
            }
        }
    }
}

/// Find the minimum Chebyshev distance from (row, col) to a mask boundary.
/// A mask boundary is where mask value changes (255→0) or the array edge.
fn min_mask_boundary_dist(mask: &Mask, row: usize, col: usize) -> usize {
    let rows = mask.nrows();
    let cols = mask.ncols();
    let mut min_dist = usize::MAX;
    let search = 8;

    for dy in -search..=search {
        for dx in -search..=search {
            let nr = row as i32 + dy;
            let nc = col as i32 + dx;
            if nr < 0 || nr >= rows as i32 || nc < 0 || nc >= cols as i32 {
                let d = (dy.abs().max(dx.abs())) as usize;
                if d < min_dist {
                    min_dist = d;
                }
            } else if mask[[nr as usize, nc as usize]] != mask[[row, col]] {
                let d = (dy.abs().max(dx.abs())) as usize;
                if d < min_dist {
                    min_dist = d;
                }
            }
        }
    }

    if min_dist == usize::MAX {
        search as usize + 1
    } else {
        min_dist
    }
}
