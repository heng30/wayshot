use crate::{
    db::{DashStyleData, ImageAnimationConfigData, ImageAnimationType, VIDEO_EDITOR_TABLE},
    global_store,
    logic::{
        recorder::picker_directory_with_location,
        share_screen::picker_file,
        toast::async_toast_warn,
        tr::tr,
        video_editor::{playlist::import_file_to_playlist, project::IMG_ANIMATION_CONFIG_ID},
    },
    logic_cb,
    slint_generatedAppWindow::{
        AppWindow, DashStyle as UIDashStyle, GradeMarkType as UIGradeMarkType,
        ImageAnimationConfig, ImageAnimationType as UIImageAnimationType,
    },
};
use image_animation::rect_draw::{DashStyle as RectDashStyle, LineStyle as RectDrawLineStyle};
use image_animation::{
    Animation, AnimationPreviewConfig, AnimationRecordConfig, ArrowDashStyle, ArrowDrawConfig,
    ArrowLineStyle, ArrowStyle, GradeMarkConfig, GradeMarkType, ImageScrollConfig, Receiver,
    RectDrawConfig, RectStyle, RgbaImage,
};
use slint::{ComponentHandle, Image, Weak};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

static RECORD_STOP_SIG: Mutex<Option<Arc<AtomicBool>>> = Mutex::new(None);
static PREVIEW_STOP_SIG: Mutex<Option<Arc<AtomicBool>>> = Mutex::new(None);
pub const IMAGE_EXTENSIONS: [&str; 6] = ["png", "jpg", "jpeg", "bmp", "webp", "tiff"];

crate::impl_c_like_enum_convert!(
    UIImageAnimationType,
    ImageAnimationType,
    Scroll,
    GradeMark,
    Arrow,
    RectDraw
);
crate::impl_c_like_enum_convert!(UIGradeMarkType, GradeMarkType, Circle, Checkmark, Cross);
crate::impl_c_like_enum_convert!(UIDashStyle, DashStyleData, Solid, Dash);
crate::impl_slint_enum_serde!(UIGradeMarkType, Circle, Checkmark, Cross);
crate::impl_slint_enum_serde!(UIDashStyle, Solid, Dash);

pub fn init(ui: &AppWindow) {
    inner_init(ui);

    logic_cb!(video_editor_img_animation_start_preview, ui, config);
    logic_cb!(video_editor_img_animation_stop_preview, ui);
    logic_cb!(video_editor_img_animation_start_record, ui, config);
    logic_cb!(video_editor_img_animation_stop_record, ui);
    logic_cb!(video_editor_img_animation_picker_image, ui);
}

fn inner_init(ui: &AppWindow) {
    let ui_weak = ui.as_weak();

    tokio::spawn(async move {
        let config = match sqldb::entry::select(VIDEO_EDITOR_TABLE, IMG_ANIMATION_CONFIG_ID).await {
            Ok(entry) => {
                serde_json::from_str::<ImageAnimationConfigData>(&entry.data).unwrap_or_default()
            }
            Err(_) => ImageAnimationConfigData::default(),
        };

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            let config: ImageAnimationConfig = config.into();
            global_store!(ui).set_video_editor_img_animation_config(config);
        });
    });
}

fn video_editor_img_animation_start_preview(ui: &AppWindow, config: ImageAnimationConfig) {
    stop_preview_thread();
    global_store!(ui).set_video_editor_img_animation_is_previewing(true);

    let data: ImageAnimationConfigData = config.into();
    save_config_async(&data);

    let Some((anim_config, frame_width)) = prepare_preview_config(ui, &data) else {
        return;
    };

    let frame_receiver = anim_config.receiver();
    let stop_sig = anim_config.stop_sig();
    *PREVIEW_STOP_SIG.lock().unwrap() = Some(stop_sig.clone());

    let data_clone = data.clone();
    std::thread::spawn(move || {
        run_animation_loop(&data_clone, anim_config);
    });

    run_preview_frame_receiver(
        ui.as_weak(),
        frame_receiver,
        stop_sig,
        frame_width,
        data.height as u32,
    );
}

fn video_editor_img_animation_stop_preview(ui: &AppWindow) {
    stop_preview_thread();
    global_store!(ui).set_video_editor_img_animation_is_previewing(false);
    global_store!(ui).set_video_editor_img_animation_preview_image(Image::default());
}

