use eframe::egui;
use image::RgbaImage;
use std::{path::PathBuf, sync::Arc, time::Duration};
use video_editor::{
    metadata::get_metadata,
    tracks::{
        image_track::ImageTrack,
        manager::Manager,
        segment::Segment,
        track::{InnerTrack, Track},
        video_track::{UnifiedVideoTracksCompositorIterator, VideoTrack},
    },
};

struct VideoViewerApp {
    manager: Manager,
    frame_iter: Option<UnifiedVideoTracksCompositorIterator>,
    current_frame: Option<RgbaImage>,
    current_frame_index: usize,
    fps: f32,
    output_width: u32,
    output_height: u32,
    is_playing: bool,
    is_finished: bool,
    start_timeline_offset: Duration,
}

impl VideoViewerApp {
    fn new() -> Self {
        let mut manager = Manager::new();

        // ===== 添加 Image 轨道: 图片 (最上层) =====
        let image_path = PathBuf::from("data").join("test.png");
        if !image_path.exists() {
            log::warn!(
                "Image file not found: {}, using default metadata",
                image_path.display()
            );
        }

        let image_duration = Duration::from_secs(3);
        let image_metadata = Arc::new(get_metadata(&image_path).unwrap());

        let image_segment = Arc::new(Segment::new(
            Duration::from_secs_f64(0.5), // 从 0.5 秒开始
            image_duration,
            image_metadata.clone(),
            1.0,
        ));

        let image_track = ImageTrack::new(
            image_metadata,
            Duration::from_secs_f64(0.5) + image_duration,
            vec![image_segment],
        );

        manager.add_track(Track::Image(Arc::new(image_track)));
        log::info!(
            "Added image track: 'test.png' (0.5-3.5s) [Layer 0 - Top]"
        );

        // ===== 添加第一个视频轨道: test.mkv (第二层) =====
        let mkv_path = PathBuf::from("data").join("test.mkv");
        log::info!("Loading MKV video from: {}", mkv_path.display());

        let mkv_metadata = match get_metadata(&mkv_path) {
            Ok(meta) => Arc::new(meta),
            Err(e) => panic!("Failed to get MKV metadata: {:?}", e),
        };

        if mkv_metadata.videos.is_empty() {
            panic!("No video tracks found in MKV file");
        }

        let mkv_video_meta = &mkv_metadata.videos[0];
        log::info!("MKV video track info:");
        log::info!(
            "  Resolution: {}x{}",
            mkv_video_meta.width,
            mkv_video_meta.height
        );
        log::info!("  FPS: {}", mkv_video_meta.fps);
        log::info!(
            "  Total Duration: {:.2}s",
            mkv_metadata.duration.as_secs_f64()
        );

        let mkv_segment = Arc::new(Segment::new(
            Duration::ZERO,
            mkv_metadata.duration,
            mkv_metadata.clone(),
            1.0,
        ));

        let mkv_inner_track = InnerTrack::new(mkv_metadata.clone(), mkv_metadata.duration, vec![mkv_segment]);

        let mkv_video_track = VideoTrack {
            name: "MKV Video Track".to_string(),
            hiding: false,
            muted: false,
            locked: false,
            track: mkv_inner_track,
        };

        manager.add_track(Track::Video(Arc::new(mkv_video_track)));
        log::info!("Added MKV video track: test.mkv [Layer 1]");

        // ===== 添加第二个视频轨道: test.mp4 (最底层) =====
        let file_path = PathBuf::from("data").join("test.mp4");
        log::info!("Loading MP4 video from: {}", file_path.display());

        let metadata = match get_metadata(&file_path) {
            Ok(meta) => Arc::new(meta),
            Err(e) => panic!("Failed to get MP4 metadata: {:?}", e),
        };

        if metadata.videos.is_empty() {
            panic!("No video tracks found in MP4 file");
        }

        let video_meta = &metadata.videos[0];
        let fps = video_meta.fps;
        log::info!("MP4 video track info:");
        log::info!("  Resolution: {}x{}", video_meta.width, video_meta.height);
        log::info!("  FPS: {}", fps);
        log::info!("  Total Duration: {:.2}s", metadata.duration.as_secs_f64());

        let segment_timeline_offset = Duration::from_secs(2);
        let source_offset = Duration::from_secs(1);
        let segment_duration = metadata.duration - source_offset;

        let segment = Arc::new(Segment::new_with_source_offset(
            segment_timeline_offset,
            source_offset,
            segment_duration,
            1.0,
            1.0,
            metadata.clone(),
        ));

        let inner_track = InnerTrack::new(metadata.clone(), segment_timeline_offset + segment_duration, vec![segment]);

        let video_track = VideoTrack {
            name: "MP4 Video Track".to_string(),
            hiding: false,
            muted: false,
            locked: false,
            track: inner_track,
        };

        manager.add_track(Track::Video(Arc::new(video_track)));
        log::info!("Added MP4 video track: test.mp4 [Layer 2 - Bottom]");

        let output_width = video_meta.width;
        let output_height = video_meta.height;
        let start_timeline_offset = Duration::from_secs(1);

        // 创建迭代器并预加载缓存（在后台线程中执行）
        // 这样当用户点击播放按钮时，已经有部分帧在缓存中了
        let frame_iter = Some(Self::create_iterator_with_manager(
            &manager,
            start_timeline_offset,
            output_width,
            output_height,
            fps,
        ));

        Self {
            manager,
            frame_iter,
            current_frame: None,
            current_frame_index: 0,
            fps,
            output_width,
            output_height,
            is_playing: false,
            is_finished: false,
            start_timeline_offset,
        }
    }

