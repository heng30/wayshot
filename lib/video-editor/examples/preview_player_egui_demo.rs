//! Preview Player Demo using egui
//!
//! Demonstrates how to integrate the PreviewRenderer with an egui-based GUI
//! for real-time video playback with audio and subtitle support.

use eframe::egui;
use image::RgbaImage;
use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use video_editor::{
    filters::subtitle::style::{
        alignment::AlignmentFilter,
        colors::{BackgroundColorFilter, OutlineColorFilter, PrimaryColorFilter},
        font_path::FontPathFilter,
        font_size::FontSizeFilter,
        border::OutlineWidthFilter,
    },
    metadata::get_metadata,
    preview::{PreviewConfig, PreviewRenderer},
    tracks::{
        audio_track::AudioTrack, segment::Segment, track::InnerTrack, Track,
        video_track::VideoTrack,
    },
};

struct PreviewPlayerApp {
    renderer: PreviewRenderer,

    // UI state
    current_frame: Option<RgbaImage>,
    current_texture: Option<egui::TextureHandle>,
    last_update_time: Instant,
    frame_count: usize,

    // Display settings
    show_info_panel: bool,
}

impl PreviewPlayerApp {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let mut manager = video_editor::tracks::Manager::new();

        // === Load Video Track ===
        let video_path = PathBuf::from("data/test.mp4");
        log::info!("Loading video from: {}", video_path.display());

        if video_path.exists() {
            let video_metadata = match get_metadata(&video_path) {
                Ok(meta) => Arc::new(meta),
                Err(e) => {
                    log::warn!("Failed to load video: {:?}", e);
                    return Err(e.into());
                }
            };

            if video_metadata.videos.is_empty() {
                log::warn!("No video tracks found");
                return Err("No video tracks found".into());
            }

            let video_meta = &video_metadata.videos[0];
            log::info!("Video: {}x{} @ {:.2}fps", video_meta.width, video_meta.height, video_meta.fps);

            let video_segment = Arc::new(Segment::new(
                Duration::ZERO,
                video_metadata.duration,
                video_metadata.clone(),
                1.0,
            ));

            let video_inner_track = InnerTrack::new(video_metadata.clone(), video_metadata.duration, vec![video_segment]);

            let video_track = VideoTrack {
                name: "Video Track".to_string(),
                hiding: false,
                muted: false,
                locked: false,
                track: video_inner_track,
            };

            manager.add_track(Track::Video(Arc::new(video_track)));
        } else {
            log::warn!("Video file not found: {}", video_path.display());
            return Err("Video file not found".into());
        }

        // === Load Audio Track ===
        let audio_path = PathBuf::from("data/test.wav");
        log::info!("Loading audio from: {}", audio_path.display());

        if audio_path.exists() {
            if let Ok(audio_metadata) = get_metadata(&audio_path) {
                let audio_metadata = Arc::new(audio_metadata);
                if !audio_metadata.audios.is_empty() {
                    let audio_meta = &audio_metadata.audios[0];
                    log::info!("Audio: {} channels @ {} Hz", audio_meta.channels, audio_meta.sample_rate);

                    let audio_segment = Arc::new(Segment::new(
                        Duration::ZERO,
                        audio_metadata.duration,
                        audio_metadata.clone(),
                        1.0,
                    ));

                    let audio_inner_track = InnerTrack::new(audio_metadata.clone(), audio_metadata.duration, vec![audio_segment]);

                    let audio_track = AudioTrack {
                        name: "Audio Track".to_string(),
                        hiding: false,
                        locked: false,
                        track: audio_inner_track,
                    };

                    manager.add_track(Track::Audio(Arc::new(audio_track)));
                }
            }
        } else {
            log::warn!("Audio file not found: {}", audio_path.display());
        }

        // === Load Subtitle Track ===
        let subtitle_path = PathBuf::from("data/test.srt");
        log::info!("Loading subtitle from: {}", subtitle_path.display());

