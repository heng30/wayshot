use crate::{
    db::{CutoutConfigData, VIDEO_EDITOR_TABLE},
    global_store,
    logic::{
        downloader::{downloader_cancel, downloader_start},
        recorder::picker_directory,
        share_screen::picker_file,
        toast::{self, async_toast_warn},
        tr::tr,
        video_editor::{
            img_animation::IMAGE_EXTENSIONS, playlist::import_file_to_playlist,
            project::CUTOUT_CONFIG_ID,
        },
    },
    logic_cb, logic_cb_pure,
    slint_generatedAppWindow::{
        AppWindow, Downloader as UIDownloader, DownloaderState as UIDownloaderState,
        VideoEditorCutoutConfig as UICutoutConfig,
    },
};
use anyhow::{Result, bail};
use cutout::{cutout::CutoutOptions, manager::ModelManager, model::Model};
use downloader::DownloadState;
use image::{GenericImageView, GrayImage};
use slint::{ComponentHandle, Model as SlintModel, SharedString, VecModel};
use std::{path::PathBuf, sync::Mutex};

#[macro_export]
macro_rules! store_video_editor_cutout_models_downloader {
    ($ui:expr) => {
        crate::global_store!($ui)
            .get_video_editor_cutout_models_downloader()
            .as_any()
            .downcast_ref::<VecModel<UIDownloader>>()
            .expect(
                "We know we set a VecModel<UIDownloader> earlier for video editor cutout models",
            )
    };
}

static IMPORTED_IMAGE_PATH: Mutex<Option<PathBuf>> = Mutex::new(None);
static BRUSH_MASK_STATE: Mutex<Option<BrushMaskState>> = Mutex::new(None);

struct BrushMaskState {
    mask: GrayImage,
    width: u32,
    height: u32,
    last_x: Option<f32>,
    last_y: Option<f32>,
    history: Vec<GrayImage>,
    redo_stack: Vec<GrayImage>,
}

pub fn init(ui: &AppWindow) {
    inner_init(ui);

    logic_cb!(video_editor_cutout_import_image, ui);
    logic_cb!(video_editor_cutout_process_image, ui);
    logic_cb!(video_editor_cutout_export_to_playlist, ui);
    logic_cb!(video_editor_cutout_remove_all_images, ui);
    logic_cb!(video_editor_cutout_choose_export_dir, ui);
    logic_cb!(video_editor_cutout_choose_model_path, ui, index);
    logic_cb!(video_editor_cutout_update_config, ui, config);
    logic_cb!(video_editor_cutout_model_start_download, ui, index, url);
    logic_cb!(video_editor_cutout_model_cancel_download, ui, index, url);
    logic_cb_pure!(video_editor_cutout_setting_is_valid, ui);
    logic_cb!(video_editor_cutout_brush_move, ui, x, y);
    logic_cb!(video_editor_cutout_brush_start, ui, x, y);
    logic_cb!(video_editor_cutout_brush_size_change, ui, delta);
    logic_cb!(video_editor_cutout_undo, ui);
    logic_cb!(video_editor_cutout_redo, ui);
    logic_cb!(video_editor_cutout_overlay_refresh, ui);
}

fn inner_init(ui: &AppWindow) {
    let downloaders = Model::all_models()
        .into_iter()
        .map(|m| UIDownloader {
            url: m.download_url().to_string().into(),
            filename: m.to_filename().to_string().into(),
            state: UIDownloaderState::UnStart,
            progress: 0.0,
        })
        .collect::<Vec<_>>();
    store_video_editor_cutout_models_downloader!(ui).set_vec(downloaders);

    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        let config = load_cutout_config()
            .await
            .unwrap_or_else(|| CutoutConfigData {
                id: CUTOUT_CONFIG_ID.to_string(),
                ..Default::default()
            });

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            global_store!(ui).set_video_editor_cutout_config(config.into());
        });
    });
}

fn save_cutout_config(config: CutoutConfigData) {
    tokio::spawn(async move {
        let data = serde_json::to_string(&config).expect("serialize cutout config failed");
        if sqldb::entry::insert(VIDEO_EDITOR_TABLE, CUTOUT_CONFIG_ID, &data)
            .await
            .is_err()
        {
            if let Err(e) = sqldb::entry::update(VIDEO_EDITOR_TABLE, CUTOUT_CONFIG_ID, &data).await
            {
                log::warn!("Failed to save cutout config: {:?}", e);
            }
        }
    });
}

