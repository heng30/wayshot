use crate::{
    AsyncErrorSender, ProcessMode, cursor_tracker::TransitionType, resolution::Resolution,
};
use background_remover::Model as BackgroundRemoverModel;
use camera::{Shape, ShapeCircle};
use chrono::Local;
use derive_setters::Setters;
use image_effect::realtime::RealtimeImageEffect;
use screen_capture::LogicalSize;
use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicI32, AtomicU8},
    },
    time::{Duration, Instant},
};
use wrtc::RTCIceServer;

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub enum FPS {
    Fps24,
    Fps25,
    Fps30,
    Fps60,
}

impl FPS {
    pub fn to_u32(&self) -> u32 {
        match self {
            FPS::Fps24 => 24,
            FPS::Fps25 => 25,
            FPS::Fps30 => 30,
            FPS::Fps60 => 60,
        }
    }

    pub fn max() -> u32 {
        FPS::Fps60.to_u32()
    }
}

#[derive(Debug, Clone, Setters)]
#[setters(prefix = "with_")]
pub struct RecorderConfig {
    pub save_path: PathBuf,
    pub process_mode: ProcessMode,
    pub async_error_sender: Option<AsyncErrorSender>,

    pub screen_name: String,
    pub screen_size: LogicalSize,
    pub fps: FPS,
    pub resolution: Resolution,
    pub include_cursor: bool,

    pub audio_device_name: Option<String>,
    pub enable_recording_speaker: bool,
    pub enable_audio_level_channel: bool,
    pub enable_speaker_level_channel: bool,
    pub enable_denoise: bool,
    pub convert_to_mono: bool,

    #[setters(strip_option)]
    pub audio_gain: Option<Arc<AtomicI32>>,

    #[setters(strip_option)]
    pub speaker_gain: Option<Arc<AtomicI32>>,

    pub enable_cursor_tracking: bool,
    pub region_width: i32,
    pub region_height: i32,
    pub debounce_radius: u32,
    pub stable_radius: u32,
    pub fast_moving_duration: u64,
    pub zoom_transition_duration: u64,
    pub reposition_edge_threshold: f32,
    pub reposition_transition_duration: u64,
    pub max_stable_region_duration: u64,
    pub zoom_in_transition_type: TransitionType,
    pub zoom_out_transition_type: TransitionType,

    pub share_screen_config: ShareScreenConfig,
    pub push_stream_config: PushStreamConfig,
    pub camera_mix_config: CameraMixConfig,
    pub realtime_image_effect: Arc<AtomicU8>,
}

impl RecorderConfig {
    pub fn new(screen_name: String, screen_size: LogicalSize, save_path: PathBuf) -> Self {
        Self {
            save_path,
            process_mode: ProcessMode::RecordScreen,
            async_error_sender: None,

            screen_name,
            screen_size,
            fps: FPS::Fps25,
            resolution: Resolution::P1080,
            include_cursor: true,

            audio_device_name: None,
            enable_recording_speaker: false,

            enable_audio_level_channel: false,
            enable_speaker_level_channel: false,

            audio_gain: None,
            speaker_gain: None,
            enable_denoise: false,
            convert_to_mono: false,

            enable_cursor_tracking: false,
            region_width: 1280,
            region_height: 720,
            debounce_radius: 30,
            stable_radius: 30,
            fast_moving_duration: 100,
            zoom_transition_duration: 1000,
            reposition_edge_threshold: 0.15,
            reposition_transition_duration: 100,
            max_stable_region_duration: 5,
            zoom_in_transition_type: TransitionType::EaseIn,
            zoom_out_transition_type: TransitionType::EaseOut,

            share_screen_config: ShareScreenConfig::default(),
            push_stream_config: PushStreamConfig::default(),
            camera_mix_config: CameraMixConfig::default(),
            realtime_image_effect: Arc::new(AtomicU8::new(RealtimeImageEffect::None.into())),
        }
    }

    pub fn frame_interval_ms(&self) -> u64 {
        (1000.0 / self.fps.to_u32() as f64) as u64
    }

    pub fn make_filename(dir: impl AsRef<Path>) -> PathBuf {
        let mut filename = Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
        filename.push_str(".mp4");
        dir.as_ref().to_path_buf().join(filename)
    }
}

#[non_exhaustive]
#[derive(Debug, Default, Clone, Setters)]
#[setters(prefix = "with_")]
pub struct ShareScreenConfig {
    pub save_mp4: bool,

    pub listen_addr: String,
    pub auth_token: Option<String>,
    pub turn_server: Option<RTCIceServer>,
    pub stun_server: Option<RTCIceServer>,
    pub host_ips: Vec<String>,
    pub disable_host_ipv6: bool,

    pub enable_https: bool,
    pub cert_file: Option<String>,
    pub key_file: Option<String>,
}

impl ShareScreenConfig {
    pub fn new(listen_addr: String) -> Self {
        ShareScreenConfig::default().with_listen_addr(listen_addr)
    }
}

#[non_exhaustive]
#[derive(Debug, Default, Clone, Setters)]
#[setters(prefix = "with_")]
pub struct PushStreamConfig {
    #[setters[skip]]
    pub server_addr: String,

    #[setters[skip]]
    pub app: String,

    #[setters[skip]]
    pub stream_key: String,

    pub query_params: String,
    pub save_mp4: bool,
}

impl PushStreamConfig {
    pub fn new(server_addr: String, app: String, stream_key: String) -> Self {
        PushStreamConfig {
            server_addr,
            app,
            stream_key,
            query_params: String::new(),
            save_mp4: true,
        }
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, Setters)]
#[setters(prefix = "with_")]
pub struct CameraMixConfig {
    pub enable: bool,
    pub camera_name: Option<String>,

    pub fps: u32,
    pub width: u32,
    pub height: u32,
    pub mirror_horizontal: bool,
    pub pixel_format: camera::PixelFormat,

    pub shape: Shape,

    pub background_remover_model: Option<BackgroundRemoverModel>,
    pub background_remover_model_path: Option<PathBuf>,
}

impl Default for CameraMixConfig {
    fn default() -> Self {
        Self {
            enable: false,
            camera_name: None,
            width: 640,
            height: 480,
            fps: 25,
            pixel_format: camera::PixelFormat::RGBA,
            shape: Shape::Circle(ShapeCircle::default()),
            mirror_horizontal: false,
            background_remover_model: None,
            background_remover_model_path: None,
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct SimpleFpsCounter {
    pub fps: f32,
    frames: VecDeque<Instant>,
}

impl SimpleFpsCounter {
    pub fn new() -> Self {
        Self {
            frames: VecDeque::new(),
            fps: 0.0,
        }
    }

    pub fn add_frame(&mut self, timestamp: Instant) -> f32 {
        let three_seconds_ago = timestamp - Duration::from_secs(3);

        while let Some(&oldest) = self.frames.front() {
            if oldest < three_seconds_ago {
                self.frames.pop_front();
            } else {
                break;
            }
        }

        self.frames.push_back(timestamp);

        if self.frames.len() >= 2 {
            let time_span = timestamp.duration_since(*self.frames.front().unwrap());
            if time_span.as_secs_f64() > 0.0 {
                self.fps = (self.frames.len() as f64 / time_span.as_secs_f64()) as f32;
                return self.fps;
            }
        }

        0.0
    }
}