        if subtitle_path.exists() {
            match Track::new(&subtitle_path, 1.0) {
                Ok(subtitle_tracks) => {
                    for subtitle_track in subtitle_tracks {
                        if let Track::Subtitle(st) = subtitle_track {
                            // Apply subtitle filters to all segments
                            let font_path = PathBuf::from("../../wayshot/ui/fonts/SourceHanSansCN.otf");
                            let mut st_mut = (*st).clone();

                            for segment in &mut st_mut.track.segments {
                                let segment_mut = Arc::make_mut(segment);

                                // Font path
                                if font_path.exists() {
                                    log::info!("Using font: {}", font_path.display());
                                    segment_mut.add_subtitle_filter(Box::new(FontPathFilter::new(font_path.clone(), "SourceHanSansCN".to_string(), String::new())));
                                } else {
                                    log::warn!("Font file not found: {}", font_path.display());
                                }

                                // Font size: 30px
                                segment_mut.add_subtitle_filter(Box::new(FontSizeFilter::new(30)));

                                // Alignment: top center
                                segment_mut.add_subtitle_filter(Box::new(AlignmentFilter::top_center()));

                                // Semi-transparent yellow background
                                segment_mut.add_subtitle_filter(Box::new(
                                    BackgroundColorFilter::from_rgba(255, 255, 0, 180),
                                ));

                                // White text color
                                segment_mut.add_subtitle_filter(Box::new(
                                    PrimaryColorFilter::from_rgba(255, 255, 255, 255),
                                ));

                                // Black outline color
                                segment_mut.add_subtitle_filter(Box::new(
                                    OutlineColorFilter::from_rgba(0, 0, 0, 255),
                                ));

                                // Outline width: 2px
                                segment_mut.add_subtitle_filter(Box::new(OutlineWidthFilter::new(2)));
                            }

                            manager.add_track(Track::Subtitle(Arc::new(st_mut)));
                            log::info!("Subtitle track loaded with styles");
                        } else {
                            manager.add_track(subtitle_track);
                        }
                    }
                }
                Err(e) => {
                    log::warn!("Failed to load subtitle: {:?}. Continuing without subtitles.", e);
                }
            }
        } else {
            log::warn!("Subtitle file not found: {}", subtitle_path.display());
        }

        let manager_arc = Arc::new(manager);

        // Get video dimensions from the first video track
        let (width, height, fps) = manager_arc
            .iter()
            .filter_map(|track| match track {
                Track::Video(vt) if !vt.hiding => {
                    let v = vt.track.metadata.videos.first()?;
                    Some((v.width, v.height, v.fps))
                }
                _ => None,
            })
            .next()
            .unwrap_or((1920, 1080, 30.0));

        let mut mixer_config = video_editor::tracks::UnifiedMixerConfig::default();
        mixer_config.output_width = Some(width);
        mixer_config.output_height = Some(height);
        mixer_config.output_fps = Some(fps as f32);

        let config = PreviewConfig {
            mixer: mixer_config,
            loop_region: None,
        };

        let renderer = PreviewRenderer::new(manager_arc.clone(), config);

