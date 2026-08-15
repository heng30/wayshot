use crate::{
    db::{TtfxConfigData, VIDEO_EDITOR_TABLE},
    global_store,
    logic::{
        toast::async_toast_warn,
        tr::tr,
        video_editor::{
            export::picker_save_file, playlist::import_file_to_playlist, project::TTFX_CONFIG_ID,
        },
    },
    logic_cb,
    slint_generatedAppWindow::{AppWindow, TtfxConfig as UITtfxConfig},
};
use crossbeam::channel::{Receiver, Sender};
use slint::{ComponentHandle, Image, Weak};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use ttfx_rs::{
    effects::EffectCommand,
    engine::canvas::Anchor,
    render::{Font, RenderConfig, SequenceRenderer},
    utils::graphics::Color,
};
use video_encoder::{EncodedFrame, VideoEncoderConfig};

static PREVIEW_STOP_SIG: Mutex<Option<Arc<AtomicBool>>> = Mutex::new(None);
static RECORD_STOP_SIG: Mutex<Option<Arc<AtomicBool>>> = Mutex::new(None);

pub fn init(ui: &AppWindow) {
    inner_init(ui);

    logic_cb!(video_editor_ttfx_start_preview, ui, config);
    logic_cb!(video_editor_ttfx_stop_preview, ui);
    logic_cb!(video_editor_ttfx_start_record, ui, config);
    logic_cb!(video_editor_ttfx_stop_record, ui);
    logic_cb!(video_editor_ttfx_update_config, ui, config);
}

fn inner_init(ui: &AppWindow) {
    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        let config = match sqldb::entry::select(VIDEO_EDITOR_TABLE, TTFX_CONFIG_ID).await {
            Ok(entry) => serde_json::from_str::<TtfxConfigData>(&entry.data).unwrap_or_default(),
            Err(_) => TtfxConfigData::default(),
        };

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            let ui_config = UITtfxConfig::from(config);
            global_store!(ui).set_video_editor_ttfx_config(ui_config);
        });
    });
}

fn video_editor_ttfx_update_config(ui: &AppWindow, config: UITtfxConfig) {
    global_store!(ui).set_video_editor_ttfx_config(config.clone());
    let data = TtfxConfigData::from(config);
    save_config_async(&data);
}

fn save_config_async(data: &TtfxConfigData) {
    let data_clone = data.clone();
    tokio::spawn(async move {
        save_ttfx_config(&data_clone).await;
    });
}

async fn save_ttfx_config(config: &TtfxConfigData) {
    let data = serde_json::to_string(config).expect("serialize ttfx config failed");
    if sqldb::entry::insert(VIDEO_EDITOR_TABLE, TTFX_CONFIG_ID, &data)
        .await
        .is_err()
    {
        if let Err(e) = sqldb::entry::update(VIDEO_EDITOR_TABLE, TTFX_CONFIG_ID, &data).await {
            log::warn!("Failed to save ttfx config: {:?}", e);
        }
    }
}