async fn load_cutout_config() -> Option<CutoutConfigData> {
    match sqldb::entry::select(VIDEO_EDITOR_TABLE, CUTOUT_CONFIG_ID).await {
        Ok(entry) => serde_json::from_str(&entry.data).ok(),
        Err(_) => None,
    }
}

fn video_editor_cutout_import_image(ui: &AppWindow) {
    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        let Some(filepath) = picker_file(
            ui_weak.clone(),
            &tr("Select image"),
            &tr("Image Files"),
            &IMAGE_EXTENSIONS,
        ) else {
            return;
        };

        *IMPORTED_IMAGE_PATH.lock().unwrap() = Some(filepath.clone());

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            if let Ok(img) = image::open(&filepath) {
                let (w, h) = img.dimensions();
                let rgba_img = img.to_rgba8();
                let buffer = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
                    rgba_img.as_raw(),
                    w,
                    h,
                );
                global_store!(ui).set_video_editor_cutout_image(slint::Image::from_rgba8(buffer));
                global_store!(ui).set_video_editor_cutout_processed_image(slint::Image::default());
                global_store!(ui).set_video_editor_cutout_mask_overlay(slint::Image::default());
                global_store!(ui).set_video_editor_cutout_is_brush_mode(false);

                *BRUSH_MASK_STATE.lock().unwrap() = Some(BrushMaskState {
                    mask: image::GrayImage::new(w, h),
                    width: w,
                    height: h,
                    last_x: None,
                    last_y: None,
                    history: vec![],
                    redo_stack: vec![],
                });
            }
        });
    });
}

fn video_editor_cutout_process_image(ui: &AppWindow) {
    let (model_path, model) = match get_and_check_model_setting(ui) {
        Ok(result) => result,
        Err(e) => {
            global_store!(ui).set_video_editor_is_show_cutout_setting_dialog(true);
            crate::toast_warn!(ui, format!("{e}"));
            return;
        }
    };

    let Some(filepath) = IMPORTED_IMAGE_PATH.lock().unwrap().clone() else {
        crate::toast_warn!(ui, tr("No image imported"));
        return;
    };

    let mask = {
        let state = BRUSH_MASK_STATE.lock().unwrap();
        match state.as_ref() {
            Some(s) => {
                let has_mask = s.mask.pixels().any(|p| p.0[0] > 0);
                if has_mask { Some(s.mask.clone()) } else { None }
            }
            None => None,
        }
    };

    let config = global_store!(ui).get_video_editor_cutout_config();
    let threshold = config.threshold.clamp(0, 255) as u8;
    let binary = config.binary;
    let sticker = {
        let a = config.sticker_color_a;
        if a > 0 {
            Some(image::Rgba([
                config.sticker_color_r.clamp(0, 255) as u8,
                config.sticker_color_g.clamp(0, 255) as u8,
                config.sticker_color_b.clamp(0, 255) as u8,
                a.clamp(0, 255) as u8,
            ]))
        } else {
            None
        }
    };

    let ui_weak = ui.as_weak();
    global_store!(ui).set_video_editor_cutout_processed_image(slint::Image::default());
    global_store!(ui).set_video_editor_cutout_is_processing(true);

    tokio::spawn(async move {
        let mut manager = match ModelManager::from_file(&model_path) {
            Ok(m) => m,
            Err(e) => {
                async_toast_warn(ui_weak, format!("{}: {e}", tr("Failed to load model")));
                return;
            }
        };

        let options = CutoutOptions::default()
            .with_threshold(threshold)
            .with_binary(binary)
            .with_sticker(sticker)
            .with_mask(mask);

        let img = match image::open(&filepath) {
            Ok(img) => img,
            Err(e) => {
                async_toast_warn(ui_weak, format!("{}: {e}", tr("Failed to open image")));
                return;
            }
        };

        match cutout::cutout::cutout(&mut manager, model, img, &options) {
            Ok(result) => {
                let rgba_img = result.image;
                let buffer = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
                    rgba_img.as_raw(),
                    rgba_img.width(),
                    rgba_img.height(),
                );
                _ = ui_weak.upgrade_in_event_loop(move |ui| {
                    if !global_store!(ui).get_video_editor_cutout_is_processing() {
                        return;
                    }

                    global_store!(ui).set_video_editor_cutout_is_processing(false);
                    global_store!(ui)
                        .set_video_editor_cutout_processed_image(slint::Image::from_rgba8(buffer));
                });
            }
            Err(e) => {
                _ = ui_weak.upgrade_in_event_loop(move |ui| {
                    global_store!(ui).set_video_editor_cutout_is_processing(false);
                    crate::toast_warn!(ui, format!("{}: {e}", tr("Failed to process image")));
                });
            }
        }
    });
}

