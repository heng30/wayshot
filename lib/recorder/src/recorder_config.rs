use crate::resolution::Resolution;
use capture::LogicalSize;
use chrono::Local;
use derive_setters::Setters;
use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    sync::{Arc, atomic::AtomicI32},
    time::{Duration, Instant},
};

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

    pub save_path: PathBuf,
}

impl RecorderConfig {
    pub fn new(screen_name: String, screen_size: LogicalSize, save_path: PathBuf) -> Self {
        Self {
            screen_name,
            save_path,
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
        }
    }

    pub fn frame_interval_ms(&self) -> u64 {
        (1000.0 / self.fps.to_u32() as f64) as u64
    }

    pub fn make_filename(dir: impl AsRef<Path>) -> PathBuf {
        let mut filename = Local::now().format("%Y-%m-%d_%H:%M:%S").to_string();
        filename.push_str(".mp4");
        dir.as_ref().to_path_buf().join(filename)
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