fn stop_preview_thread() {
    if let Some(sig) = PREVIEW_STOP_SIG.lock().unwrap().take() {
        sig.store(true, Ordering::SeqCst);
    }
}

fn prepare_preview_config(
    ui: &AppWindow,
    data: &ImageAnimationConfigData,
) -> Option<(AnimationPreviewConfig, u32)> {
    let height = data.height as u32;
    let fps = data.fps as u32;

    match data.animation_type {
        ImageAnimationType::Scroll => {
            let image_path = PathBuf::from(&data.scroll.image_path);
            if !image_path.exists() {
                global_store!(ui).set_video_editor_img_animation_is_previewing(false);
                return None;
            }

            let scroll_config = ImageScrollConfig::new(image_path)
                .with_output_height(height)
                .with_fps(fps);

            let image_width = match scroll_config.validate() {
                Ok(w) => w,
                Err(e) => {
                    log::warn!("Failed to validate image: {}", e);
                    global_store!(ui).set_video_editor_img_animation_is_previewing(false);
                    return None;
                }
            };

            Some((
                AnimationPreviewConfig::new(image_width, height, fps),
                image_width,
            ))
        }
        ImageAnimationType::GradeMark => {
            let width = data.width as u32;
            Some((AnimationPreviewConfig::new(width, height, fps), width))
        }
        ImageAnimationType::Arrow => {
            let width = data.width as u32;
            Some((AnimationPreviewConfig::new(width, height, fps), width))
        }
        ImageAnimationType::RectDraw => {
            let width = data.width as u32;
            Some((AnimationPreviewConfig::new(width, height, fps), width))
        }
    }
}

fn run_preview_frame_receiver(
    ui_weak: Weak<AppWindow>,
    frame_receiver: Receiver<RgbaImage>,
    stop_sig: Arc<AtomicBool>,
    frame_width: u32,
    frame_height: u32,
) {
    std::thread::spawn(move || {
        loop {
            if stop_sig.load(Ordering::SeqCst) {
                break;
            }

            match frame_receiver.recv_timeout(Duration::from_millis(100)) {
                Ok(frame) => {
                    let pixels: Vec<u8> = frame.into_raw();
                    let stop_sig_clone = stop_sig.clone();

                    _ = ui_weak.upgrade_in_event_loop(move |ui| {
                        if !stop_sig_clone.load(Ordering::SeqCst) {
                            let image = Image::from_rgba8(slint::SharedPixelBuffer::<
                                slint::Rgba8Pixel,
                            >::clone_from_slice(
                                &pixels, frame_width, frame_height
                            ));
                            global_store!(ui).set_video_editor_img_animation_preview_image(image);
                        }
                    });
                }
                Err(crossbeam::channel::RecvTimeoutError::Disconnected) => {
                    _ = ui_weak.upgrade_in_event_loop(|ui| {
                        global_store!(ui).set_video_editor_img_animation_is_previewing(false);
                        global_store!(ui)
                            .set_video_editor_img_animation_preview_image(Default::default());
                    });
                    break;
                }
                Err(crossbeam::channel::RecvTimeoutError::Timeout) => {}
            }
        }
    });
}

fn save_config_async(data: &ImageAnimationConfigData) {
    let data_clone = data.clone();
    tokio::spawn(async move {
        save_img_animation_config(&data_clone).await;
    });
}

fn run_record_progress_receiver(
    ui_weak: Weak<AppWindow>,
    progress_receiver: Receiver<f32>,
    stop_sig: Arc<AtomicBool>,
    output_path: PathBuf,
) {
    std::thread::spawn(move || {
        loop {
            if stop_sig.load(Ordering::SeqCst) {
                break;
            }

            if let Ok(progress) = progress_receiver.recv_timeout(Duration::from_millis(100)) {
                let stop_sig_inner = stop_sig.clone();
                _ = ui_weak.upgrade_in_event_loop(move |ui| {
                    if !stop_sig_inner.load(Ordering::SeqCst) {
                        global_store!(ui).set_video_editor_img_animation_record_progress(progress);
                    }
                });

                if progress >= 1.0 {
                    break;
                }
            }
        }

        _ = ui_weak.clone().upgrade_in_event_loop(|ui| {
            global_store!(ui).set_video_editor_img_animation_is_recording(false);
            global_store!(ui).set_video_editor_img_animation_record_progress(1.0);
        });

        if !stop_sig.load(Ordering::SeqCst) {
            _ = slint::invoke_from_event_loop(move || {
                tokio::spawn(async move {
                    import_file_to_playlist(ui_weak, output_path, None).await;
                });
            });
        }
    });
}

