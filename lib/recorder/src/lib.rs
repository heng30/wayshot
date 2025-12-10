mod audio_level;
mod audio_recorder;
mod config;
mod cursor_tracker;
mod denoise;
mod error;
mod process_mode;
mod recorder;
mod resolution;
mod speaker_recorder;

pub use audio_level::*;
pub use audio_recorder::{AudioDeviceInfo, AudioRecorder, AudioRecorderError};
pub use config::{FPS, RecorderConfig, SimpleFpsCounter};
pub use crossbeam::channel::{Receiver, Sender, bounded};
pub use cursor_tracker::{CursorTracker, CursorTrackerConfig, TransitionType};
pub use denoise::*;
pub use error::RecorderError;
pub use recorder::{RecordingSession, ResizedImageBuffer};
pub use resolution::Resolution;
pub use speaker_recorder::{
    SpeakerRecorder, SpeakerRecorderConfig, SpeakerRecorderError, platform_speaker_recoder,
};
pub use video_encoder::{EncodedFrame, VideoEncoder, VideoEncoderConfig, new as video_encoder_new};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressState {
    Finished,
    Stopped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessMode {
    RecordScreen,
    ShareScreen,
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
    pub buffer: ResizedImageBuffer,
}

pub fn platform_screen_capture() -> impl screen_capture::ScreenCapture + Clone + Send + 'static {
    #[cfg(all(target_os = "linux", feature = "wayland-wlr"))]
    let screen_capturer = screen_capture_wayland_wlr::ScreenCaptureWaylandWlr::default();

    #[cfg(all(target_os = "linux", feature = "wayland-portal"))]
    let screen_capturer = screen_capture_wayland_portal::ScreenCaptureWaylandPortal::default();

    #[cfg(all(target_os = "windows", feature = "windows"))]
    let screen_capturer = screen_capture_windows::ScreenCaptureWindows::default();

    screen_capturer
}