/// 把 UI 配置转换成 ttfx-rs 的渲染配置。字体缺失或文本为空时返回 None。
fn prepare_render(ui: &AppWindow, data: &TtfxConfigData) -> Option<(String, String, RenderConfig)> {
    if data.text.trim().is_empty() {
        crate::toast_warn!(ui, tr("Please enter text first"));
        return None;
    }

    let ascii_font_path = PathBuf::from(&data.ascii_font_path);
    if !ascii_font_path.exists() {
        crate::toast_warn!(
            ui,
            format!(
                "{}: {}",
                tr("No found ascii font path"),
                data.ascii_font_path
            )
        );
        return None;
    }
    let non_ascii_font_path = PathBuf::from(&data.non_ascii_font_path);
    if !non_ascii_font_path.exists() {
        crate::toast_warn!(
            ui,
            format!(
                "{}: {}",
                tr("No found non-ascii font path"),
                data.non_ascii_font_path
            )
        );
        return None;
    }

    let font = match Font::from_files(&ascii_font_path, &non_ascii_font_path) {
        Ok(font) => font,
        Err(e) => {
            crate::toast_warn!(ui, format!("{}: {}", tr("Failed to load font"), e));
            return None;
        }
    };

    // 背景色支持 6 位（不透明）和 8 位（带 alpha）hex；alpha 仅 GIF 导出生效。
    let (background, background_alpha) = {
        let hex = data.background_color.trim().trim_start_matches('#');
        if hex.len() == 8 {
            let alpha = u8::from_str_radix(&hex[6..8], 16).unwrap_or(255);
            let color = Color::from_hex(&hex[..6])
                .unwrap_or_else(|_| Color::from_hex("000000").expect("valid hex"));
            (color, alpha)
        } else {
            let color = if hex.is_empty() {
                Color::from_hex("000000").expect("valid hex")
            } else {
                Color::from_hex(hex)
                    .unwrap_or_else(|_| Color::from_hex("000000").expect("valid hex"))
            };
            (color, 255)
        }
    };

    // 固定分辨率画布：用户通过分辨率 + 字体大小灵活调整，避免字符被裁剪。
    let width = (data.width.max(64) as u32) & !1;
    let height = (data.height.max(64) as u32) & !1;
    let mut render = RenderConfig::new(width, height, font);
    // 字号按高度缩放到 1080P 基准，保证不同分辨率下视觉大小一致。
    let scale = height as f32 / 1080.0;
    render.font_size = data.font_size.max(4.0) * scale;
    render.fps = data.fps.max(1) as u32;
    render.background = background;

    // GIF 导出支持透明背景，MP4 不透明。
    render.background_alpha = if data.export_format == "gif" {
        background_alpha
    } else {
        255
    };

    // 库默认把文本锚定在画布左下角（Sw），这里改为居中。
    render.anchor_text = Anchor::C;
    render.seed = if data.seed > 0 {
        Some(data.seed as u64)
    } else {
        None
    };

    Some((data.text.clone(), data.effect_name.clone(), render))
}

fn video_editor_ttfx_start_preview(ui: &AppWindow, config: UITtfxConfig) {
    stop_preview_thread();
    global_store!(ui).set_video_editor_ttfx_is_previewing(true);

    let data: TtfxConfigData = config.into();
    save_config_async(&data);

    let Some((input, effect_name, render)) = prepare_render(ui, &data) else {
        global_store!(ui).set_video_editor_ttfx_is_previewing(false);
        return;
    };

    let stop_sig = Arc::new(AtomicBool::new(false));
    *PREVIEW_STOP_SIG.lock().unwrap() = Some(stop_sig.clone());

    let (sender, receiver) = crossbeam::channel::bounded::<image::RgbaImage>(2);
    let ui_weak = ui.as_weak();
    let fps = render.fps;
    let thread_stop_sig = stop_sig.clone();
    let loops = data.loops.max(1) as usize;

    std::thread::spawn(move || {
        let frame_duration = Duration::from_secs_f32(1.0 / fps as f32);
        let start = Instant::now();
        let mut frame_idx: u32 = 0;

        for _ in 0..loops {
            let Some(mut effect) = EffectCommand::from_name(&effect_name).map(|c| c.build_effect())
            else {
                return;
            };
            let mut renderer = match SequenceRenderer::new(&input, render.clone()) {
                Ok(renderer) => renderer,
                Err(e) => {
                    log::warn!("ttfx preview failed to create renderer: {e}");
                    return;
                }
            };
            if let Err(e) = renderer.build_effect(effect.as_mut()) {
                log::warn!("ttfx preview failed to build effect: {e}");
                return;
            }

            while let Some(frame) = renderer.next_frame(effect.as_mut()) {
                if thread_stop_sig.load(Ordering::SeqCst) {
                    return;
                }
                spin_sleep::sleep_until(start + frame_duration * frame_idx);
                if sender.try_send(frame.image).is_err() {
                    // 接收端（UI）繁忙时丢弃这一帧，保证渲染线程不被阻塞。
                }
                frame_idx += 1;
            }
        }
    });

    run_preview_frame_receiver(ui_weak, receiver, stop_sig);
}

fn video_editor_ttfx_stop_preview(ui: &AppWindow) {
    stop_preview_thread();
    global_store!(ui).set_video_editor_ttfx_is_previewing(false);
    global_store!(ui).set_video_editor_ttfx_preview_image(Image::default());
}

