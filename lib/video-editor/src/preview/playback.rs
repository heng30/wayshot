use crate::tracks::{Manager, frame_position::TimeToFrameConverter};
use std::{sync::Arc, time::Duration};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackState {
    Stopped,
    Playing,
    Paused,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackSpeed {
    Paused,    // 0x
    Quarter,   // 0.25x
    Half,      // 0.5x
    Normal,    // 1.0x
    Double,    // 2.0x
    Quadruple, // 4.0x
}

impl PlaybackSpeed {
    pub fn multiplier(&self) -> f64 {
        match self {
            PlaybackSpeed::Paused => 0.0,
            PlaybackSpeed::Quarter => 0.25,
            PlaybackSpeed::Half => 0.5,
            PlaybackSpeed::Normal => 1.0,
            PlaybackSpeed::Double => 2.0,
            PlaybackSpeed::Quadruple => 4.0,
        }
    }

    pub fn from_multiplier(multiplier: f64) -> Option<Self> {
        match multiplier {
            0.0 => Some(PlaybackSpeed::Paused),
            0.25 => Some(PlaybackSpeed::Quarter),
            0.5 => Some(PlaybackSpeed::Half),
            1.0 => Some(PlaybackSpeed::Normal),
            2.0 => Some(PlaybackSpeed::Double),
            4.0 => Some(PlaybackSpeed::Quadruple),
            _ => None,
        }
    }

    pub fn all() -> &'static [PlaybackSpeed] {
        &[
            PlaybackSpeed::Paused,
            PlaybackSpeed::Quarter,
            PlaybackSpeed::Half,
            PlaybackSpeed::Normal,
            PlaybackSpeed::Double,
            PlaybackSpeed::Quadruple,
        ]
    }
}

#[derive(Debug)]
pub struct PlaybackController {
    manager: Arc<Manager>,
    state: PlaybackState,
    position_frame: usize, // Internal position as frame index
    total_frames: usize,   // Total number of frames
    speed: PlaybackSpeed,
    time_converter: TimeToFrameConverter,
    volume: f32, // 0.0 to 1.0
}

impl PlaybackController {
    pub fn new(manager: Arc<Manager>, frame_rate: f64) -> Self {
        let time_converter = TimeToFrameConverter::from_f32(frame_rate as f32);
        let total_frames = time_converter.duration_to_frame(manager.duration);
        Self {
            manager,
            state: PlaybackState::Stopped,
            position_frame: 0,
            total_frames,
            speed: PlaybackSpeed::Normal,
            time_converter,
            volume: 1.0,
        }
    }

    pub fn state(&self) -> PlaybackState {
        self.state
    }

    pub fn position(&self) -> Duration {
        self.time_converter.frame_to_duration(self.position_frame)
    }

    pub fn speed(&self) -> PlaybackSpeed {
        self.speed
    }

    pub fn frame_rate(&self) -> f64 {
        self.time_converter.fps_as_f32() as f64
    }

    pub fn volume(&self) -> f32 {
        self.volume
    }

    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
    }

    pub fn set_position(&mut self, position: Duration) {
        let frame = self.time_converter.duration_to_frame(position);
        self.position_frame = frame.min(self.total_frames);
    }

    pub fn set_frame_rate(&mut self, frame_rate: f64) {
        self.time_converter = TimeToFrameConverter::from_f32(frame_rate as f32);
        self.total_frames = self.time_converter.duration_to_frame(self.manager.duration);
        self.position_frame = self.position_frame.min(self.total_frames);
    }

    pub fn set_speed(&mut self, speed: PlaybackSpeed) {
        self.speed = speed;
    }

    pub fn play(&mut self) {
        self.state = PlaybackState::Playing;
        if self.speed == PlaybackSpeed::Paused {
            self.speed = PlaybackSpeed::Normal;
        }
    }

    pub fn pause(&mut self) {
        self.state = PlaybackState::Paused;
    }

    pub fn stop(&mut self) {
        self.state = PlaybackState::Stopped;
        self.position_frame = 0;
        self.speed = PlaybackSpeed::Normal;
    }

    pub fn toggle_playback(&mut self) {
        match self.state {
            PlaybackState::Playing => self.pause(),
            PlaybackState::Paused | PlaybackState::Stopped => self.play(),
        }
    }

    pub fn step_forward(&mut self) {
        self.position_frame = (self.position_frame + 1).min(self.total_frames);
        self.state = PlaybackState::Paused;
    }

    pub fn step_backward(&mut self) {
        self.position_frame = self.position_frame.saturating_sub(1);
        self.state = PlaybackState::Paused;
    }

    pub fn skip_forward(&mut self, seconds: f64) {
        let skip_duration = Duration::from_secs_f64(seconds);
        let current_pos = self.position();
        let new_pos = (current_pos + skip_duration).min(self.manager.duration);
        self.position_frame = self.time_converter.duration_to_frame(new_pos);
    }

    pub fn skip_backward(&mut self, seconds: f64) {
        let skip_duration = Duration::from_secs_f64(seconds);
        let current_pos = self.position();
        let new_pos = current_pos.saturating_sub(skip_duration);
        self.position_frame = self.time_converter.duration_to_frame(new_pos);
    }

    pub fn current_frame(&self) -> u64 {
        self.position_frame as u64
    }

    pub fn percentage(&self) -> f64 {
        if self.total_frames == 0 {
            0.0
        } else {
            self.position_frame as f64 / self.total_frames as f64 * 100.0
        }
    }

    pub fn is_playing(&self) -> bool {
        self.state == PlaybackState::Playing
    }
}
