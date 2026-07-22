use crate::{
    Result,
    filters::traits::{AudioData, AudioFilter},
};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NormalizeFilter {
    pub target_level_db: f32,
}

impl Default for NormalizeFilter {
    fn default() -> Self {
        Self {
            target_level_db: -1.0,
        }
    }
}

impl NormalizeFilter {
    pub const NAME: &'static str = "normalize";

    pub fn new(target_level_db: f32) -> Self {
        Self { target_level_db }
    }
}

impl AudioFilter for NormalizeFilter {
    crate::impl_default_audio_filter!(NormalizeFilter);

    fn apply(&self, data: &mut AudioData) -> Result<()> {
        if data.samples.is_empty() {
            return Ok(());
        }

        let peak_level = data
            .samples
            .iter()
            .map(|s| s.abs())
            .fold(0.0f32, |a, b| a.max(b));

        if peak_level <= 0.0 {
            return Ok(());
        }

        let target_level = 10f32.powf(self.target_level_db / 20.0);
        let gain = target_level / peak_level;

        for sample in &mut data.samples {
            *sample = (*sample * gain).clamp(-1.0, 1.0);
        }

        Ok(())
    }
}