fn stop_preview_thread() {
    if let Some(sig) = PREVIEW_STOP_SIG.lock().unwrap().take() {
        sig.store(true, Ordering::SeqCst);
    }
}

fn run_preview_frame_receiver(
    ui_weak: Weak<AppWindow>,
    frame_receiver: Receiver<image::RgbaImage>,
    stop_sig: Arc<AtomicBool>,
) {
    std::thread::spawn(move || {
        loop {
            if stop_sig.load(Ordering::SeqCst) {
                break;
            }
            match frame_receiver.recv_timeout(Duration::from_millis(100)) {
                Ok(frame) => {
                    let width = frame.width();
                    let height = frame.height();
                    let pixels: Vec<u8> = frame.into_raw();
                    let stop_sig_clone = stop_sig.clone();
                    _ = ui_weak.upgrade_in_event_loop(move |ui| {
                        if !stop_sig_clone.load(Ordering::SeqCst) {
                            let image = Image::from_rgba8(slint::SharedPixelBuffer::<
                                slint::Rgba8Pixel,
                            >::clone_from_slice(
                                &pixels, width, height
                            ));
                            global_store!(ui).set_video_editor_ttfx_preview_image(image);
                        }
                    });
                }
                Err(crossbeam::channel::RecvTimeoutError::Disconnected) => {
                    _ = ui_weak.upgrade_in_event_loop(|ui| {
                        global_store!(ui).set_video_editor_ttfx_is_previewing(false);
                        global_store!(ui).set_video_editor_ttfx_preview_image(Image::default());
                    });
                    break;
                }
                Err(crossbeam::channel::RecvTimeoutError::Timeout) => {}
            }
        }
    });
}

fn video_editor_ttfx_start_record(ui: &AppWindow, config: UITtfxConfig) {
    stop_record_thread();
    global_store!(ui).set_video_editor_ttfx_is_recording(true);
    global_store!(ui).set_video_editor_ttfx_record_progress(0.0);

    let data: TtfxConfigData = config.into();
    save_config_async(&data);

    let Some((input, effect_name, render)) = prepare_render(ui, &data) else {
        global_store!(ui).set_video_editor_ttfx_is_recording(false);
        return;
    };
    let loops = data.loops.max(1) as usize;

    let stop_sig = Arc::new(AtomicBool::new(false));
    *RECORD_STOP_SIG.lock().unwrap() = Some(stop_sig.clone());
    let (progress_sender, progress_receiver) = crossbeam::channel::bounded(1);

    let ui_weak = ui.as_weak();
    let ui_weak_worker = ui_weak.clone();
    let thread_stop_sig = stop_sig.clone();

    tokio::spawn(async move {
        let is_gif = data.export_format == "gif";
        let (filter_name, ext, default_name) = if is_gif {
            (
                tr("GIF Image"),
                "gif".to_string(),
                format!("ttfx_{}.gif", chrono::Local::now().format("%Y%m%d_%H%M%S")),
            )
        } else {
            (
                tr("MP4 Video"),
                "mp4".to_string(),
                format!("ttfx_{}.mp4", chrono::Local::now().format("%Y%m%d_%H%M%S")),
            )
        };

        let Some(output_path) = picker_save_file(
            ui_weak_worker.clone(),
            &tr("Export TTFX Video"),
            &filter_name,
            &[ext.as_str()],
            &default_name,
        ) else {
            stop_record_thread();
            _ = ui_weak_worker.upgrade_in_event_loop(move |ui| {
                global_store!(ui).set_video_editor_ttfx_is_recording(false);
                global_store!(ui).set_video_editor_ttfx_record_progress(0.0);
            });
            return;
        };

        std::thread::spawn(move || {
            let result = if is_gif {
                record_to_gif(
                    &input,
                    &effect_name,
                    &render,
                    loops,
                    &output_path,
                    &thread_stop_sig,
                    &progress_sender,
                )
            } else {
                record_to_mp4(
                    &input,
                    &effect_name,
                    &render,
                    loops,
                    &output_path,
                    &thread_stop_sig,
                    &progress_sender,
                )
            };
            match result {
                Ok(path) => {
                    _ = progress_sender.send(1.0);
                    _ = ui_weak_worker.upgrade_in_event_loop(move |ui| {
                        let ui_weak = ui.as_weak();
                        tokio::spawn(async move {
                            import_file_to_playlist(ui_weak, path, None).await;
                        });
                    });
                }
                Err(e) => {
                    log::warn!("ttfx record failed: {e}");
                    _ = progress_sender.send(1.0);
                    if !thread_stop_sig.load(Ordering::SeqCst) {
                        async_toast_warn(ui_weak_worker, format!("{}: {e}", tr("Record failed")));
                    }
                }
            }
        });
    });

    run_record_progress_receiver(ui_weak, progress_receiver, stop_sig);
}

