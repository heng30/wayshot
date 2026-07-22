use eframe::egui;
use image::RgbaImage;
use portable_atomic::AtomicF32;
use rodio::{DeviceSinkBuilder, MixerDeviceSink, Player, buffer::SamplesBuffer};
use std::{
    num::NonZero,
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
    sync::{Arc, Mutex},
    time::Duration,
};

type AudioSink = Player;
use video_editor::{
    filters::{
        audio::GainFilter,
        subtitle::style::{
            alignment::AlignmentFilter,
            border::OutlineWidthFilter,
            colors::{BackgroundColorFilter, OutlineColorFilter, PrimaryColorFilter},
            font_path::FontPathFilter,
            font_size::FontSizeFilter,
            padding::PaddingFilter,
        },
    },
    metadata::get_metadata,
    tracks::{
        audio_track::{AudioTrack, UnifiedAudioTracksMixerIterator},
        manager::Manager,
        segment::Segment,
        subtitle_track::{UnifiedSubtitleTracksCompositorIterator, apply_segment_subtitle_filters},
        track::{InnerTrack, Track},
        video_track::{UnifiedVideoTracksCompositorIterator, VideoTrack},
    },
};

struct AVPlayerApp {
    manager: Manager,
    frame_iter: Option<UnifiedVideoTracksCompositorIterator>,
    mixer_iter: Option<UnifiedAudioTracksMixerIterator>,
    subtitle_iter: Option<UnifiedSubtitleTracksCompositorIterator>,
    current_frame: Option<RgbaImage>,
    current_frame_index: usize,
    fps: f32,
    output_width: u32,
    output_height: u32,
    is_playing: bool,
    is_finished: bool,
    start_timeline_offset: Duration,
    audio_sink: Option<Arc<Mutex<AudioSink>>>,
    audio_stream: Option<MixerDeviceSink>,
    volume: Arc<AtomicF32>,
    stop_signal: Arc<AtomicBool>,
    playback_start_time: Option<std::time::Instant>,
    paused_time: Duration,
}