fn video_editor_cutout_export_to_playlist(ui: &AppWindow) {
    let processed_image = global_store!(ui).get_video_editor_cutout_processed_image();
    let Some(buffer) = processed_image.to_rgba8() else {
        crate::toast_warn!(ui, tr("No processed image to export"));
        return;
    };

    let Some(filepath) = IMPORTED_IMAGE_PATH.lock().unwrap().clone() else {
        crate::toast_warn!(ui, tr("No original image path stored"));
        return;
    };

    let filename = filepath
        .file_stem()
        .and_then(|n| n.to_str())
        .unwrap_or("output");
    let config = global_store!(ui).get_video_editor_cutout_config();
    let export_dir = config.export_dir.to_string();
    let output_path = PathBuf::from(export_dir).join(format!("{}_cutout.png", filename));

    let Some(rgba_img): Option<image::RgbaImage> =
        image::ImageBuffer::from_raw(buffer.width(), buffer.height(), buffer.as_bytes().to_vec())
    else {
        crate::toast_warn!(ui, tr("Failed to create RgbaImage from buffer"));
        return;
    };

    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        if let Err(e) = rgba_img.save(&output_path) {
            toast::async_toast_warn(
                ui_weak.clone(),
                format!("{}: {}", tr("Failed to save image"), e),
            );
            return;
        }

        import_file_to_playlist(ui_weak.clone(), output_path.clone(), None).await;
        toast::async_toast_success(
            ui_weak,
            format!("{} {}", tr("Exported to"), output_path.display()),
        );
    });
}

fn video_editor_cutout_remove_all_images(ui: &AppWindow) {
    global_store!(ui).set_video_editor_cutout_is_processing(false);
    global_store!(ui).set_video_editor_cutout_is_brush_mode(false);
    global_store!(ui).set_video_editor_cutout_image(slint::Image::default());
    global_store!(ui).set_video_editor_cutout_processed_image(slint::Image::default());
    global_store!(ui).set_video_editor_cutout_mask_overlay(slint::Image::default());
    *BRUSH_MASK_STATE.lock().unwrap() = None;
    *IMPORTED_IMAGE_PATH.lock().unwrap() = None;
}

fn video_editor_cutout_choose_export_dir(ui: &AppWindow) {
    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        let Some(dirpath) = picker_directory(
            ui_weak.clone(),
            &crate::logic::tr::tr("Choose export directory"),
        ) else {
            return;
        };

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            let mut config = global_store!(ui).get_video_editor_cutout_config();
            config.export_dir = dirpath.to_string_lossy().to_string().into();
            global_store!(ui).set_video_editor_cutout_config(config.clone());
            save_cutout_config(config.into());
        });
    });
}

fn video_editor_cutout_choose_model_path(ui: &AppWindow, index: i32) {
    let ui_weak = ui.as_weak();
    let title = tr("Choose cutout model");

    tokio::spawn(async move {
        let Some(filepath) = picker_file(ui_weak.clone(), &title, &tr("ONNX Model"), &["onnx"])
        else {
            return;
        };

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            let filepath_str = filepath.to_string_lossy().to_string().into();
            let mut config = global_store!(ui).get_video_editor_cutout_config();
            set_model_path(&mut config, index as i32, filepath_str);
            global_store!(ui).set_video_editor_cutout_config(config.clone());
            save_cutout_config(config.into());
        });
    });
}

fn video_editor_cutout_model_start_download(ui: &AppWindow, index: i32, url: SharedString) {
    let index = index as usize;
    let models = Model::all_models();
    if index >= models.len() {
        return;
    }
    let filename = models[index].to_filename().into();

    downloader_start(
        ui,
        url,
        filename,
        move |ui: &AppWindow, _downloaded: u64, _total: u64, progress: f32| {
            if let Some(mut item) = store_video_editor_cutout_models_downloader!(ui).row_data(index)
            {
                item.progress = progress;
                store_video_editor_cutout_models_downloader!(ui).set_row_data(index, item);
            }
        },
        move |ui: &AppWindow, filepath: PathBuf| {
            if let Some(mut item) = store_video_editor_cutout_models_downloader!(ui).row_data(index)
            {
                item.state = UIDownloaderState::Downloading;
                store_video_editor_cutout_models_downloader!(ui).set_row_data(index, item);
            }

            let filepath_str = filepath.to_string_lossy().to_string().into();
            let mut config = global_store!(ui).get_video_editor_cutout_config();
            set_model_path(&mut config, index as i32, filepath_str);
            global_store!(ui).set_video_editor_cutout_config(config.clone());
            save_cutout_config(config.into());
        },
        move |ui: &AppWindow, result: downloader::Result<downloader::DownloadState>| {
            if let Some(mut item) = store_video_editor_cutout_models_downloader!(ui).row_data(index)
            {
                match result {
                    Ok(DownloadState::Cancelled) => item.state = UIDownloaderState::Cancelled,
                    Ok(DownloadState::Incompleted) => item.state = UIDownloaderState::Failed,
                    Ok(DownloadState::Finsished) => item.state = UIDownloaderState::Finished,
                    Err(_) => item.state = UIDownloaderState::Failed,
                }
                store_video_editor_cutout_models_downloader!(ui).set_row_data(index, item);
            }
        },
    );
}

