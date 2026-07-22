//! # Speed Filter for Audio
//!
//! ## Purpose
//! Adjusts the playback speed of audio content.
//!
//! ## How it works
//! - This is a metadata filter that syncs with `segment.playback_speed`
//! - Speed adjustment happens at the decode layer (see `audio_track.rs`)
//! - The filter does NOT process audio samples directly
//!
//! ## Usage
//! - `speed: 1.0` - Normal speed
//! - `speed: 0.5` - Half speed (audio plays slower, pitch unchanged with proper resampling)
//! - `speed: 2.0` - Double speed (audio plays faster)
//!
//! ## Design Note
//! Both video and audio speed filters modify the same `segment.playback_speed` property.
//! This provides UI consistency - users can access "Speed" from both video and audio filter menus.

use crate::{
    Result,
    filters::traits::{AudioData, AudioFilter},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeedFilter {
    pub speed: f32, // Playback speed multiplier (0.1 to 10.0)
}

impl Default for SpeedFilter {
    fn default() -> Self {
        Self { speed: 1.0 }
    }
}

impl SpeedFilter {
    pub const NAME: &'static str = "speed";

    pub fn new(speed: f32) -> Self {
        Self {
            speed: speed.clamp(0.1, 10.0),
        }
    }
}

impl AudioFilter for SpeedFilter {
    crate::impl_default_audio_filter!(SpeedFilter);

    fn apply(&self, _data: &mut AudioData) -> Result<()> {
        // No sample processing - speed adjustment happens in decode layer
        // The segment.playback_speed property is used by audio_track.rs
        Ok(())
    }
}

