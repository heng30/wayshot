use crate::{
    Result,
    filters::{
        interpolation::get_float_at_time,
        keyframe::{AnimatableProperty, KeyframeTracks, KeyframeValue},
        traits::{VideoData, VideoFilter},
    },
    tracks::video_frame_cache::VideoImage,
};
use image::Rgba;
use rayon::prelude::*;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChromaKeyFilter {
    #[serde(with = "crate::project::filters::color_serde::required")]
    pub target_color: Rgba<u8>,
    pub similarity: f32,
    pub softness: f32,
    pub feather: f32,
    pub spill_reduction: f32,
    #[serde(skip_serializing_if = "KeyframeTracks::is_empty", default)]
    pub keyframe_tracks: KeyframeTracks,
}

impl Default for ChromaKeyFilter {
    fn default() -> Self {
        ChromaKeyFilter::new(
            Rgba([0, 255, 0, 255]), // Green screen default
            0.4,                    // similarity
            0.1,                    // softness
            0.0,                    // feather
            0.0,                    // spill_reduction
        )
    }
}

impl ChromaKeyFilter {
    pub const NAME: &'static str = "chroma key";

    pub fn new(
        target_color: Rgba<u8>,
        similarity: f32,
        softness: f32,
        feather: f32,
        spill_reduction: f32,
    ) -> Self {
        Self {
            target_color,
            similarity,
            softness,
            feather,
            spill_reduction,
            keyframe_tracks: KeyframeTracks::default(),
        }
    }

    /// Get the animatable properties for this filter
    pub fn animatable_properties() -> Vec<AnimatableProperty> {
        vec![
            AnimatableProperty::float("similarity", "Similarity", 0.0, 1.0, 0.4),
            AnimatableProperty::float("softness", "Softness", 0.0, 1.0, 0.1),
            AnimatableProperty::float("feather", "Feather", 0.0, 1.0, 0.0),
            AnimatableProperty::float("spill_reduction", "Spill Reduction", 0.0, 1.0, 0.0),
        ]
    }

    /// Get interpolated values at a specific time
    fn get_values_at_time(&self, time_ms: i64) -> ChromaKeyValues {
        let similarity = self
            .keyframe_tracks
            .get_track("similarity")
            .map(|track| get_float_at_time(track, time_ms, self.similarity))
            .unwrap_or(self.similarity);

        let softness = self
            .keyframe_tracks
            .get_track("softness")
            .map(|track| get_float_at_time(track, time_ms, self.softness))
            .unwrap_or(self.softness);

        let feather = self
            .keyframe_tracks
            .get_track("feather")
            .map(|track| get_float_at_time(track, time_ms, self.feather))
            .unwrap_or(self.feather);

        let spill_reduction = self
            .keyframe_tracks
            .get_track("spill_reduction")
            .map(|track| get_float_at_time(track, time_ms, self.spill_reduction))
            .unwrap_or(self.spill_reduction);

        ChromaKeyValues {
            target_color: self.target_color,
            similarity: similarity.clamp(0.0, 1.0),
            softness: softness.clamp(0.0, 1.0),
            feather: feather.clamp(0.0, 1.0),
            spill_reduction: spill_reduction.clamp(0.0, 1.0),
        }
    }

    fn apply_to_buffer_with_values(values: &ChromaKeyValues, buffer: &mut image::RgbaImage) {
        let target = values.target_color;
        let similarity = values.similarity;
        let softness = values.softness;
        let feather = values.feather;
        let spill_reduction = values.spill_reduction;

        // Transition zone: similarity defines where keying starts,
        // softness + feather define the transition width to fully opaque
        let transition_start = similarity;
        let transition_end = (similarity + softness + feather).max(similarity);
        let transition_width = transition_end - transition_start;

        // Determine which color channel to suppress for spill reduction
        // (the dominant channel of the target color, e.g., green for green screen)
        let spill_channel = if target[1] >= target[0] && target[1] >= target[2] {
            1 // Green
        } else if target[2] >= target[0] {
            2 // Blue
        } else {
            0 // Red
        };

        buffer.par_pixels_mut().for_each(|pixel| {
            let current = *pixel;

            // Compute Euclidean color distance, normalized to [0, ~1.73]
            let dist = (current[0] as f32 - target[0] as f32).powi(2)
                + (current[1] as f32 - target[1] as f32).powi(2)
                + (current[2] as f32 - target[2] as f32).powi(2);

            let dist_norm = dist.sqrt() / 255.0;

            // Compute alpha based on transition zone
            let alpha = if transition_width <= 0.0 {
                // No transition zone: hard threshold
                if dist_norm <= transition_start {
                    0.0
                } else {
                    1.0
                }
            } else if dist_norm <= transition_start {
                0.0 // Fully transparent (within key color)
            } else if dist_norm >= transition_end {
                1.0 // Fully opaque (far from key color)
            } else {
                // Smooth transition using smoothstep (same as vignette/circle_mask filters)
                let t = (dist_norm - transition_start) / transition_width;
                t * t * (3.0 - 2.0 * t)
            };

            // Apply spill reduction: suppress the target color channel on visible pixels
            let (mut r, mut g, mut b) = (current[0] as f32, current[1] as f32, current[2] as f32);
            if spill_reduction > 0.0 && alpha > 0.0 {
                // Weight by proximity to key color: stronger suppression near edges
                let contamination = 1.0 - alpha;

                match spill_channel {
                    0 => {
                        // Red spill
                        let other_max = g.max(b);
                        if r > other_max {
                            let excess = r - other_max;
                            r -= excess * spill_reduction * contamination;
                        }
                    }
                    1 => {
                        // Green spill
                        let other_max = r.max(b);
                        if g > other_max {
                            let excess = g - other_max;
                            g -= excess * spill_reduction * contamination;
                        }
                    }
                    2 => {
                        // Blue spill
                        let other_max = r.max(g);
                        if b > other_max {
                            let excess = b - other_max;
                            b -= excess * spill_reduction * contamination;
                        }
                    }
                    _ => {}
                }
            }

            // Write final pixel values
            pixel.0[0] = r.clamp(0.0, 255.0) as u8;
            pixel.0[1] = g.clamp(0.0, 255.0) as u8;
            pixel.0[2] = b.clamp(0.0, 255.0) as u8;
            pixel.0[3] = (current[3] as f32 * alpha).clamp(0.0, 255.0) as u8;
        });
    }
}

/// Interpolated chroma key values at a specific time
struct ChromaKeyValues {
    target_color: Rgba<u8>,
    similarity: f32,
    softness: f32,
    feather: f32,
    spill_reduction: f32,
}

impl VideoFilter for ChromaKeyFilter {
    crate::impl_default_video_filter!(ChromaKeyFilter);

    fn apply(&self, data: &mut VideoData) -> Result<()> {
        // Calculate current time in milliseconds relative to segment start
        let time_ms = data.relative_timeline_offset.as_millis() as i64;

        // Get interpolated values at current time
        let values = self.get_values_at_time(time_ms);

        for frame in &mut data.frames {
            if let VideoImage::Image { buffer, .. } = frame {
                Self::apply_to_buffer_with_values(&values, buffer);
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
        let mut updated = false;

        for (property, value) in [
            ("similarity", self.similarity),
            ("softness", self.softness),
            ("feather", self.feather),
            ("spill_reduction", self.spill_reduction),
        ] {
            if let Some(track) = tracks.get_track(property)
                && track.keyframes.iter().any(|k| k.time_ms == time_ms)
            {
                tracks.update_keyframe_value(property, time_ms, KeyframeValue::Float(value));
                updated = true;
            }
        }

        updated
    }
}