        Ok(Self {
            renderer,
            current_frame: None,
            current_texture: None,
            last_update_time: Instant::now(),
            frame_count: 0,
            show_info_panel: true,
        })
    }

    fn update_renderer(&mut self, ctx: &egui::Context) {
        let now = Instant::now();
        let _delta = now.duration_since(self.last_update_time);
        self.last_update_time = now;

        // Update renderer and check for new frame
        if self.renderer.update().is_ok() {
            // Take the new frame if available
            if let Some(frame) = self.renderer.take_frame() {
                self.current_frame = Some(frame);
                self.frame_count += 1;

                if self.frame_count % 30 == 0 {
                    log::info!("Playing: frame {} at {:.2}s",
                        self.frame_count,
                        self.renderer.position().as_secs_f32()
                    );
                }

                // Request repaint at frame rate
                let frame_duration = Duration::from_secs_f64(1.0 / self.renderer.frame_rate());
                ctx.request_repaint_after(frame_duration);

                // Take audio data (audio playback not implemented in this demo)
                if let Some(audio) = self.renderer.take_audio() {
                    log::trace!("Audio samples: {} channels, {} Hz",
                        audio.channels, audio.sample_rate);
                }
            }
        }

        // Request continuous repaint if playing
        if self.renderer.is_playing() {
            let frame_duration = Duration::from_secs_f64(1.0 / self.renderer.frame_rate());
            ctx.request_repaint_after(frame_duration);
        }
    }

    fn render_video_frame(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx();
        if let Some(ref frame) = self.current_frame {
            let size = [frame.width() as usize, frame.height() as usize];
            let color_image = egui::ColorImage::from_rgba_unmultiplied(
                size,
                frame.as_flat_samples().as_slice(),
            );

            let texture_handle = ctx.load_texture(
                "current_frame",
                color_image,
                egui::TextureOptions::LINEAR,
            );

            self.current_texture = Some(texture_handle);
        }

        egui::CentralPanel::default().show_inside(ui, |ui| {
            let available_size = ui.available_size();

            if let Some(ref texture) = self.current_texture {
                let texture_size = texture.size();
                let texture_size_f = [texture_size[0] as f32, texture_size[1] as f32];

                // Calculate aspect ratio
                let frame_aspect = texture_size_f[0] / texture_size_f[1];
                let available_aspect = available_size.x / available_size.y;

                let display_size = if available_aspect > frame_aspect {
                    let height = available_size.y;
                    let width = height * frame_aspect;
                    egui::vec2(width, height)
                } else {
                    let width = available_size.x;
                    let height = width / frame_aspect;
                    egui::vec2(width, height)
                };

                // Center and display the frame
                ui.centered_and_justified(|ui: &mut egui::Ui| {
                    ui.image((texture.id(), display_size));
                });
            } else {
                // Show placeholder
                ui.vertical_centered(|ui| {
                    ui.add_space(available_size.y * 0.4);
                    ui.heading(egui::RichText::new("Preview Player").size(32.0));
                    ui.add_space(20.0);
                    ui.label(egui::RichText::new("Click ▶ to start playback").size(18.0));
                    ui.label(egui::RichText::new(
                        "Uses PreviewRenderer for playback control",
                    ).size(14.0).color(egui::Color32::GRAY));
                });
            }
        });
    }

    fn render_control_panel(&mut self, ui: &mut egui::Ui) {
        let is_playing = self.renderer.is_playing();
        let is_finished = self.renderer.position() >= self.renderer.duration() && !is_playing;

        egui::Panel::top(egui::Id::new("control_panel")).show_inside(ui, |ui| {
            ui.add_space(10.0);

            // Time and progress row
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(10.0, 5.0);

                // Timecode display
                let time_text = format!(
                    "{:.1} / {:.1}s",
                    self.renderer.position().as_secs_f32(),
                    self.renderer.duration().as_secs_f32(),
                );
                ui.label(egui::RichText::new(time_text).size(20.0).strong());

                // Progress bar
                let progress = (self.renderer.progress() / 100.0) as f32;
                let progress_response =
                    ui.add(egui::ProgressBar::new(progress).show_percentage().desired_width(200.0));

                // Allow seeking by clicking on progress bar
                if progress_response.clicked() {
                    if let Some(pos) = progress_response.interact_pointer_pos() {
                        let rect = progress_response.rect;
                        let click_pos = (pos.x - rect.min.x) / rect.width();
                        let percentage = (click_pos * 100.0).clamp(0.0, 100.0) as f64;
                        let _ = self.renderer.jump_to_percentage(percentage);
                    }
                }

                // Frame info
                let frame_text = format!(
                    "Frame: {} / {}",
                    self.renderer.current_frame_number(),
                    self.renderer.total_frames().unwrap_or(0)
                );
                ui.label(egui::RichText::new(frame_text).size(16.0));
            });

            ui.add_space(10.0);

            // Control buttons row
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(15.0, 10.0);

                // Play/Pause/Stop buttons
                let play_button_text = if is_finished {
                    "↺" // Replay
                } else if is_playing {
                    "⏸" // Pause
                } else {
                    "▶" // Play
                };

                let play_btn = egui::Button::new(
                    egui::RichText::new(play_button_text).size(24.0).color(egui::Color32::BLACK),
                )
                .fill(egui::Color32::WHITE)
                .stroke(egui::Stroke::new(2.0, egui::Color32::from_rgb(200, 200, 200)))
                .min_size(egui::vec2(60.0, 60.0));

                if ui.add(play_btn).clicked() {
                    if is_finished {
                        self.renderer.stop();
                    }
                    let _ = self.renderer.toggle_playback();
                }

                // Stop button
                let stop_btn = egui::Button::new(
                    egui::RichText::new("⏹").size(20.0).color(egui::Color32::BLACK),
                )
                .fill(egui::Color32::WHITE)
                .stroke(egui::Stroke::new(2.0, egui::Color32::from_rgb(200, 200, 200)))
                .min_size(egui::vec2(50.0, 50.0));

                if ui.add(stop_btn).clicked() {
                    let _ = self.renderer.stop();
                }

                ui.separator();

                // Step backward
                let step_back_btn = egui::Button::new(
                    egui::RichText::new("⏮").size(18.0).color(egui::Color32::BLACK),
                )
                .fill(egui::Color32::WHITE)
                .stroke(egui::Stroke::new(2.0, egui::Color32::from_rgb(200, 200, 200)))
                .min_size(egui::vec2(40.0, 40.0));

                if ui.add(step_back_btn).clicked() {
                    let _ = self.renderer.step_backward();
                }

                // Step forward
                let step_fwd_btn = egui::Button::new(
                    egui::RichText::new("⏭").size(18.0).color(egui::Color32::BLACK),
                )
                .fill(egui::Color32::WHITE)
                .stroke(egui::Stroke::new(2.0, egui::Color32::from_rgb(200, 200, 200)))
                .min_size(egui::vec2(40.0, 40.0));

                if ui.add(step_fwd_btn).clicked() {
                    let _ = self.renderer.step_forward();
                }

                ui.separator();

                // Speed control
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new("Speed").size(14.0));
                    let current_speed = self.renderer.speed();
                    let speed_multiplier = current_speed.multiplier();

                    ui.horizontal(|ui| {
                        if ui
                            .add(egui::Button::new("<<").small())
                            .clicked()
                        {
                            use video_editor::preview::PlaybackSpeed;
                            let new_speed = match current_speed {
                                PlaybackSpeed::Paused => PlaybackSpeed::Paused,
                                PlaybackSpeed::Quarter => PlaybackSpeed::Paused,
                                PlaybackSpeed::Half => PlaybackSpeed::Quarter,
                                PlaybackSpeed::Normal => PlaybackSpeed::Half,
                                PlaybackSpeed::Double => PlaybackSpeed::Normal,
                                PlaybackSpeed::Quadruple => PlaybackSpeed::Double,
                            };
                            let _ = self.renderer.set_speed(new_speed);
                        }

                        ui.label(format!("{:.2}x", speed_multiplier));

                        if ui
                            .add(egui::Button::new(">>").small())
                            .clicked()
                        {
                            use video_editor::preview::PlaybackSpeed;
                            let new_speed = match current_speed {
                                PlaybackSpeed::Paused => PlaybackSpeed::Quarter,
                                PlaybackSpeed::Quarter => PlaybackSpeed::Half,
                                PlaybackSpeed::Half => PlaybackSpeed::Normal,
                                PlaybackSpeed::Normal => PlaybackSpeed::Double,
                                PlaybackSpeed::Double => PlaybackSpeed::Quadruple,
                                PlaybackSpeed::Quadruple => PlaybackSpeed::Quadruple,
                            };
                            let _ = self.renderer.set_speed(new_speed);
                        }
                    });
                });

                ui.separator();

                // Skip controls
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new("Skip").size(14.0));

                    ui.horizontal(|ui| {
                        if ui.button("-5s").clicked() {
                            let _ = self.renderer.skip_backward(5.0);
                        }

                        if ui.button("+5s").clicked() {
                            let _ = self.renderer.skip_forward(5.0);
                        }
                    });
                });
            });

            ui.add_space(5.0);
        });
    }

    fn render_info_panel(&mut self, ctx: &egui::Context) {
        egui::Window::new("Playback Info")
            .open(&mut self.show_info_panel)
            .resizable(true)
            .default_size([200.0, 150.0])
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    ui.label(format!(
                        "State: {:?}",
                        self.renderer.state()
                    ));
                    ui.label(format!(
                        "Position: {:.2}s",
                        self.renderer.position().as_secs_f32()
                    ));
                    ui.label(format!(
                        "Duration: {:.2}s",
                        self.renderer.duration().as_secs_f32()
                    ));
                    ui.label(format!("FPS: {:.2}", self.renderer.frame_rate()));
                    ui.label(format!("Speed: {:.2}x", self.renderer.speed().multiplier()));
                    ui.label(format!(
                        "Frames rendered: {}",
                        self.frame_count
                    ));
                });
            });
    }
}

impl eframe::App for PreviewPlayerApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Update renderer state
        self.update_renderer(ui.ctx());

        // Render video frame
        self.render_video_frame(ui);

        // Render control panel
        self.render_control_panel(ui);

        // Render info panel
        self.render_info_panel(ui.ctx());
    }
}

fn main() -> Result<(), eframe::Error> {
    env_logger::init();

    // Try to create the app
    let app = PreviewPlayerApp::new().unwrap_or_else(|e| {
        log::error!("Failed to create app: {:?}", e);
        panic!("Failed to create preview player app: {:?}", e);
    });

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 720.0])
            .with_title("Preview Player Demo - PreviewRenderer + egui"),
        ..Default::default()
    };

    eframe::run_native(
        "Preview Player Demo",
        options,
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::new(app))
        }),
    )
}
