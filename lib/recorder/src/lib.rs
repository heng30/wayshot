mod audio_level;
mod audio_recorder;
mod config;
mod cursor_tracker;
mod denoise;
mod error;
mod recorder;
mod resolution;
mod speaker_recorder;
mod video_encoder;

pub use audio_level::*;
pub use audio_recorder::{AudioDeviceInfo, AudioRecorder, AudioRecorderError};
pub use config::{FPS, RecorderConfig, SimpleFpsCounter};
pub use crossbeam::channel::{Receiver, Sender, bounded};
pub use cursor_tracker::{CursorTracker, CursorTrackerConfig};
pub use denoise::*;
pub use error::RecorderError;
pub use recorder::RecordingSession;
pub use resolution::Resolution;
pub use speaker_recorder::{
    SpeakerRecorder, SpeakerRecorderConfig, SpeakerRecorderError, platform_speaker_recoder,
};
pub use video_encoder::{EncodedFrame, VideoEncoder};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressState {
    Finished,
    Stopped,
}

#[derive(Debug, Clone)]
pub struct Frame {
    pub thread_id: u32,
    pub cb_data: screen_capture::CaptureStreamCallbackData,
    pub timestamp: std::time::Instant,
}

#[derive(Debug, Clone)]
pub struct StatsUser {
    pub fps: f32,
    pub total_frames: u64,
    pub loss_frames: u64,
}

#[derive(Debug, Clone)]
pub struct FrameUser {
    pub stats: StatsUser,
    pub frame: Frame,
}

pub fn platform_screen_capture() -> impl screen_capture::ScreenCapture + Clone + Send + 'static {
    #[cfg(all(target_os = "linux", feature = "wayland-wlr"))]
    let screen_capturer = screen_capture_wayland_wlr::ScreenCaptureWaylandWlr::default();

    screen_capturer
}