fn video_editor_ttfx_stop_record(ui: &AppWindow) {
    stop_record_thread();
    global_store!(ui).set_video_editor_ttfx_is_recording(false);
}

fn stop_record_thread() {
    if let Some(sig) = RECORD_STOP_SIG.lock().unwrap().take() {
        sig.store(true, Ordering::SeqCst);
    }
}

fn run_record_progress_receiver(
    ui_weak: Weak<AppWindow>,
    progress_receiver: Receiver<f32>,
    stop_sig: Arc<AtomicBool>,
) {
    std::thread::spawn(move || {
        loop {
            if stop_sig.load(Ordering::SeqCst) {
                break;
            }
            match progress_receiver.recv_timeout(Duration::from_millis(100)) {
                Ok(progress) => {
                    let stop_sig_inner = stop_sig.clone();
                    _ = ui_weak.upgrade_in_event_loop(move |ui| {
                        if !stop_sig_inner.load(Ordering::SeqCst) {
                            global_store!(ui).set_video_editor_ttfx_record_progress(progress);
                        }
                    });
                    if progress >= 1.0 {
                        break;
                    }
                }
                // 渲染线程结束、sender 被丢弃时退出循环。
                Err(crossbeam::channel::RecvTimeoutError::Disconnected) => break,
                Err(crossbeam::channel::RecvTimeoutError::Timeout) => {}
            }
        }

        _ = ui_weak.clone().upgrade_in_event_loop(|ui| {
            global_store!(ui).set_video_editor_ttfx_is_recording(false);
            global_store!(ui).set_video_editor_ttfx_record_progress(1.0);
        });
    });
}

