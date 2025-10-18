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

/// Supported frame rates for screen recording.
///
/// These frame rates represent common standards for video recording.
/// Higher frame rates provide smoother motion but require more processing power
/// and storage space.
///
/// # Examples
///
/// ```
/// use recorder::FPS;
///
/// let fps = FPS::Fps30;
/// println!("Frame rate: {} FPS", fps.to_u32());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub enum FPS {
    /// 24 frames per second - cinematic standard
    Fps24,
    /// 25 frames per second - PAL standard
    Fps25,
    /// 30 frames per second - common for screen recording
    Fps30,
    /// 60 frames per second - high frame rate for smooth motion
    Fps60,
}

impl FPS {
    /// Convert FPS enum to numeric value.
    ///
    /// # Returns
    ///
    /// The frame rate as a `u32` value.
    ///
    /// # Examples
    ///
    /// ```
    /// use recorder::FPS;
    ///
    /// assert_eq!(FPS::Fps24.to_u32(), 24);
    /// assert_eq!(FPS::Fps60.to_u32(), 60);
    /// ```
    pub fn to_u32(&self) -> u32 {
        match self {
            FPS::Fps24 => 24,
            FPS::Fps25 => 25,
            FPS::Fps30 => 30,
            FPS::Fps60 => 60,
        }
    }

    /// Get the maximum supported frame rate.
    ///
    /// # Returns
    ///
    /// The maximum frame rate as a `u32` value.
    ///
    /// # Examples
    ///
    /// ```
    /// use recorder::FPS;
    ///
    /// assert_eq!(FPS::max(), 60);
    /// ```
    pub fn max() -> u32 {
        FPS::Fps60.to_u32()
    }
}

/// Configuration for screen recording sessions.
///
/// This struct contains all parameters needed to configure a recording session,
/// including output settings, performance parameters, and optional features.
/// The struct uses the `derive_setters` crate to provide a builder pattern
/// for convenient configuration.
///
/// # Examples
///
/// ```
/// use recorder::{RecorderConfig, FPS, Resolution};
/// use capture::LogicalSize;
/// use std::path::PathBuf;
///
/// // Create configuration with builder pattern
/// let config = RecorderConfig::new(
///     "HDMI-A-1".to_string(),
///     LogicalSize { width: 1920, height: 1080 },
///     PathBuf::from("recording.mp4"),
/// )
/// .with_fps(FPS::Fps30)
/// .with_resolution(Resolution::P1080)
/// .with_audio_device_name(Some("default".to_string()))
/// .with_enable_recording_speaker(true)
/// .with_max_queue_size(512);
/// ```
#[derive(Debug, Clone, Setters)]
#[setters(prefix = "with_")]
pub struct RecorderConfig {
    /// Screen output name to capture (e.g., "HDMI-A-1", "eDP-1")
    pub name: String,
    /// Screen logical size in pixels
    pub screen_logical_size: LogicalSize,
    /// Frame rate for the recording
    pub fps: FPS,
    /// Output resolution for the recorded video
    pub resolution: Resolution,
    /// Output file path for the final MP4 file
    pub output_path: PathBuf,
    /// Whether to include cursor in the screen capture
    pub include_cursor: bool,
    /// Whether to remove temporary cache files after recording
    pub remove_cache_files: bool,
    /// Audio device name for input recording (None for default device)
    pub audio_device_name: Option<String>,
    /// Enable recording of speaker output (system audio)
    pub enable_recording_speaker: bool,
    /// Enable sending frames to user channel for real-time processing
    pub enable_frame_channel_user: bool,
    /// Enable sending input audio levels to user channel
    pub enable_audio_channel_user: bool,
    /// Enable sending speaker audio levels to user channel
    pub enable_speaker_channel_user: bool,
    /// Enable preview mode (process frames without writing to file)
    pub enable_preview_mode: bool,

    pub enable_denoise: bool,

    pub disable_save_file: bool,

    #[setters(strip_option)]
    pub audio_amplification: Option<Arc<AtomicI32>>,

    #[setters(strip_option)]
    pub speaker_amplification: Option<Arc<AtomicI32>>,

    pub convert_input_wav_to_mono: bool,
}

impl RecorderConfig {
    /// Create a new recording configuration with default settings.
    ///
    /// # Arguments
    ///
    /// * `name` - Screen output name to capture
    /// * `screen_logical_size` - Logical size of the screen in pixels
    /// * `output_path` - Path where the final MP4 file will be saved
    ///
    /// # Returns
    ///
    /// A new `RecorderConfig` with default settings:
    /// - FPS: 25
    /// - Resolution: 1080p
    /// - Include cursor: true
    /// - Remove cache files: false
    /// - Max queue size: 256
    /// - Audio recording: disabled
    /// - Speaker recording: disabled
    /// - User channels: disabled
    /// - Preview mode: disabled
    ///
    /// # Examples
    ///
    /// ```
    /// use recorder::RecorderConfig;
    /// use capture::LogicalSize;
    /// use std::path::PathBuf;
    ///
    /// let config = RecorderConfig::new(
    ///     "HDMI-A-1".to_string(),
    ///     LogicalSize { width: 1920, height: 1080 },
    ///     PathBuf::from("recording.mp4"),
    /// );
    /// ```
    pub fn new(name: String, screen_logical_size: LogicalSize, output_path: PathBuf) -> Self {
        Self {
            name,
            screen_logical_size,
            output_path,
            fps: FPS::Fps25,
            resolution: Resolution::P1080,
            include_cursor: true,
            remove_cache_files: false,
            audio_device_name: None,
            enable_recording_speaker: false,
            enable_frame_channel_user: false,
            enable_audio_channel_user: false,
            enable_speaker_channel_user: false,
            enable_preview_mode: false,
            enable_denoise: false,
            disable_save_file: false,
            audio_amplification: None,
            speaker_amplification: None,
            convert_input_wav_to_mono: false,
        }
    }

    /// Get the capture interval in milliseconds based on the configured FPS.
    ///
    /// This value represents the time between consecutive frame captures.
    ///
    /// # Returns
    ///
    /// The capture interval in milliseconds as `u64`.
    ///
    /// # Examples
    ///
    /// ```
    /// use recorder::{RecorderConfig, FPS};
    /// use capture::LogicalSize;
    ///
    /// let config = RecorderConfig::new("output".to_string(), Default::default(), "output.mp4".into())
    ///     .with_fps(FPS::Fps30);
    ///
    /// // 1000ms / 30fps ≈ 33ms
    /// assert_eq!(config.frame_interval_ms(), 33);
    /// ```
    pub fn frame_interval_ms(&self) -> u64 {
        (1000.0 / self.fps.to_u32() as f64) as u64
    }

    /// Generate a filename with timestamp for automatic file naming.
    ///
    /// The generated filename uses the format: `YYYY-MM-DD_HH:MM:SS.mp4`
    ///
    /// # Arguments
    ///
    /// * `dir` - Directory where the file will be created
    ///
    /// # Returns
    ///
    /// A `PathBuf` containing the full path to the generated filename.
    ///
    /// # Examples
    ///
    /// ```
    /// use recorder::RecorderConfig;
    ///
    /// let filename = RecorderConfig::make_filename("/home/user/recordings");
    /// // Example: "/home/user/recordings/2024-01-15_14:30:00.mp4"
    /// ```
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