fn run_animation_loop(config: &ImageAnimationConfigData, anim_config: AnimationPreviewConfig) {
    match config.animation_type {
        ImageAnimationType::Scroll => run_scroll_preview(config, anim_config),
        ImageAnimationType::GradeMark => run_grade_mark_preview(config, anim_config),
        ImageAnimationType::Arrow => run_arrow_preview(config, anim_config),
        ImageAnimationType::RectDraw => run_rect_draw_preview(config, anim_config),
    }
}

fn run_scroll_preview(config: &ImageAnimationConfigData, anim_config: AnimationPreviewConfig) {
    let mut scroll_config = ImageScrollConfig::new(PathBuf::from(&config.scroll.image_path))
        .with_output_height(config.height as u32)
        .with_fps(config.fps as u32)
        .with_scroll_speed(config.scroll.scroll_speed.clamp(0.0, 1.0))
        .with_start_pause(config.scroll.start_pause)
        .with_end_pause(config.scroll.end_pause);

    _ = scroll_config.animate_preview(anim_config);
}

fn run_grade_mark_preview(config: &ImageAnimationConfigData, anim_config: AnimationPreviewConfig) {
    let mark_type: GradeMarkType = config.grade_mark.mark_type.into();

    let color = (
        config.grade_mark.color_r as u8,
        config.grade_mark.color_g as u8,
        config.grade_mark.color_b as u8,
        config.grade_mark.color_a as u8,
    );

    let position = (config.grade_mark.position_x, config.grade_mark.position_y);

    let mut grade_mark_config = GradeMarkConfig::new(mark_type)
        .with_color(color)
        .with_size(config.grade_mark.size)
        .with_line_width(config.grade_mark.line_width)
        .with_duration_ms(config.grade_mark.duration_ms as u32)
        .with_end_pause(config.grade_mark.end_pause)
        .with_position(position)
        .with_width(config.width as u32)
        .with_height(config.height as u32);

    _ = grade_mark_config.animate_preview(anim_config);
}

fn make_arrow_dash_style(dash_style: UIDashStyle, dash_length: f32) -> ArrowDashStyle {
    let data: DashStyleData = dash_style.into();
    match data {
        DashStyleData::Solid => ArrowDashStyle::Solid,
        DashStyleData::Dash => ArrowDashStyle::Dash(dash_length),
    }
}

fn run_arrow_preview(config: &ImageAnimationConfigData, anim_config: AnimationPreviewConfig) {
    let color = (
        config.arrow.color_r as u8,
        config.arrow.color_g as u8,
        config.arrow.color_b as u8,
        config.arrow.color_a as u8,
    );
    let position = (config.arrow.position_x, config.arrow.position_y);
    let dash = make_arrow_dash_style(config.arrow.dash_style, config.arrow.dash_length);

    let mut arrow_config = ArrowDrawConfig::new()
        .with_line_style(ArrowLineStyle {
            color,
            width: config.arrow.line_width,
            dash,
        })
        .with_arrow_style(ArrowStyle {
            length: config.arrow.length,
            head_length: config.arrow.head_length,
            head_width: config.arrow.head_width,
            direction: config.arrow.direction,
        })
        .with_duration_ms(config.arrow.duration_ms as u32)
        .with_end_pause(config.arrow.end_pause)
        .with_position(position)
        .with_width(config.width as u32)
        .with_height(config.height as u32);

    _ = arrow_config.animate_preview(anim_config);
}

fn make_rect_dash_style(dash_style: UIDashStyle, dash_length: f32) -> RectDashStyle {
    let data: DashStyleData = dash_style.into();
    match data {
        DashStyleData::Solid => RectDashStyle::Solid,
        DashStyleData::Dash => RectDashStyle::Dash(dash_length),
    }
}

