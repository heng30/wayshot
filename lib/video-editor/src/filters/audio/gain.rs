use crate::{
    Result,
    filters::{
        interpolation::get_float_at_time,
        keyframe::{AnimatableProperty, KeyframeTracks, KeyframeValue},
        traits::{AudioData, AudioFilter},
    },
};
use derivative::Derivative;
use derive_setters::Setters;

const THRESHOLD: f32 = 0.95;

#[derive(Debug, Clone, Derivative, Setters, serde::Serialize, serde::Deserialize)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct GainFilter {
    // Linear amplitude gain (1.0 = no change, 2.0 = +6dB, 0.5 = -6dB)
    #[derivative(Default(value = "1.0"))]
    pub amplitude: f32,
    #[serde(skip_serializing_if = "KeyframeTracks::is_empty", default)]
    pub keyframe_tracks: KeyframeTracks,
}

impl GainFilter {
    pub const NAME: &'static str = "gain";

    pub fn from_db(db: f32) -> Self {
        let amplitude = 10f32.powf(db / 20.0);
        Self {
            amplitude,
            keyframe_tracks: KeyframeTracks::default(),
        }
    }

    pub fn db(&self) -> f32 {
        if self.amplitude <= 0.0 {
            f32::NEG_INFINITY
        } else {
            20.0 * self.amplitude.log10()
        }
    }

    /// Get the animatable properties for this filter
    pub fn animatable_properties() -> Vec<AnimatableProperty> {
        vec![AnimatableProperty::float(
            "amplitude",
            "Amplitude",
            0.0,
            10.0,
            1.0,
        )]
    }

    /// Get interpolated amplitude at a specific time
    fn get_amplitude_at_time(&self, time_ms: i64) -> f32 {
        self.keyframe_tracks
            .get_track("amplitude")
            .map(|track| get_float_at_time(track, time_ms, self.amplitude))
            .unwrap_or(self.amplitude)
    }
}

impl AudioFilter for GainFilter {
    crate::impl_default_audio_filter!(GainFilter);

    fn apply(&self, data: &mut AudioData) -> Result<()> {
        if data.samples.is_empty() {
            return Ok(());
        }

        let sample_rate = data.config.sample_rate;
        let channels = data.config.channels as usize;
        let start_time_ms = data.relative_timeline_offset.as_millis() as i64;

        // 计算每个样本对应的时间（毫秒）
        // samples_per_ms = sample_rate * channels / 1000
        let samples_per_ms = (sample_rate as f64 * channels as f64 / 1000.0) as f64;

        for (i, sample) in data.samples.iter_mut().enumerate() {
            // 计算当前样本相对于 segment 开始的时间
            let sample_time_ms = start_time_ms + (i as f64 / samples_per_ms) as i64;
            let amplitude = self.get_amplitude_at_time(sample_time_ms);
            *sample *= amplitude;
        }

        // soft clipping — output bounded to [-1.0, 1.0]
        for sample in &mut data.samples {
            *sample = soft_clip(*sample, THRESHOLD);
        }

        Ok(())
    }

    fn get_animatable_properties(&self) -> Vec<AnimatableProperty> {
        Self::animatable_properties()
    }

    fn get_keyframe_tracks(&self) -> KeyframeTracks {
        self.keyframe_tracks.clone()
    }

    fn set_keyframe_tracks(&mut self, tracks: KeyframeTracks) {
        self.keyframe_tracks = tracks;
    }

    fn supports_keyframes(&self) -> bool {
        true
    }

    fn update_keyframes_at_time(&self, tracks: &mut KeyframeTracks, time_ms: i64) -> bool {
        if let Some(track) = tracks.get_track("amplitude")
            && track.keyframes.iter().any(|k| k.time_ms == time_ms)
        {
            tracks.update_keyframe_value(
                "amplitude",
                time_ms,
                KeyframeValue::Float(self.amplitude),
            );
            return true;
        }
        false
    }
}

/// Soft clip a sample using atan-based curve, output bounded to [-1.0, 1.0]
/// Formula: sign(x) * (|x| * 2/PI * atan(PI/2 * (|x| - T) / (1 - T)) + T) for |x| > T
#[inline]
fn soft_clip(sample: f32, threshold: f32) -> f32 {
    if sample > threshold {
        let excess = sample - threshold;
        let headroom = 1.0 - threshold;
        // 2/PI * atan(PI/2 * excess / headroom) ∈ (0, 1)
        let normalized =
            (2.0 / std::f32::consts::PI) * (std::f32::consts::PI / 2.0 * excess / headroom).atan();
        threshold + headroom * normalized
    } else if sample < -threshold {
        let excess = -sample - threshold;
        let headroom = 1.0 - threshold;
        let normalized =
            (2.0 / std::f32::consts::PI) * (std::f32::consts::PI / 2.0 * excess / headroom).atan();
        -threshold - headroom * normalized
    } else {
        sample
    }
}
