//! # Wayshot Recorder Library
//!
//! A high-performance screen recording library for Wayland compositors with audio support.
//! This library provides comprehensive recording capabilities including screen capture,
//! video encoding, audio recording, and file output in various formats.
//!
//! ## Features
//!
//! - **Screen Recording**: Capture screen content with configurable frame rates and resolutions
//! - **Audio Recording**: Record both input audio (microphone) and speaker output
//! - **Video Encoding**: Real-time H.264 video encoding with OpenH264
//! - **File Output**: Save recordings as H.264 files or combine with audio into MP4 containers
//! - **Performance Optimized**: Multi-threaded processing with configurable queue sizes
//! - **Preview Mode**: Real-time frame processing without file output
//!
//! ## Quick Start
//!
//! ```no_run
//! use recorder::{RecorderConfig, RecordingSession, Resolution, FPS};
//! use capture::LogicalSize;
//! use std::path::PathBuf;
//!
//! // Initialize the recording session
//! RecordingSession::init().unwrap();
//!
//! // Create recording configuration
//! let config = RecorderConfig::new(
//!     "HDMI-A-1".to_string(),
//!     LogicalSize { width: 1920, height: 1080 },
//!     PathBuf::from("recording.mp4"),
//! )
//! .with_fps(FPS::Fps30)
//! .with_resolution(Resolution::P1080)
//! .with_audio_device_name(Some("default".to_string()))
//! .with_enable_recording_speaker(true);
//!
//! // Start recording
//! let mut session = RecordingSession::new(config);
//! session.start().unwrap();
//!
//! // Wait for recording to complete
//! let result = session.wait(|progress| {
//!     println!("Recording progress: {:.1}%", progress * 100.0);
//! });
//! ```
//!
//! ## Architecture
//!
//! The library uses a multi-threaded pipeline:
//! 1. **Capture Threads**: Multiple threads capture screen frames concurrently
//! 2. **Resize Workers**: Resize frames to target resolution if needed
//! 3. **Encoder Worker**: Encode frames to H.264 format
//! 4. **H.264 Writer**: Write encoded frames to file
//! 5. **Audio Recorders**: Capture input and speaker audio in parallel
//! 6. **Track Combiner**: Combine video and audio tracks into final MP4 file
//!
//! ## Modules
//!
//! - [`recorder`]: Main recording session management
//! - [`recorder_config`]: Configuration types and builders
//! - [`recorder_error`]: Error types for recording operations
//! - [`resolution`]: Video resolution handling
//! - [`video_encoder`]: H.264 video encoding
//! - [`h264_writer`]: H.264 file writing with queue management
//! - [`record_audio`]: Input audio recording
//! - [`record_speaker`]: Speaker output recording
//! - [`audio_level`]: Audio level calculation utilities
//! - [`mp4_builtin`]: Built-in MP4 track combining (requires `mp4-builtin` feature)
//! - [`mp4_ffmpeg`]: FFmpeg-based MP4 track combining (requires `mp4-ffmpeg` feature)

mod audio_level;
mod deniose;
mod h264_writer;
mod mp4_ffmpeg;
mod record_audio;
mod record_speaker;
mod recorder;
mod recorder_config;
mod recorder_error;
mod resolution;
mod video_encoder;

pub use audio_level::*;
pub use crossbeam::channel::{Receiver, Sender, bounded};
pub use deniose::*;
pub use h264_writer::H264Writer;
pub use mp4_ffmpeg::{is_ffmpeg_installed, merge_tracks};
pub use record_audio::{
    AudioDeviceInfo, AudioError, AudioFileWriter, AudioRecorder, StreamingAudioRecorder,
};
pub use record_speaker::{SpeakerError, SpeakerRecorder};
pub use recorder::RecordingSession;
pub use recorder_config::{FPS, RecorderConfig, SimpleFpsCounter};
pub use recorder_error::RecorderError;
pub use resolution::Resolution;
pub use video_encoder::{EncodedFrame, VideoEncoder};

use capture::CaptureIterCallbackData;
use std::{
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool},
    time::Instant,
};

/// Represents the final state of a recording session.
///
/// This enum is returned by [`RecordingSession::wait`] to indicate whether
/// the recording completed successfully or was stopped by the user.
///
/// # Examples
///
/// ```no_run
/// use recorder::{RecordingSession, RecorderConfig, ProgressState};
///
/// let config = RecorderConfig::new("output".to_string(), Default::default(), "output.mp4".into());
/// let mut session = RecordingSession::new(config);
/// session.start().unwrap();
///
/// let result = session.wait(|_| {});
/// match result {
///     Ok(ProgressState::Finished) => println!("Recording completed successfully"),
///     Ok(ProgressState::Stopped) => println!("Recording was stopped by user"),
///     Err(e) => eprintln!("Recording failed: {}", e),
/// }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressState {
    /// Recording completed successfully and all frames were processed
    Finished,
    /// Recording was stopped by user before completion
    Stopped,
}

/// Represents a captured screen frame with metadata.
///
/// Contains the raw pixel data along with timing information and thread identification.
/// This structure is passed through the recording pipeline and can be accessed
/// via user channels if enabled in the configuration.
///
/// # Examples
///
/// ```no_run
/// use recorder::{RecordingSession, RecorderConfig, Frame};
///
/// let config = RecorderConfig::new("output".to_string(), Default::default(), "output.mp4".into())
///     .with_enable_frame_channel_user(true);
///
/// let mut session = RecordingSession::new(config);
/// session.start().unwrap();
///
/// if let Some(receiver) = session.get_frame_receiver_user() {
///     while let Ok(frame) = receiver.recv() {
///         println!("Received frame from thread {} at {:?}", frame.thread_id, frame.timestamp);
///     }
/// }
/// ```
#[derive(Debug, Clone)]
pub struct Frame {
    /// ID of the capture thread that produced this frame
    pub thread_id: u32,
    /// Raw capture data from the screen including pixel data and dimensions
    pub cb_data: CaptureIterCallbackData,
    /// Timestamp when the frame was captured
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
    pub frame: Option<Frame>,
}

/// Configuration for combining video and audio tracks into a final MP4 file.
///
/// This structure is used by both the built-in and FFmpeg track combiners
/// to specify input files and output parameters for the final video file.
///
/// # Examples
///
/// ```no_run
/// use recorder::{MergeTracksConfig, FPS};
/// use std::sync::Arc;
/// use std::sync::atomic::AtomicBool;
///
/// let config = MergeTracksConfig {
///     h264_path: "video.h264".into(),
///     input_wav_path: Some("audio.wav".into()),
///     speaker_wav_path: Some("speaker.wav".into()),
///     output_path: "output.mp4".into(),
///     fps: FPS::Fps30,
///     stop_sig: Arc::new(AtomicBool::new(false)),
/// };
/// ```
#[derive(Debug, Clone)]
pub struct MergeTracksConfig {
    /// Path to the H.264 video file containing encoded frames
    pub h264_path: PathBuf,
    /// Optional path to the input WAV audio file (microphone recording)
    pub input_wav_path: Option<PathBuf>,
    /// Optional path to the speaker WAV audio file (system audio recording)
    pub speaker_wav_path: Option<PathBuf>,
    /// Output path for the combined MP4 file
    pub output_path: PathBuf,
    /// Frame rate for the output video
    pub fps: FPS,
    /// Signal to stop the combining process if requested by user
    pub stop_sig: Arc<AtomicBool>,

    pub convert_input_wav_to_mono: bool,
}