fn run_rect_draw_preview(config: &ImageAnimationConfigData, anim_config: AnimationPreviewConfig) {
    let color = (
        config.rect_draw.color_r as u8,
        config.rect_draw.color_g as u8,
        config.rect_draw.color_b as u8,
        config.rect_draw.color_a as u8,
    );
    let position = (config.rect_draw.position_x, config.rect_draw.position_y);
    let dash = make_rect_dash_style(config.rect_draw.dash_style, config.rect_draw.dash_length);

    let mut rect_config = RectDrawConfig::new()
        .with_line_style(RectDrawLineStyle {
            color,
            width: config.rect_draw.line_width,
            dash,
        })
        .with_rect_style(RectStyle {
            width: config.rect_draw.rect_width,
            height: config.rect_draw.rect_height,
            corner_radius: config.rect_draw.corner_radius,
        })
        .with_duration_ms(config.rect_draw.duration_ms as u32)
        .with_end_pause(config.rect_draw.end_pause)
        .with_position(position)
        .with_width(config.width as u32)
        .with_height(config.height as u32);

    _ = rect_config.animate_preview(anim_config);
}

fn video_editor_img_animation_start_record(ui: &AppWindow, config: ImageAnimationConfig) {
    stop_preview_thread();
    stop_record_thread();

    let ui_weak = ui.as_weak();
    let data: ImageAnimationConfigData = config.into();

    tokio::spawn(async move {
        let mut data = data;
        if data.animation_type == ImageAnimationType::Scroll {
            let image_path = PathBuf::from(&data.scroll.image_path);
            if !image_path.exists() {
                async_toast_warn(ui_weak.clone(), tr("Please import image"));
                return;
            }
        }

        let Some(dir) = picker_directory_with_location(
            ui_weak.clone(),
            &tr("Choose save directory"),
            &data.save_dir,
        ) else {
            return;
        };

        data.save_dir = dir.to_string_lossy().to_string();
        save_img_animation_config(&data).await;

        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let filename = match data.animation_type {
            ImageAnimationType::Scroll => format!("img_scroll_{}.mp4", timestamp),
            ImageAnimationType::GradeMark => {
                let mark_type: GradeMarkType = data.grade_mark.mark_type.into();
                format!("grade_mark_{}_{}.webp", mark_type, timestamp)
            }
            ImageAnimationType::Arrow => {
                let dash: ArrowDashStyle =
                    make_arrow_dash_style(data.arrow.dash_style, data.arrow.dash_length);
                format!("arrow_{}_{}.webp", dash, timestamp)
            }
            ImageAnimationType::RectDraw => {
                let dash: RectDashStyle =
                    make_rect_dash_style(data.rect_draw.dash_style, data.rect_draw.dash_length);
                format!("rect_draw_{}_{}.webp", dash, timestamp)
            }
        };
        generate_animation_video(ui_weak, data, dir.join(&filename)).await;
    });
}

fn video_editor_img_animation_stop_record(ui: &AppWindow) {
    stop_record_thread();
    global_store!(ui).set_video_editor_img_animation_is_recording(false);
}

fn stop_record_thread() {
    if let Some(sig) = RECORD_STOP_SIG.lock().unwrap().take() {
        sig.store(true, Ordering::SeqCst);
    }
}

async fn generate_animation_video(
    ui_weak: Weak<AppWindow>,
    config: ImageAnimationConfigData,
    output_path: PathBuf,
) {
    match config.animation_type {
        ImageAnimationType::Scroll => {
            generate_scroll_video_impl(ui_weak, config, output_path).await;
        }
        ImageAnimationType::GradeMark => {
            generate_grade_mark_video_impl(ui_weak, config, output_path).await;
        }
        ImageAnimationType::Arrow => {
            generate_arrow_video_impl(ui_weak, config, output_path).await;
        }
        ImageAnimationType::RectDraw => {
            generate_rect_draw_video_impl(ui_weak, config, output_path).await;
        }
    }
}