impl AVPlayerApp {
    fn new() -> Self {
        let mut manager = Manager::new();

        // === 加载视频轨道 ===
        let video_path = PathBuf::from("data").join("test.mp4");
        log::info!("Loading video from: {}", video_path.display());

        let video_metadata = match get_metadata(&video_path) {
            Ok(meta) => Arc::new(meta),
            Err(e) => panic!("Failed to get video metadata: {:?}", e),
        };

        if video_metadata.videos.is_empty() {
            panic!("No video tracks found in video file");
        }

        let video_meta = &video_metadata.videos[0];
        let fps = video_meta.fps;
        log::info!("Video track info:");
        log::info!("  Resolution: {}x{}", video_meta.width, video_meta.height);
        log::info!("  FPS: {}", fps);
        log::info!(
            "  Total Duration: {:.2}s",
            video_metadata.duration.as_secs_f64()
        );

        let video_segment = Arc::new(Segment::new(
            Duration::from_secs(3),
            video_metadata.duration,
            video_metadata.clone(),
            1.0,
        ));

        let video_segment2 = Arc::new(Segment::new(
            video_metadata.duration + Duration::from_secs(5),
            video_metadata.duration,
            video_metadata.clone(),
            1.0,
        ));

        let video_inner_track = InnerTrack::new(
            video_metadata.clone(),
            video_metadata.duration + video_metadata.duration + Duration::from_secs(5),
            vec![video_segment, video_segment2],
        );

        let video_track = VideoTrack::new(video_inner_track);

        manager.add_track(Track::Video(Arc::new(video_track)));

        // === 加载音频轨道 ===
        let audio_files = vec![
            ("data/test.mp4", -3.0), // MP4 with -3dB gain
            ("data/test.wav", 0.0),  // WAV with normal volume
        ];

        for (file_path, gain_db) in &audio_files {
            let file_path = PathBuf::from(file_path);
            log::info!("Loading audio from: {}", file_path.display());

            let audio_metadata = match get_metadata(&file_path) {
                Ok(meta) => Arc::new(meta),
                Err(e) => {
                    log::warn!("Failed to load {}: {:?}", file_path.display(), e);
                    continue;
                }
            };

            if audio_metadata.audios.is_empty() {
                log::warn!("No audio tracks found in {}", file_path.display());
                continue;
            }

            let audio_meta = &audio_metadata.audios[0];
            log::info!("  Sample Rate: {} Hz", audio_meta.sample_rate);
            log::info!("  Channels: {}", audio_meta.channels);
            log::info!("  Duration: {:.2}s", audio_metadata.duration.as_secs_f64());

            // Create segment with gain filter
            let mut audio_segment = Segment::new(
                Duration::ZERO,
                audio_metadata.duration,
                audio_metadata.clone(),
                1.0,
            );

            let gain_filter = GainFilter::from_db(*gain_db);
            audio_segment.add_audio_filter(Box::new(gain_filter));
            log::info!("  Applied gain filter: {} dB", gain_db);

            let audio_inner_track = InnerTrack::new(
                audio_metadata.clone(),
                audio_metadata.duration,
                vec![Arc::new(audio_segment)],
            );

            let audio_track = AudioTrack {
                name: "".to_string(),
                hiding: false,
                locked: false,
                track: audio_inner_track,
            };

            manager.add_track(Track::Audio(Arc::new(audio_track)));
        }

        // === 加载字幕轨道 ===
        let subtitle_path = PathBuf::from("data").join("test.srt");
        log::info!("Loading subtitle from: {}", subtitle_path.display());

        if subtitle_path.exists() {
            match Track::new(&subtitle_path, 1.0) {
                Ok(subtitle_tracks) => {
                    for mut subtitle_track in subtitle_tracks {
                        if let Track::Subtitle(ref mut arc_subtitle_track) = subtitle_track {
                            let subtitle_track = Arc::make_mut(arc_subtitle_track);

                            // Apply subtitle filters to all segments
                            for segment in &mut subtitle_track.track.segments {
                                let segment_mut = Arc::make_mut(segment);

                                // 字体设置 - 使用 SourceHanSansCN
                                let font_path =
                                    PathBuf::from("../../wayshot/ui/fonts/SourceHanSansCN.otf");
                                if font_path.exists() {
                                    segment_mut.add_subtitle_filter(Box::new(FontPathFilter::new(
                                        font_path,
                                        "SourceHanSansCN".to_string(),
                                        String::new(),
                                    )));
                                } else {
                                    log::warn!("Font file not found: {}", font_path.display());
                                }

                                // 字号: 30px
                                segment_mut.add_subtitle_filter(Box::new(FontSizeFilter::new(30)));

                                // 对齐: 顶部居中 (alignment = 8)
                                segment_mut
                                    .add_subtitle_filter(Box::new(AlignmentFilter::top_center()));

                                // 半透明黄色背景 (RGBA: 255, 255, 0, 180)
                                segment_mut.add_subtitle_filter(Box::new(
                                    BackgroundColorFilter::from_rgba(255, 255, 0, 180),
                                ));

                                // 文字颜色: 白色
                                segment_mut.add_subtitle_filter(Box::new(
                                    PrimaryColorFilter::from_rgba(255, 255, 255, 255),
                                ));

                                // 描边颜色: 黑色
                                segment_mut.add_subtitle_filter(Box::new(
                                    OutlineColorFilter::from_rgba(0, 0, 0, 255),
                                ));

                                // 描边宽度: 2px
                                segment_mut.add_subtitle_filter(Box::new(OutlineWidthFilter::new(
                                    2,
                                )));

                                // 内边距: 8px
                                segment_mut
                                    .add_subtitle_filter(Box::new(PaddingFilter::new(8)));
                            }

                            log::info!(
                                "  Loaded {} subtitle entries",
                                subtitle_track.get_subtitle_entries().len()
                            );

                            manager.add_track(Track::Subtitle(Arc::clone(arc_subtitle_track)));
                        }
                    }
                }
                Err(e) => {
                    log::warn!("Failed to load subtitle file: {:?}", e);
                }
            }
        } else {
            log::warn!("Subtitle file not found: {}", subtitle_path.display());
        }

        let output_width = video_meta.width;
        let output_height = video_meta.height;
        let start_timeline_offset = Duration::from_secs(1);

        // 创建视频迭代器
        let frame_iter = Some(Self::create_video_iterator(
            &manager,
            start_timeline_offset,
            output_width,
            output_height,
            fps,
        ));

        // 创建音频迭代器
        let mixer_iter = Some(Self::create_audio_mixer_iter(
            &manager,
            start_timeline_offset,
        ));

        // 创建字幕迭代器
        let subtitle_iter = Self::create_subtitle_iterator(&manager, start_timeline_offset);

        let volume = Arc::new(AtomicF32::new(0.1));
        let stop_signal = Arc::new(AtomicBool::new(false));

        let mut app = Self {
            manager,
            frame_iter,
            mixer_iter,
            subtitle_iter,
            current_frame: None,
            current_frame_index: 0,
            fps,
            output_width,
            output_height,
            is_playing: false,
            is_finished: false,
            start_timeline_offset,
            audio_sink: None,
            audio_stream: None,
            volume,
            stop_signal,
            playback_start_time: None,
            paused_time: Duration::ZERO,
        };

        // Auto-start playback on launch
        let _ = app.start_audio_playback();
        app.is_playing = true;

        app
    }