/// 把效果渲染成 mp4。输出尺寸为偶数（H.264 要求），奇数时用背景色补 1 像素。
/// 动画按 `loops` 参数循环重放（相同 seed 产生相同动画）。
fn record_to_mp4(
    input: &str,
    effect_name: &str,
    render: &RenderConfig,
    loops: usize,
    output_path: &Path,
    stop_sig: &Arc<AtomicBool>,
    progress_sender: &Sender<f32>,
) -> Result<PathBuf, String> {
    // 先探测一遍：获得总帧数、尺寸。
    let (frame_count, frame_width, frame_height) = probe_render(input, effect_name, render)?;
    if stop_sig.load(Ordering::SeqCst) {
        return Err("record stopped".to_string());
    }

    let width = (frame_width + 1) & !1;
    let height = (frame_height + 1) & !1;
    let need_pad = width != frame_width || height != frame_height;

    let total_frames = frame_count * loops as u32;

    let mp4_config = mp4m::mp4_processor::Mp4ProcessorConfigBuilder::default()
        .save_path(output_path.to_path_buf())
        .video_config(mp4m::mp4_processor::VideoConfig {
            width,
            height,
            fps: render.fps,
        })
        .build()
        .map_err(|e| format!("Failed to build MP4 config: {e}"))?;
    let mut processor = mp4m::mp4_processor::Mp4Processor::new(mp4_config);
    let video_sender = processor.h264_sender();

    let encoder_config = VideoEncoderConfig::new(width, height).with_fps(render.fps);
    let mut encoder = video_encoder::new(encoder_config)
        .map_err(|e| format!("Failed to create video encoder: {e}"))?;
    let headers = encoder
        .headers()
        .map_err(|e| format!("Failed to get encoder headers: {e}"))?;

    let processor_thread = std::thread::spawn(move || {
        if let Err(e) = processor.run_processing_loop(Some(headers)) {
            log::warn!("MP4 processing error: {}", e);
        }
    });

    let background_rgba = render.background.rgb_ints();
    let mut encoded_frames: u32 = 0;

    for _ in 0..loops {
        let Some(mut effect) = EffectCommand::from_name(effect_name).map(|c| c.build_effect())
        else {
            return Err(format!("unknown effect: {effect_name}"));
        };
        let mut renderer = SequenceRenderer::new(input, render.clone())
            .map_err(|e| format!("Failed to create renderer: {e}"))?;
        renderer
            .build_effect(effect.as_mut())
            .map_err(|e| format!("Failed to build effect: {e}"))?;

        while let Some(frame) = renderer.next_frame(effect.as_mut()) {
            if stop_sig.load(Ordering::SeqCst) {
                return Err("record stopped".to_string());
            }
            encoded_frames += 1;
            let progress = (encoded_frames as f32 / total_frames as f32).min(0.99);
            _ = progress_sender.try_send(progress);

            let rgb = frame_to_rgb(frame.image, width, height, need_pad, background_rgba);
            match encoder.encode_frame(rgb) {
                Ok(EncodedFrame::Frame {
                    data, is_keyframe, ..
                }) => {
                    if video_sender
                        .send(mp4m::mp4_processor::VideoFrameType::Frame {
                            data,
                            is_sync: is_keyframe,
                        })
                        .is_err()
                    {
                        return Err("MP4 processor channel closed".to_string());
                    }
                }
                Ok(EncodedFrame::End) => break,
                Ok(_) => {}
                Err(e) => return Err(format!("Failed to encode frame: {e}")),
            }
        }
    }

    let sender_clone = video_sender.clone();
    encoder
        .flush(Box::new(move |data, is_keyframe| {
            if let Err(e) = sender_clone.send(mp4m::mp4_processor::VideoFrameType::Frame {
                data,
                is_sync: is_keyframe,
            }) {
                log::warn!("Failed to send flushed data: {}", e);
            }
        }))
        .map_err(|e| format!("Failed to flush encoder: {e}"))?;

    video_sender
        .send(mp4m::mp4_processor::VideoFrameType::End)
        .map_err(|e| format!("Failed to send end signal: {e}"))?;
    drop(video_sender);

    processor_thread
        .join()
        .map_err(|_| "Processor thread error".to_string())?;

    Ok(output_path.to_path_buf())
}

/// 把效果渲染成 gif（无限循环）。帧保留 alpha 通道直接编码，
/// GifEncoder 会把 alpha=0 的像素作为透明色，实现透明背景。
fn record_to_gif(
    input: &str,
    effect_name: &str,
    render: &RenderConfig,
    loops: usize,
    output_path: &Path,
    stop_sig: &Arc<AtomicBool>,
    progress_sender: &Sender<f32>,
) -> Result<PathBuf, String> {
    // 先探测一遍：获得总帧数。
    let (frame_count, _, _) = probe_render(input, effect_name, render)?;
    if stop_sig.load(Ordering::SeqCst) {
        return Err("record stopped".to_string());
    }
    let total_frames = frame_count * loops as u32;
    let delay_ms = 1000 / render.fps.max(1) as u32;

    // GIF 编码是纯 Rust 的 256 色量化 + LZW 压缩，比 x264 慢得多；
    // 放到独立线程异步编码，渲染线程只负责渲染帧并投递，不被编码阻塞。
    let (frame_sender, frame_receiver) = crossbeam::channel::bounded::<(image::RgbaImage, u32)>(8);
    let encoder_stop = Arc::new(AtomicBool::new(false));
    let encoder_stop_thread = encoder_stop.clone();
    let output_owned = output_path.to_path_buf();
    let encoder_thread = std::thread::spawn(move || -> Result<(), String> {
        let file = std::fs::File::create(&output_owned)
            .map_err(|e| format!("Failed to create GIF file: {e}"))?;
        let mut encoder = image::codecs::gif::GifEncoder::new(file);
        encoder
            .set_repeat(image::codecs::gif::Repeat::Infinite)
            .map_err(|e| format!("Failed to set GIF repeat: {e}"))?;

        while let Ok((image, delay_ms)) = frame_receiver.recv() {
            if encoder_stop_thread.load(Ordering::SeqCst) {
                return Err("record stopped".to_string());
            }
            // 用 from_numer_denom_ms(ms, 1) 而不是 from_saturating_duration：后者生成
            // (ms, 1000) 的 ratio，GIF 编码器 to_integer() 得到 0，导致帧延迟全为 0。
            let delay = image::Delay::from_numer_denom_ms(delay_ms, 1);
            let gif_frame = image::Frame::from_parts(image, 0, 0, delay);
            encoder
                .encode_frame(gif_frame)
                .map_err(|e| format!("Failed to encode GIF frame: {e}"))?;
        }
        Ok(())
    });

    let mut encoded_frames: u32 = 0;
    let result = (|| -> Result<(), String> {
        for _ in 0..loops {
            let Some(mut effect) = EffectCommand::from_name(effect_name).map(|c| c.build_effect())
            else {
                return Err(format!("unknown effect: {effect_name}"));
            };
            let mut renderer = SequenceRenderer::new(input, render.clone())
                .map_err(|e| format!("Failed to create renderer: {e}"))?;
            renderer
                .build_effect(effect.as_mut())
                .map_err(|e| format!("Failed to build effect: {e}"))?;

            while let Some(frame) = renderer.next_frame(effect.as_mut()) {
                if stop_sig.load(Ordering::SeqCst) {
                    encoder_stop.store(true, Ordering::SeqCst);
                    return Err("record stopped".to_string());
                }
                encoded_frames += 1;
                let progress = (encoded_frames as f32 / total_frames as f32).min(0.99);
                let _ = progress_sender.try_send(progress);

                if frame_sender.send((frame.image, delay_ms)).is_err() {
                    return Err("GIF encoder channel closed".to_string());
                }
            }
        }
        Ok(())
    })();
    // 渲染结束：关闭通道，等待编码线程把队列里的帧写完。
    drop(frame_sender);
    encoder_thread
        .join()
        .map_err(|_| "GIF encoder thread panicked".to_string())??;
    result?;
    Ok(output_path.to_path_buf())
}

