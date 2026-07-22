use crate::{
    Result,
    filters::{
        interpolation::get_float_at_time,
        keyframe::{AnimatableProperty, KeyframeTracks, KeyframeValue},
        traits::{VideoData, VideoFilter},
    },
    tracks::video_frame_cache::VideoImage,
};
use rayon::prelude::*;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OpacityFilter {
    pub opacity: f32, // 0.0 = fully transparent, 1.0 = fully opaque
    #[serde(skip_serializing_if = "KeyframeTracks::is_empty", default)]
    pub keyframe_tracks: KeyframeTracks,
}

impl Default for OpacityFilter {
    fn default() -> Self {
        Self {
            opacity: 1.0,
            keyframe_tracks: KeyframeTracks::default(),
        }
    }
}

impl OpacityFilter {
    pub const NAME: &'static str = "opacity";

    pub fn new(opacity: f32) -> Self {
        Self {
            opacity: opacity.clamp(0.0, 1.0),
            keyframe_tracks: KeyframeTracks::default(),
        }
    }

    /// Get the animatable properties for this filter
    pub fn animatable_properties() -> Vec<AnimatableProperty> {
        vec![AnimatableProperty::float(
            "opacity", "Opacity", 0.0, 1.0, 1.0,
        )]
    }

    /// Get interpolated opacity at a specific time
    fn get_opacity_at_time(&self, time_ms: i64) -> f32 {
        self.keyframe_tracks
            .get_track("opacity")
            .map(|track| get_float_at_time(track, time_ms, self.opacity))
            .unwrap_or(self.opacity)
    }
}

impl VideoFilter for OpacityFilter {
    crate::impl_default_video_filter!(OpacityFilter);

    fn apply(&self, data: &mut VideoData) -> Result<()> {
        // Calculate current time in milliseconds relative to segment start
        let time_ms = data.relative_timeline_offset.as_millis() as i64;

        // Get interpolated value at current time
        let opacity = self.get_opacity_at_time(time_ms).clamp(0.0, 1.0);

        for frame in data.frames.iter_mut() {
            if let VideoImage::Image { buffer, .. } = frame {
                buffer.par_pixels_mut().for_each(|pixel| {
                    pixel.0[3] = ((pixel.0[3] as f32) * opacity).clamp(0.0, 255.0) as u8;
                });
            }
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
        if let Some(track) = tracks.get_track("opacity")
            && track.keyframes.iter().any(|k| k.time_ms == time_ms)
        {
            tracks.update_keyframe_value("opacity", time_ms, KeyframeValue::Float(self.opacity));
            return true;
        }
        false
    }
}