    fn create_video_iterator(
        manager: &Manager,
        start_timeline_offset: Duration,
        output_width: u32,
        output_height: u32,
        fps: f32,
    ) -> UnifiedVideoTracksCompositorIterator {
        manager
            .unified_video_tracks_compositor_iter(
                start_timeline_offset,
                Duration::from_secs_f64(3.0),
                Duration::from_secs(8),
                Some(output_width),
                Some(output_height),
                Some(fps),
            )
            .unwrap()
    }

    fn create_audio_mixer_iter(
        manager: &Manager,
        start_timestamp: Duration,
    ) -> UnifiedAudioTracksMixerIterator {
        manager
            .unified_audio_tracks_mixer_iter(
                start_timestamp,
                Duration::from_secs(3),
                Duration::from_secs(10),
                Duration::from_secs(1),
                None, // output_channels: auto-detect
                None, // output_sample_rate: auto-detect
            )
            .unwrap()
    }

    fn create_subtitle_iterator(
        manager: &Manager,
        start_timeline_offset: Duration,
    ) -> Option<UnifiedSubtitleTracksCompositorIterator> {
        manager
            .unified_subtitle_tracks_compositor_iter(start_timeline_offset)
            .ok()
    }

    fn reset(&mut self) {
        // 重置停止信号
        self.stop_signal.store(false, Ordering::Relaxed);

        // 重新创建视频迭代器
        self.frame_iter = Some(Self::create_video_iterator(
            &self.manager,
            self.start_timeline_offset,
            self.output_width,
            self.output_height,
            self.fps,
        ));

        // 重新创建音频迭代器
        self.mixer_iter = Some(Self::create_audio_mixer_iter(
            &self.manager,
            self.start_timeline_offset,
        ));

        // 重新创建字幕迭代器
        self.subtitle_iter =
            Self::create_subtitle_iterator(&self.manager, self.start_timeline_offset);

        self.current_frame = None;
        self.current_frame_index = 0;
        self.is_playing = false;
        self.is_finished = false;
        self.playback_start_time = None;
        self.paused_time = Duration::ZERO;
    }

