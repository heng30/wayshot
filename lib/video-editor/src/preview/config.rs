use crate::tracks::unified_mixer::UnifiedMixerConfig;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct PreviewConfig {
    pub mixer: UnifiedMixerConfig,
    pub loop_region: Option<LoopRegion>,
}

impl PreviewConfig {
    pub fn frame_rate(&self) -> f64 {
        self.mixer.output_fps.map(|f| f as f64).unwrap_or(25.0)
    }
}

impl Default for PreviewConfig {
    fn default() -> Self {
        let mut mixer = UnifiedMixerConfig::default();
        mixer.output_width = Some(1920);
        mixer.output_height = Some(1080);
        mixer.output_fps = Some(25.0);

        Self {
            mixer,
            loop_region: None,
        }
    }
}

impl From<UnifiedMixerConfig> for PreviewConfig {
    fn from(mixer: UnifiedMixerConfig) -> Self {
        Self {
            mixer,
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone)]
pub struct LoopRegion {
    pub start: Duration,
    pub end: Duration,
    pub enabled: bool,
}

impl LoopRegion {
    pub fn new(start: Duration, end: Duration) -> Self {
        Self {
            start,
            end,
            enabled: true,
        }
    }

    pub fn contains(&self, position: Duration) -> bool {
        !self.enabled || (position >= self.start && position < self.end)
    }

    pub fn clamp(&self, position: Duration) -> Duration {
        if !self.enabled {
            return position;
        }
        if position < self.start {
            self.start
        } else if position >= self.end {
            self.start
        } else {
            position
        }
    }
}