async fn generate_scroll_video_impl(
    ui_weak: Weak<AppWindow>,
    config: ImageAnimationConfigData,
    output_path: PathBuf,
) {
    _ = ui_weak.upgrade_in_event_loop(|ui| {
        global_store!(ui).set_video_editor_img_animation_is_recording(true);
        global_store!(ui).set_video_editor_img_animation_record_progress(0.0);
    });

    let height = config.height as u32;
    let fps = config.fps as u32;

    let scroll_config = ImageScrollConfig::new(PathBuf::from(&config.scroll.image_path))
        .with_output_height(height)
        .with_fps(fps);
    let image_width = match scroll_config.validate() {
        Ok(w) => w,
        Err(e) => {
            log::warn!("Failed to validate image for recording: {}", e);
            _ = ui_weak.upgrade_in_event_loop(|ui| {
                global_store!(ui).set_video_editor_img_animation_is_recording(false);
            });
            return;
        }
    };

    let anim_config = AnimationRecordConfig::new(
        image_width,
        height,
        fps,
        Duration::ZERO,
        output_path.clone(),
    );
    let stop_sig = anim_config.stop_sig();
    let progress_receiver = anim_config.progress_receiver();
    *RECORD_STOP_SIG.lock().unwrap() = Some(stop_sig.clone());

    let mut scroll_config = ImageScrollConfig::new(PathBuf::from(&config.scroll.image_path))
        .with_output_height(height)
        .with_fps(fps)
        .with_scroll_speed(config.scroll.scroll_speed)
        .with_start_pause(config.scroll.start_pause)
        .with_end_pause(config.scroll.end_pause);

    std::thread::spawn(move || {
        if let Err(e) = scroll_config.animate_record(anim_config) {
            log::error!("animate_record failed: {}", e);
        }
    });

    run_record_progress_receiver(ui_weak, progress_receiver, stop_sig, output_path);
}

async fn generate_grade_mark_video_impl(
    ui_weak: Weak<AppWindow>,
    config: ImageAnimationConfigData,
    output_path: PathBuf,
) {
    _ = ui_weak.upgrade_in_event_loop(|ui| {
        global_store!(ui).set_video_editor_img_animation_is_recording(true);
        global_store!(ui).set_video_editor_img_animation_record_progress(0.0);
    });

    let width = config.width as u32;
    let height = config.height as u32;
    let fps = config.fps as u32;

    let total_duration_ms =
        config.grade_mark.duration_ms as u32 + (config.grade_mark.end_pause * 1000.0) as u32;
    let duration = Duration::from_millis(total_duration_ms as u64);

    let anim_config = AnimationRecordConfig::new(width, height, fps, duration, output_path.clone());
    let stop_sig = anim_config.stop_sig();
    let progress_receiver = anim_config.progress_receiver();
    *RECORD_STOP_SIG.lock().unwrap() = Some(stop_sig.clone());

    let mark_type: GradeMarkType = config.grade_mark.mark_type.into();
    let color = (
        config.grade_mark.color_r as u8,
        config.grade_mark.color_g as u8,
        config.grade_mark.color_b as u8,
        config.grade_mark.color_a as u8,
    );
    let position = (config.grade_mark.position_x, config.grade_mark.position_y);
    let mut grade_mark_config = GradeMarkConfig::new(mark_type)
        .with_color(color)
        .with_size(config.grade_mark.size)
        .with_line_width(config.grade_mark.line_width)
        .with_duration_ms(config.grade_mark.duration_ms as u32)
        .with_end_pause(config.grade_mark.end_pause)
        .with_position(position)
        .with_width(width)
        .with_height(height);

    std::thread::spawn(move || {
        if let Err(e) = grade_mark_config.animate_record_webp(anim_config) {
            log::error!("grade_mark animate_record_webp failed: {}", e);
        }
    });

    run_record_progress_receiver(ui_weak, progress_receiver, stop_sig, output_path);
}