    fn start_audio_playback(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.mixer_iter.is_none() {
            return Ok(());
        }

        let device_sink = DeviceSinkBuilder::open_default_sink()?;
        let sink = AudioSink::connect_new(&device_sink.mixer());

        // 设置初始音量
        let initial_volume = self.volume.load(Ordering::Relaxed).max(0.0);
        sink.set_volume(initial_volume);

        // 保存 sink 的 Arc 包装，用于音量控制
        let sink_arc = Arc::new(Mutex::new(sink));
        self.audio_sink = Some(sink_arc.clone());

        let mixer_iter = self.mixer_iter.take().unwrap();
        let volume = self.volume.clone();
        let stop_signal = self.stop_signal.clone();

        // 在后台线程中处理音频播放
        std::thread::spawn(move || {
            if let Err(e) =
                Self::audio_worker_with_shared_sink(mixer_iter, sink_arc, volume, stop_signal)
            {
                log::error!("Audio playback error: {:?}", e);
            }
        });

        self.audio_stream = Some(device_sink);
        Ok(())
    }

    fn audio_worker_with_shared_sink(
        mixer_iter: UnifiedAudioTracksMixerIterator,
        sink_arc: Arc<Mutex<AudioSink>>,
        volume: Arc<AtomicF32>,
        stop_sig: Arc<AtomicBool>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut current_iter = mixer_iter;

        loop {
            if stop_sig.load(Ordering::Relaxed) {
                if let Ok(sink) = sink_arc.try_lock() {
                    sink.stop();
                }
                break;
            }

            match current_iter.next() {
                Some(audio_data) if !audio_data.samples.is_empty() => {
                    if let Ok(sink) = sink_arc.try_lock() {
                        sink.set_speed(1.0);
                        sink.set_volume(volume.load(Ordering::Relaxed).max(0.0));

                        let channels = NonZero::new(audio_data.channels)
                            .ok_or("Audio channels must be non-zero")?;
                        let sample_rate = NonZero::new(audio_data.sample_rate)
                            .ok_or("Audio sample rate must be non-zero")?;
                        let source = SamplesBuffer::new(channels, sample_rate, audio_data.samples);
                        sink.append(source);

                        if sink.len() > 3 {
                            while !sink.empty() {
                                sink.set_speed(1.0);
                                sink.set_volume(volume.load(Ordering::Relaxed).max(0.0));

                                if stop_sig.load(Ordering::Relaxed) {
                                    sink.stop();
                                    return Ok(());
                                }
                                std::thread::sleep(Duration::from_millis(10));
                            }
                        }
                    }
                }
                Some(_) => continue,
                None => break,
            }
        }

        if let Ok(sink) = sink_arc.try_lock() {
            while !sink.empty() {
                sink.set_speed(1.0);
                sink.set_volume(volume.load(Ordering::Relaxed).max(0.0));

                if stop_sig.load(Ordering::Relaxed) {
                    sink.stop();
                    return Ok(());
                }
                std::thread::sleep(Duration::from_millis(100));
            }

            sink.sleep_until_end();
        }

        Ok(())
    }

    fn stop_audio(&mut self) {
        self.stop_signal.store(true, Ordering::Relaxed);
        self.audio_sink = None;
        self.audio_stream = None;
    }

    fn get_current_playback_time(&self) -> Duration {
        if let Some(start_time) = self.playback_start_time {
            let elapsed = start_time.elapsed();
            let total_time = self.start_timeline_offset + elapsed;
            std::cmp::min(total_time, self.manager.duration)
        } else {
            self.start_timeline_offset
                + Duration::from_secs_f64(self.current_frame_index as f64 / self.fps as f64)
        }
    }

    fn render_subtitle_to_frame(&mut self) {
        if self.current_frame.is_none() {
            return;
        }

        let current_time = self.get_current_playback_time();

        // 使用 subtitle_iter 的 get_subtitle_at 方法
        if let Some(iter) = &self.subtitle_iter {
            if let Some(subtitle) = iter.get_subtitle_at(current_time) {
                log::info!(
                    "Rendering subtitle at {:.2}s: '{}'",
                    current_time.as_secs_f64(),
                    subtitle.subtitle.text
                );
                if let Some(frame) = &mut self.current_frame {
                    match apply_segment_subtitle_filters(frame, &subtitle.subtitle, subtitle.segment) {
                        Ok(()) => log::debug!("Subtitle rendered successfully"),
                        Err(e) => log::error!("Failed to render subtitle: {:?}", e),
                    }
                }
            }
        }
    }
}

