mod audio_level;
mod config;
mod denoise;
mod error;
mod record_audio;
mod record_speaker;
mod recorder;
mod resolution;
mod video_encoder;

pub use audio_level::*;
pub use config::{FPS, RecorderConfig, SimpleFpsCounter};
pub use crossbeam::channel::{Receiver, Sender, bounded};
pub use denoise::*;
pub use error::RecorderError;
pub use record_audio::{AudioDeviceInfo, AudioError, AudioRecorder};
pub use record_speaker::SpeakerRecorder;
pub use recorder::RecordingSession;
pub use resolution::Resolution;
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
    #[cfg(feature = "wayland-wlr")]
    let screen_capturer = wayland_wlr_screen_capture::WaylandWlrScreenCapture::new();

    screen_capturer
}