    fn create_iterator_with_manager(
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

    fn reset(&mut self) {
        // 重新创建迭代器并预加载缓存
        self.frame_iter = Some(Self::create_iterator_with_manager(
            &self.manager,
            self.start_timeline_offset,
            self.output_width,
            self.output_height,
            self.fps,
        ));
        self.current_frame = None;
        self.current_frame_index = 0;
        self.is_playing = false;
        self.is_finished = false;
    }
}

impl eframe::App for VideoViewerApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // 只在播放状态时才获取下一帧
        if self.is_playing {
            // 获取下一帧
            if let Some(iter) = &mut self.frame_iter {
                if let Some(layer_frames) = iter.next() {
                    self.current_frame = Some(layer_frames.composited_image);
                    self.current_frame_index += 1;
                    // 请求下一帧的重绘
                    ui.ctx().request_repaint_after(Duration::from_secs_f64(1.0 / self.fps as f64));
                } else {
                    // 播放结束
                    self.is_playing = false;
                    self.is_finished = true;
                }
            }
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
                    ui.heading("Video Track Viewer");
                    ui.add_space(20.0);
                    ui.label("Click ▶ to start playback");
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

        // 悬浮控制面板 - 在主上下文中绘制
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

                        // 播放/暂停按钮 - 白色大按钮
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
                                self.is_playing = true;
                            } else if self.is_playing {
                                // 暂停
                                self.is_playing = false;
                            } else {
                                // 开始播放
                                self.is_playing = true;
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
                                // 停止并重置到开头
                                self.reset();
                            }
                        }

                        ui.separator();

                        // 时间信息 - 白色粗体
                        ui.label(
                            egui::RichText::new(format!(
                                "{:.1}s / {:.1}s",
                                self.start_timeline_offset.as_secs_f32()
                                    + self.current_frame_index as f32 / self.fps,
                                self.manager.duration.as_secs_f32(),
                            ))
                            .color(egui::Color32::WHITE)
                            .size(24.0)
                            .strong(),
                        );

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
            .with_title("Video Track Viewer"),
        ..Default::default()
    };

    eframe::run_native(
        "Video Track Viewer",
        options,
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::new(VideoViewerApp::new()))
        }),
    )
}