fn video_editor_cutout_model_cancel_download(ui: &AppWindow, index: i32, url: SharedString) {
    let index = index as usize;

    downloader_cancel(ui, url, move |ui: &AppWindow| {
        if let Some(mut item) = store_video_editor_cutout_models_downloader!(ui).row_data(index) {
            item.state = UIDownloaderState::Cancelled;
            store_video_editor_cutout_models_downloader!(ui).set_row_data(index, item);
        }
    });
}

fn video_editor_cutout_setting_is_valid(ui: &AppWindow) -> bool {
    get_and_check_model_setting(ui).is_ok()
}

fn video_editor_cutout_brush_start(ui: &AppWindow, x: f32, y: f32) {
    let brush_size = global_store!(ui).get_video_editor_cutout_brush_size();
    let radius = brush_size / 2.0;

    let mut state_guard = BRUSH_MASK_STATE.lock().unwrap();
    let Some(state) = state_guard.as_mut() else {
        return;
    };

    state.history.push(state.mask.clone());
    if state.history.len() > 50 {
        state.history.remove(0);
    }

    state.redo_stack.clear();
    state.last_x = None;
    state.last_y = None;

    paint_circle(&mut state.mask, state.width, state.height, x, y, radius);
    state.last_x = Some(x);
    state.last_y = Some(y);
}

fn video_editor_cutout_brush_move(ui: &AppWindow, x: f32, y: f32) {
    if x < 0.0 && y < 0.0 {
        let state_guard = BRUSH_MASK_STATE.lock().unwrap();
        let Some(state) = state_guard.as_ref() else {
            return;
        };
        let overlay = mask_to_overlay(&state.mask);
        let buffer = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
            overlay.as_raw(),
            overlay.width(),
            overlay.height(),
        );
        global_store!(ui).set_video_editor_cutout_mask_overlay(slint::Image::from_rgba8(buffer));
        return;
    }

    let brush_size = global_store!(ui).get_video_editor_cutout_brush_size();
    let radius = brush_size / 2.0;

    let mut state_guard = BRUSH_MASK_STATE.lock().unwrap();
    let Some(state) = state_guard.as_mut() else {
        return;
    };

    let points = if let (Some(lx), Some(ly)) = (state.last_x, state.last_y) {
        interpolate_line(lx, ly, x, y)
    } else {
        vec![(x, y)]
    };

    for (px, py) in &points {
        paint_circle(&mut state.mask, state.width, state.height, *px, *py, radius);
    }
    state.last_x = Some(x);
    state.last_y = Some(y);
}

fn video_editor_cutout_brush_size_change(ui: &AppWindow, delta: f32) {
    let current = global_store!(ui).get_video_editor_cutout_brush_size();
    let new_size = (current + delta * 0.5).clamp(5.0, 500.0);
    global_store!(ui).set_video_editor_cutout_brush_size(new_size);
}

fn video_editor_cutout_undo(ui: &AppWindow) {
    let mut state_guard = BRUSH_MASK_STATE.lock().unwrap();
    let Some(state) = state_guard.as_mut() else {
        return;
    };

    if let Some(prev_mask) = state.history.pop() {
        state.redo_stack.push(state.mask.clone());
        state.mask = prev_mask;
        state.last_x = None;
        state.last_y = None;

        let overlay = mask_to_overlay(&state.mask);
        drop(state_guard);

        let buffer = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
            overlay.as_raw(),
            overlay.width(),
            overlay.height(),
        );
        global_store!(ui).set_video_editor_cutout_mask_overlay(slint::Image::from_rgba8(buffer));
    }
}