impl Drop for AVPlayerApp {
    fn drop(&mut self) {
        self.stop_audio();
    }
}

impl eframe::App for AVPlayerApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // 首次播放时记录开始时间
        if self.is_playing {
            if self.playback_start_time.is_none() {
                self.playback_start_time = Some(std::time::Instant::now());
            }

            // 根据实际经过时间计算应该显示的帧索引
            if let Some(start_time) = self.playback_start_time {
                let elapsed = start_time.elapsed();
                let target_frame_index = (elapsed.as_secs_f64() * self.fps as f64) as usize + 1;

                // 获取帧直到达到目标帧索引
                let mut new_frame = None;

                if let Some(iter) = &mut self.frame_iter {
                    while self.current_frame_index < target_frame_index {
                        if let Some(layer_frames) = iter.next() {
                            new_frame = Some(layer_frames.composited_image);
                            self.current_frame_index += 1;
                        } else {
                            // 视频播放结束
                            self.is_playing = false;
                            self.is_finished = true;
                            break;
                        }
                    }
                }

                // 更新当前帧并渲染字幕
                if new_frame.is_some() {
                    self.current_frame = new_frame;
                    if self.is_playing {
                        self.render_subtitle_to_frame();
                    }
                }
            }

            // 请求下一帧刷新
            ui.ctx().request_repaint_after(Duration::from_secs_f64(1.0 / self.fps as f64));
        }

        // 如果已结束（视频播放完），继续更新UI以显示音频播放进度
        if self.is_finished && self.playback_start_time.is_some() {
            ui.ctx().request_repaint_after(Duration::from_millis(100));
        }

        // 显示视频帧
        egui::CentralPanel::default().show_inside(ui, |ui: &mut egui::Ui| {
            // 获取可用空间
            let available_size = ui.available_size();

            // 显示当前帧或占位符
            let current_frame = if let Some(ref frame) = self.current_frame {
                frame
            } else {
                // 没有帧时显示占位符
                ui.vertical_centered(|ui| {
                    ui.add_space(available_size.y * 0.4);
                    ui.heading("Audio-Video Player");
                    ui.add_space(20.0);
                    ui.label("Click ▶ to start playback");
                    ui.label("Loads both video and audio tracks");
                });
                return;
            };

            let frame_aspect = current_frame.width() as f32 / current_frame.height() as f32;

            let display_size = if available_size.x / available_size.y > frame_aspect {
                let height = available_size.y;
                let width = height * frame_aspect;
                egui::vec2(width, height)
            } else {
                let width = available_size.x;
                let height = width / frame_aspect;
                egui::vec2(width, height)
            };

            // 转换为纹理
            let size = [
                current_frame.width() as usize,
                current_frame.height() as usize,
            ];
            let color_image = egui::ColorImage::from_rgba_unmultiplied(
                size,
                current_frame.as_flat_samples().as_slice(),
            );

            let ctx = ui.ctx();
            let texture_handle =
                ctx.load_texture("current_frame", color_image, egui::TextureOptions::LINEAR);

            // 居中显示图像
            ui.centered_and_justified(|ui: &mut egui::Ui| {
                ui.image((texture_handle.id(), display_size));
            });
        });

        // 悬浮控制面板
        egui::Area::new(egui::Id::new("control_panel"))
            .anchor(egui::Align2::CENTER_BOTTOM, [0.0, -10.0])
            .show(ui.ctx(), |ui| {
                ui.vertical_centered(|ui| {
                    // 半透明黑色背景
                    let panel_rect = ui.max_rect();
                    ui.painter().rect_filled(
                        panel_rect,
                        egui::CornerRadius::same(12),
                        egui::Color32::from_rgba_premultiplied(0, 0, 0, 200),
                    );

                    ui.add_space(15.0);

                    // 控制按钮行
                    ui.horizontal(|ui: &mut egui::Ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(25.0, 10.0);

                        // 播放/暂停按钮
                        let play_button_text = if self.is_finished {
                            "↺" // Replay
                        } else if self.is_playing {
                            "⏸" // Pause
                        } else {
                            "▶" // Play
                        };

                        let play_btn = egui::Button::new(
                            egui::RichText::new(play_button_text)
                                .size(40.0)
                                .color(egui::Color32::BLACK),
                        )
                        .fill(egui::Color32::WHITE)
                        .stroke(egui::Stroke::new(
                            2.0,
                            egui::Color32::from_rgb(200, 200, 200),
                        ))
                        .min_size(egui::vec2(90.0, 90.0));

                        if ui.add_sized([90.0, 90.0], play_btn).clicked() {
                            if self.is_finished {
                                // 重播：重置状态并开始播放
                                self.reset();
                                let _ = self.start_audio_playback();
                                self.is_playing = true;
                            } else if self.is_playing {
                                // 暂停：保存当前播放时间
                                if self.playback_start_time.is_some() {
                                    self.paused_time = self.playback_start_time.unwrap().elapsed();
                                    self.playback_start_time = None;
                                }
                                self.is_playing = false;
                                self.stop_audio();
                            } else {
                                // 开始/继续播放
                                let _ = self.start_audio_playback();
                                self.is_playing = true;
                                // 如果有暂停时间，从这里开始
                                if self.paused_time > Duration::ZERO {
                                    self.playback_start_time =
                                        Some(std::time::Instant::now() - self.paused_time);
                                }
                            }
                        }

                        // 停止按钮
                        let stop_btn = egui::Button::new(
                            egui::RichText::new("⏹")
                                .size(35.0)
                                .color(egui::Color32::BLACK),
                        )
                        .fill(egui::Color32::WHITE)
                        .stroke(egui::Stroke::new(
                            2.0,
                            egui::Color32::from_rgb(200, 200, 200),
                        ))
                        .min_size(egui::vec2(80.0, 80.0));

                        if ui.add_sized([80.0, 80.0], stop_btn).clicked() {
                            if self.is_playing || self.current_frame.is_some() {
                                self.stop_audio();
                                self.reset();
                            }
                        }

                        ui.separator();

                        // 音量控制
                        ui.vertical(|ui| {
                            ui.label(
                                egui::RichText::new("Volume")
                                    .color(egui::Color32::WHITE)
                                    .size(14.0),
                            );
                            ui.add_space(5.0);
                            let mut vol = self.volume.load(Ordering::Relaxed);
                            let slider_response =
                                ui.add(egui::Slider::new(&mut vol, 0.0..=1.0).show_value(false));

                            // 如果滑块被拖动或改变，更新音量
                            if slider_response.changed() || slider_response.dragged() {
                                self.volume.store(vol, Ordering::Relaxed);
                            }
                        });

                        ui.separator();

                        // 时间信息
                        let current_time = self.get_current_playback_time();
                        ui.label(
                            egui::RichText::new(format!(
                                "{:.1}s / {:.1}s",
                                current_time.as_secs_f32(),
                                self.manager.duration.as_secs_f32(),
                            ))
                            .color(egui::Color32::WHITE)
                            .size(24.0)
                            .strong(),
                        );

                        // 帧信息
                        ui.vertical(|ui| {
                            ui.label(
                                egui::RichText::new(format!("Frame: {}", self.current_frame_index))
                                    .color(egui::Color32::WHITE)
                                    .size(16.0),
                            );
                            ui.label(
                                egui::RichText::new(format!("FPS: {}", self.fps))
                                    .color(egui::Color32::WHITE)
                                    .size(16.0),
                            );
                        });

                        ui.add_space(15.0);
                    });

                    ui.add_space(15.0);
                });
            });
    }
}

fn main() -> Result<(), eframe::Error> {
    env_logger::init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 720.0])
            .with_title("Audio-Video Player"),
        ..Default::default()
    };

    eframe::run_native(
        "Audio-Video Player",
        options,
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::new(AVPlayerApp::new()))
        }),
    )
}
