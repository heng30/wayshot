mod audio_level;
mod denoise;
mod record_audio;
mod record_speaker;
mod recorder;
mod recorder_config;
mod recorder_error;
mod resolution;
mod video_encoder;

pub use audio_level::*;
pub use crossbeam::channel::{Receiver, Sender, bounded};
pub use denoise::*;
pub use record_audio::{AudioDeviceInfo, AudioError, AudioRecorder};
pub use record_speaker::SpeakerRecorder;
pub use recorder::RecordingSession;
pub use recorder_config::{FPS, RecorderConfig, SimpleFpsCounter};
pub use recorder_error::RecorderError;
pub use resolution::Resolution;
pub use video_encoder::{EncodedFrame, VideoEncoder};

use capture::CaptureIterCallbackData;
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressState {
    Finished,
    Stopped,
}

#[derive(Debug, Clone)]
pub struct Frame {
    pub thread_id: u32,
    pub cb_data: CaptureIterCallbackData,
    pub timestamp: Instant,
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