async fn generate_arrow_video_impl(
    ui_weak: Weak<AppWindow>,
    config: ImageAnimationConfigData,
    output_path: PathBuf,
) {
    _ = ui_weak.upgrade_in_event_loop(|ui| {
        global_store!(ui).set_video_editor_img_animation_is_recording(true);
        global_store!(ui).set_video_editor_img_animation_record_progress(0.0);
    });

    let width = config.width as u32;
    let height = config.height as u32;
    let fps = config.fps as u32;

    let total_duration_ms =
        config.arrow.duration_ms as u32 + (config.arrow.end_pause * 1000.0) as u32;
    let duration = Duration::from_millis(total_duration_ms as u64);

    let anim_config = AnimationRecordConfig::new(width, height, fps, duration, output_path.clone());
    let stop_sig = anim_config.stop_sig();
    let progress_receiver = anim_config.progress_receiver();
    *RECORD_STOP_SIG.lock().unwrap() = Some(stop_sig.clone());

    let color = (
        config.arrow.color_r as u8,
        config.arrow.color_g as u8,
        config.arrow.color_b as u8,
        config.arrow.color_a as u8,
    );
    let position = (config.arrow.position_x, config.arrow.position_y);
    let dash = make_arrow_dash_style(config.arrow.dash_style, config.arrow.dash_length);

    let mut arrow_config = ArrowDrawConfig::new()
        .with_line_style(ArrowLineStyle {
            color,
            width: config.arrow.line_width,
            dash,
        })
        .with_arrow_style(ArrowStyle {
            length: config.arrow.length,
            head_length: config.arrow.head_length,
            head_width: config.arrow.head_width,
            direction: config.arrow.direction,
        })
        .with_duration_ms(config.arrow.duration_ms as u32)
        .with_end_pause(config.arrow.end_pause)
        .with_position(position)
        .with_width(width)
        .with_height(height);

    std::thread::spawn(move || {
        if let Err(e) = arrow_config.animate_record_webp(anim_config) {
            log::error!("arrow animate_record_webp failed: {}", e);
        }
    });

    run_record_progress_receiver(ui_weak, progress_receiver, stop_sig, output_path);
}

async fn generate_rect_draw_video_impl(
    ui_weak: Weak<AppWindow>,
    config: ImageAnimationConfigData,
    output_path: PathBuf,
) {
    _ = ui_weak.upgrade_in_event_loop(|ui| {
        global_store!(ui).set_video_editor_img_animation_is_recording(true);
        global_store!(ui).set_video_editor_img_animation_record_progress(0.0);
    });

    let width = config.width as u32;
    let height = config.height as u32;
    let fps = config.fps as u32;

    let total_duration_ms =
        config.rect_draw.duration_ms as u32 + (config.rect_draw.end_pause * 1000.0) as u32;
    let duration = Duration::from_millis(total_duration_ms as u64);

    let anim_config = AnimationRecordConfig::new(width, height, fps, duration, output_path.clone());
    let stop_sig = anim_config.stop_sig();
    let progress_receiver = anim_config.progress_receiver();
    *RECORD_STOP_SIG.lock().unwrap() = Some(stop_sig.clone());

    let color = (
        config.rect_draw.color_r as u8,
        config.rect_draw.color_g as u8,
        config.rect_draw.color_b as u8,
        config.rect_draw.color_a as u8,
    );
    let position = (config.rect_draw.position_x, config.rect_draw.position_y);
    let dash = make_rect_dash_style(config.rect_draw.dash_style, config.rect_draw.dash_length);

    let mut rect_config = RectDrawConfig::new()
        .with_line_style(RectDrawLineStyle {
            color,
            width: config.rect_draw.line_width,
            dash,
        })
        .with_rect_style(RectStyle {
            width: config.rect_draw.rect_width,
            height: config.rect_draw.rect_height,
            corner_radius: config.rect_draw.corner_radius,
        })
        .with_duration_ms(config.rect_draw.duration_ms as u32)
        .with_end_pause(config.rect_draw.end_pause)
        .with_position(position)
        .with_width(width)
        .with_height(height);

    std::thread::spawn(move || {
        if let Err(e) = rect_config.animate_record_webp(anim_config) {
            log::error!("rect_draw animate_record_webp failed: {}", e);
        }
    });

    run_record_progress_receiver(ui_weak, progress_receiver, stop_sig, output_path);
}

fn video_editor_img_animation_picker_image(ui: &AppWindow) {
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

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            let mut config = global_store!(ui).get_video_editor_img_animation_config();
            config.scroll.image_path = filepath.to_string_lossy().to_string().into();
            global_store!(ui).set_video_editor_img_animation_config(config);
        });
    });
}

async fn save_img_animation_config(config: &ImageAnimationConfigData) {
    let data = serde_json::to_string(config).expect("Serialize ImageAnimationConfigData");
    if sqldb::entry::insert(VIDEO_EDITOR_TABLE, IMG_ANIMATION_CONFIG_ID, &data)
        .await
        .is_err()
    {
        if let Err(e) =
            sqldb::entry::update(VIDEO_EDITOR_TABLE, IMG_ANIMATION_CONFIG_ID, &data).await
        {
            log::warn!("Failed to save img_animation config: {}", e);
        }
    }
}