fn video_editor_cutout_redo(ui: &AppWindow) {
    let mut state_guard = BRUSH_MASK_STATE.lock().unwrap();
    let Some(state) = state_guard.as_mut() else {
        return;
    };

    if let Some(redo_mask) = state.redo_stack.pop() {
        state.history.push(state.mask.clone());
        state.mask = redo_mask;
        state.last_x = None;
        state.last_y = None;

        let overlay = mask_to_overlay(&state.mask);
        drop(state_guard);

        let buffer = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
            overlay.as_raw(),
            overlay.width(),
            overlay.height(),
        );
        global_store!(ui).set_video_editor_cutout_mask_overlay(slint::Image::from_rgba8(buffer));
    }
}

fn video_editor_cutout_overlay_refresh(ui: &AppWindow) {
    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        let state_guard = BRUSH_MASK_STATE.lock().unwrap();
        let Some(state) = state_guard.as_ref() else {
            return;
        };
        let overlay = mask_to_overlay(&state.mask);
        drop(state_guard);

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            let buffer = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
                overlay.as_raw(),
                overlay.width(),
                overlay.height(),
            );
            global_store!(ui)
                .set_video_editor_cutout_mask_overlay(slint::Image::from_rgba8(buffer));
        });
    });
}

fn video_editor_cutout_update_config(ui: &AppWindow, config: UICutoutConfig) {
    global_store!(ui).set_video_editor_cutout_config(config.clone());
    save_cutout_config(config.into());
}

fn get_and_check_model_setting(ui: &AppWindow) -> Result<(PathBuf, Model)> {
    let config = global_store!(ui).get_video_editor_cutout_config();
    let models = Model::all_models();
    let index = config
        .selected_model_index
        .clamp(0, models.len() as i32 - 1) as usize;
    let model = models[index];

    let model_path = get_model_path(&config, index as i32).to_string();

    if model_path.is_empty() {
        bail!(tr("Please select a model file").to_string());
    }

    let path = PathBuf::from(&model_path);
    if !path.exists() {
        bail!(tr("Model file not found").to_string());
    }

    Ok((path, model))
}

fn get_model_path(config: &UICutoutConfig, index: i32) -> SharedString {
    match index {
        0 => config.u2net_path.clone(),
        1 => config.u2netp_path.clone(),
        2 => config.u2net_cloth_seg_path.clone(),
        3 => config.u2net_human_seg_path.clone(),
        4 => config.isnet_anime_path.clone(),
        5 => config.isnet_general_use_path.clone(),
        6 => config.silueta_path.clone(),
        _ => SharedString::default(),
    }
}

fn set_model_path(config: &mut UICutoutConfig, index: i32, path: SharedString) {
    match index {
        0 => config.u2net_path = path,
        1 => config.u2netp_path = path,
        2 => config.u2net_cloth_seg_path = path,
        3 => config.u2net_human_seg_path = path,
        4 => config.isnet_anime_path = path,
        5 => config.isnet_general_use_path = path,
        6 => config.silueta_path = path,
        _ => log::warn!("Unexpected cutout model index = {index}"),
    }
}

fn paint_circle(mask: &mut image::GrayImage, w: u32, h: u32, cx: f32, cy: f32, radius: f32) {
    let r2 = radius * radius;
    let x_start = ((cx - radius).max(0.0)) as u32;
    let x_end = ((cx + radius + 1.0).min(w as f32)) as u32;
    let y_start = ((cy - radius).max(0.0)) as u32;
    let y_end = ((cy + radius + 1.0).min(h as f32)) as u32;

    for y in y_start..y_end {
        for x in x_start..x_end {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            if dx * dx + dy * dy <= r2 {
                mask.put_pixel(x, y, image::Luma([255]));
            }
        }
    }
}

fn interpolate_line(x0: f32, y0: f32, x1: f32, y1: f32) -> Vec<(f32, f32)> {
    let dx = x1 - x0;
    let dy = y1 - y0;
    let dist = (dx * dx + dy * dy).sqrt();
    if dist < 1.0 {
        return vec![(x1, y1)];
    }
    let steps = (dist / 2.0).ceil() as i32;
    let mut points = Vec::with_capacity(steps as usize);
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        points.push((x0 + dx * t, y0 + dy * t));
    }
    points
}

fn mask_to_overlay(mask: &image::GrayImage) -> image::RgbaImage {
    let (w, h) = mask.dimensions();
    let mut overlay = image::RgbaImage::new(w, h);
    for (x, y, pixel) in mask.enumerate_pixels() {
        if pixel.0[0] > 0 {
            overlay.put_pixel(x, y, image::Rgba([0, 255, 0, 100]));
        }
    }
    overlay
}
