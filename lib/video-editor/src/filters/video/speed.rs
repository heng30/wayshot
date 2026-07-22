use crate::{
    Result,
    filters::traits::{VideoData, VideoFilter},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeedFilter {
    pub speed: f32,
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

impl VideoFilter for SpeedFilter {
    crate::impl_default_video_filter!(SpeedFilter);

    fn apply(&self, _data: &mut VideoData) -> Result<()> {
        // No pixel processing - speed adjustment happens in decode layer
        Ok(())
    }

    fn take_effect_in_layer_frame(&self) -> bool {
        false
    }
}