/// 渲染一遍动画（光栅化但不编码），返回 (总帧数, 宽, 高)。
fn probe_render(
    input: &str,
    effect_name: &str,
    render: &RenderConfig,
) -> Result<(u32, u32, u32), String> {
    let Some(mut effect) = EffectCommand::from_name(effect_name).map(|c| c.build_effect()) else {
        return Err(format!("unknown effect: {effect_name}"));
    };
    let mut renderer = SequenceRenderer::new(input, render.clone())
        .map_err(|e| format!("Failed to create renderer: {e}"))?;
    renderer
        .build_effect(effect.as_mut())
        .map_err(|e| format!("Failed to build effect: {e}"))?;

    let mut frame_count = 0;
    let mut dims = (0, 0);
    while let Some(frame) = renderer.next_frame(effect.as_mut()) {
        dims = (frame.image.width(), frame.image.height());
        frame_count += 1;
    }
    if frame_count == 0 {
        return Err("effect produced no frames".to_string());
    }
    Ok((frame_count, dims.0, dims.1))
}

/// RGBA 帧转 RGB，必要时用背景色把尺寸补齐为偶数。
fn frame_to_rgb(
    rgba: image::RgbaImage,
    width: u32,
    height: u32,
    need_pad: bool,
    background: (u8, u8, u8),
) -> image::ImageBuffer<image::Rgb<u8>, Vec<u8>> {
    let (src_w, src_h) = rgba.dimensions();
    if !need_pad {
        let mut rgb = Vec::with_capacity((src_w * src_h * 3) as usize);
        for px in rgba.pixels() {
            rgb.extend_from_slice(&[px[0], px[1], px[2]]);
        }
        return image::ImageBuffer::from_raw(src_w, src_h, rgb)
            .expect("create rgb buffer from raw");
    }

    let mut rgb = Vec::with_capacity((width * height * 3) as usize);
    for y in 0..height {
        for x in 0..width {
            if x < src_w && y < src_h {
                let px = rgba.get_pixel(x, y);
                rgb.extend_from_slice(&[px[0], px[1], px[2]]);
            } else {
                rgb.extend_from_slice(&[background.0, background.1, background.2]);
            }
        }
    }
    image::ImageBuffer::from_raw(width, height, rgb).expect("create rgb buffer from raw")
}
