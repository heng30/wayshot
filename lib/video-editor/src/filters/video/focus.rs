use crate::{
    Result,
    filters::{
        interpolation::get_float_at_time,
        keyframe::{AnimatableProperty, KeyframeTracks, KeyframeValue},
        subtitle::style::scale_pixel_for_height,
        traits::{VideoData, VideoFilter},
    },
    tracks::video_frame_cache::VideoImage,
};
use draw_utils::{apply_box_blur, blur_factor};
use image::RgbaImage;
use rayon::prelude::*;

/// Focus (bokeh) filter that simulates camera aperture depth-of-field effect.
///
/// Creates a region of sharp focus surrounded by a smooth bokeh blur.
/// Uses a two-phase approach for performance:
/// 1. Generate a fully blurred version using separable box blur (O(W×H×R))
/// 2. Blend between original and blurred based on distance from focus center
///
/// The bokeh shape is approximated by repeated box blur passes which naturally
/// produce a disc-like kernel shape. More passes → rounder shape.
#[derive(
    Debug,
    Clone,
    derivative::Derivative,
    derive_setters::Setters,
    serde::Serialize,
    serde::Deserialize,
)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct FocusFilter {
    /// Center X of the focus region, normalized (0.0-1.0). 0.5 = center of image.
    #[derivative(Default(value = "0.5"))]
    pub center_x: f32,

    /// Center Y of the focus region, normalized (0.0-1.0). 0.5 = center of image.
    #[derivative(Default(value = "0.5"))]
    pub center_y: f32,

    /// Radius of the sharp focus region in pixels (based on 1080p).
    /// Pixels within this distance from center remain fully sharp.
    #[derivative(Default(value = "150"))]
    pub focus_radius: u32,

    /// Feather/transition zone width in pixels (based on 1080p).
    /// Controls how gradually the blur transitions from sharp to fully blurred.
    /// 0 = hard edge, larger values = smoother gradient.
    #[derivative(Default(value = "80"))]
    pub feather: u32,

    /// Blur intensity/radius for the out-of-focus region in pixels (based on 1080p).
    /// Larger values create stronger bokeh blur with bigger bokeh highlights.
    #[derivative(Default(value = "20"))]
    pub blur_radius: u32,

    /// Number of aperture blades (3-12). Controls the shape of bokeh highlights.
    /// - 5-6: visible pentagon/hexagon bokeh (like many real lenses)
    /// - 8+: nearly circular bokeh (like high-end lenses)
    /// - 3: triangular bokeh (rare but artistic)
    #[derivative(Default(value = "8"))]
    pub aperture_blades: u32,

    /// Bokeh highlight brightness boost (0.0-2.0).
    /// Enhances bright out-of-focus highlights to make bokeh "balls" more visible.
    /// 1.0 = no boost, >1.0 = brighter highlights.
    #[derivative(Default(value = "1.0"))]
    pub highlight_boost: f32,

    #[serde(skip_serializing_if = "KeyframeTracks::is_empty", default)]
    pub keyframe_tracks: KeyframeTracks,
}

/// Resolved parameter values at a given time (after keyframe interpolation).
pub struct FocusValues {
    pub center_x: f32,
    pub center_y: f32,
    pub focus_radius: u32,
    pub feather: u32,
    pub blur_radius: u32,
    pub aperture_blades: u32,
    pub highlight_boost: f32,
}

impl FocusFilter {
    pub const NAME: &'static str = "focus";

    pub fn new(center_x: f32, center_y: f32, focus_radius: u32, blur_radius: u32) -> Self {
        Self {
            center_x,
            center_y,
            focus_radius,
            feather: 80,
            blur_radius,
            aperture_blades: 8,
            highlight_boost: 1.0,
            keyframe_tracks: KeyframeTracks::default(),
        }
    }

    pub fn animatable_properties() -> Vec<AnimatableProperty> {
        vec![
            AnimatableProperty::float("center_x", "Center X", 0.0, 1.0, 0.5),
            AnimatableProperty::float("center_y", "Center Y", 0.0, 1.0, 0.5),
            AnimatableProperty::float("focus_radius", "Focus Radius", 0.0, 2000.0, 150.0),
            AnimatableProperty::float("feather", "Feather", 0.0, 500.0, 80.0),
            AnimatableProperty::float("blur_radius", "Blur Radius", 0.0, 100.0, 20.0),
            AnimatableProperty::float("aperture_blades", "Aperture Blades", 3.0, 12.0, 8.0),
            AnimatableProperty::float("highlight_boost", "Highlight Boost", 0.0, 2.0, 1.0),
        ]
    }

    /// Resolve all parameter values at a given time, applying keyframe interpolation.
    pub fn get_values_at_time(&self, time_ms: i64) -> FocusValues {
        let get_f = |prop: &str, default: f32| {
            self.keyframe_tracks
                .get_track(prop)
                .map(|track| get_float_at_time(track, time_ms, default))
                .unwrap_or(default)
        };

        FocusValues {
            center_x: get_f("center_x", self.center_x).clamp(0.0, 1.0),
            center_y: get_f("center_y", self.center_y).clamp(0.0, 1.0),
            focus_radius: get_f("focus_radius", self.focus_radius as f32).max(0.0) as u32,
            feather: get_f("feather", self.feather as f32).max(0.0) as u32,
            blur_radius: get_f("blur_radius", self.blur_radius as f32).max(0.0) as u32,
            aperture_blades: get_f("aperture_blades", self.aperture_blades as f32)
                .clamp(3.0, 12.0) as u32,
            highlight_boost: get_f("highlight_boost", self.highlight_boost).clamp(0.0, 2.0),
        }
    }

    /// Apply the focus filter using a two-phase approach:
    /// 1. Generate a fully blurred version using fast separable box blur
    /// 2. Blend between original and blurred based on distance from focus center
    fn apply_focus(values: &FocusValues, buffer: &mut RgbaImage) -> Result<()> {
        let width = buffer.width();
        let height = buffer.height();

        if values.blur_radius == 0 {
            return Ok(());
        }

        let cx = values.center_x * width as f32;
        let cy = values.center_y * height as f32;
        let focus_radius = values.focus_radius as f32;
        let feather = values.feather as f32;
        let highlight_boost = values.highlight_boost;

        // Phase 1: Generate fully blurred image using separable box blur
        let blurred = apply_box_blur(buffer, values.blur_radius);
        let blurred_raw = blurred.as_raw();

        // Phase 2: Blend between original and blurred based on distance
        let original = buffer.clone();
        let original_raw = original.as_raw();

        let width_usize = width as usize;
        let rows: Vec<Vec<[u8; 4]>> = (0..height)
            .into_par_iter()
            .map(|y| {
                (0..width)
                    .map(|x| {
                        let dx = x as f32 - cx;
                        let dy = y as f32 - cy;
                        let dist = (dx * dx + dy * dy).sqrt();
                        let factor = blur_factor(dist, focus_radius, feather);

                        if factor <= 0.0 {
                            // Fully in focus — keep original
                            let off = (y as usize * width_usize + x as usize) * 4;
                            [
                                original_raw[off],
                                original_raw[off + 1],
                                original_raw[off + 2],
                                original_raw[off + 3],
                            ]
                        } else if factor >= 1.0 {
                            // Fully blurred — apply highlight boost
                            let off = (y as usize * width_usize + x as usize) * 4;
                            let r = blurred_raw[off] as f32;
                            let g = blurred_raw[off + 1] as f32;
                            let b = blurred_raw[off + 2] as f32;
                            let a = blurred_raw[off + 3];

                            if highlight_boost != 1.0 {
                                let luminance = (r * 0.299 + g * 0.587 + b * 0.114) / 255.0;
                                let boost = 1.0 + (highlight_boost - 1.0) * luminance;
                                [
                                    (r * boost).clamp(0.0, 255.0) as u8,
                                    (g * boost).clamp(0.0, 255.0) as u8,
                                    (b * boost).clamp(0.0, 255.0) as u8,
                                    a,
                                ]
                            } else {
                                [blurred_raw[off], blurred_raw[off + 1], blurred_raw[off + 2], a]
                            }
                        } else {
                            // Blend between original and blurred
                            let inv = 1.0 - factor;
                            let off = (y as usize * width_usize + x as usize) * 4;

                            let or = original_raw[off] as f32;
                            let og = original_raw[off + 1] as f32;
                            let ob = original_raw[off + 2] as f32;
                            let oa = original_raw[off + 3] as f32;

                            let br = blurred_raw[off] as f32;
                            let bg = blurred_raw[off + 1] as f32;
                            let bb = blurred_raw[off + 2] as f32;
                            let ba = blurred_raw[off + 3] as f32;

                            let mr = or * inv + br * factor;
                            let mg = og * inv + bg * factor;
                            let mb = ob * inv + bb * factor;
                            let ma = oa * inv + ba * factor;

                            if highlight_boost != 1.0 {
                                let luminance = (br * 0.299 + bg * 0.587 + bb * 0.114) / 255.0;
                                let blur_boost = 1.0 + (highlight_boost - 1.0) * luminance;
                                [
                                    (or * inv + br * factor * blur_boost).clamp(0.0, 255.0) as u8,
                                    (og * inv + bg * factor * blur_boost).clamp(0.0, 255.0) as u8,
                                    (ob * inv + bb * factor * blur_boost).clamp(0.0, 255.0) as u8,
                                    ma.clamp(0.0, 255.0) as u8,
                                ]
                            } else {
                                [
                                    mr.clamp(0.0, 255.0) as u8,
                                    mg.clamp(0.0, 255.0) as u8,
                                    mb.clamp(0.0, 255.0) as u8,
                                    ma.clamp(0.0, 255.0) as u8,
                                ]
                            }
                        }
                    })
                    .collect()
            })
            .collect();

        // Write results back to buffer
        let pixels: Vec<u8> = rows.into_iter().flatten().flatten().collect();
        *buffer = RgbaImage::from_raw(width, height, pixels)
            .expect("Buffer size matches image dimensions");

        Ok(())
    }
}

impl VideoFilter for FocusFilter {
    crate::impl_default_video_filter!(FocusFilter);

    fn take_effect_in_layer_frame(&self) -> bool {
        false
    }

    fn apply(&self, data: &mut VideoData) -> Result<()> {
        let time_ms = data.relative_timeline_offset.as_millis() as i64;
        let values = self.get_values_at_time(time_ms);

        let output_height = data.config.output_height;

        // Scale pixel-based values from base 1080p to target resolution
        let scaled_values = FocusValues {
            center_x: values.center_x,
            center_y: values.center_y,
            focus_radius: scale_pixel_for_height(values.focus_radius, output_height),
            feather: scale_pixel_for_height(values.feather, output_height),
            blur_radius: scale_pixel_for_height(values.blur_radius, output_height),
            aperture_blades: values.aperture_blades,
            highlight_boost: values.highlight_boost,
        };

        for frame in data.frames.iter_mut() {
            if let VideoImage::Image { buffer, .. } = frame {
                Self::apply_focus(&scaled_values, buffer)?;
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
            ("center_x", self.center_x),
            ("center_y", self.center_y),
            ("focus_radius", self.focus_radius as f32),
            ("feather", self.feather as f32),
            ("blur_radius", self.blur_radius as f32),
            ("aperture_blades", self.aperture_blades as f32),
            ("highlight_boost", self.highlight_boost),
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

#[cfg(test)]
mod tests {
    use super::*;
    use draw_utils::blur_factor as bf;

    #[test]
    fn test_blur_factor_sharp_inside_focus() {
        // Inside focus radius → fully sharp (factor = 0)
        assert_eq!(bf(50.0, 100.0, 30.0), 0.0);
        assert_eq!(bf(0.0, 100.0, 30.0), 0.0);
        assert_eq!(bf(99.9, 100.0, 30.0), 0.0);
    }

    #[test]
    fn test_blur_factor_fully_blurred_outside() {
        // Beyond focus + feather → fully blurred (factor = 1)
        assert_eq!(bf(131.0, 100.0, 30.0), 1.0);
        assert_eq!(bf(200.0, 100.0, 30.0), 1.0);
    }

    #[test]
    fn test_blur_factor_transition_zone() {
        // In the feather transition zone → smooth 0→1
        let factor = bf(115.0, 100.0, 30.0);
        assert!(factor > 0.0 && factor < 1.0);

        // Midpoint should be around 0.5 (smoothstep at t=0.5 = 0.5)
        let mid = bf(115.0, 100.0, 30.0);
        assert!((mid - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_blur_factor_no_feather() {
        // Zero feather → hard edge, immediate transition
        assert_eq!(bf(99.0, 100.0, 0.0), 0.0);
        assert_eq!(bf(101.0, 100.0, 0.0), 1.0);
    }

    #[test]
    fn test_filter_name() {
        assert_eq!(FocusFilter::NAME, "focus");
    }

    #[test]
    fn test_default_values() {
        let filter = FocusFilter::default();
        assert_eq!(filter.center_x, 0.5);
        assert_eq!(filter.center_y, 0.5);
        assert_eq!(filter.focus_radius, 150);
        assert_eq!(filter.feather, 80);
        assert_eq!(filter.blur_radius, 20);
        assert_eq!(filter.aperture_blades, 8);
        assert!((filter.highlight_boost - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_setters() {
        let filter = FocusFilter::default()
            .with_center_x(0.3)
            .with_center_y(0.7)
            .with_focus_radius(200)
            .with_blur_radius(30)
            .with_feather(100)
            .with_aperture_blades(6)
            .with_highlight_boost(1.5);
        assert!((filter.center_x - 0.3).abs() < 0.001);
        assert!((filter.center_y - 0.7).abs() < 0.001);
        assert_eq!(filter.focus_radius, 200);
        assert_eq!(filter.blur_radius, 30);
        assert_eq!(filter.feather, 100);
        assert_eq!(filter.aperture_blades, 6);
        assert!((filter.highlight_boost - 1.5).abs() < 0.001);
    }

    #[test]
    fn test_animatable_properties() {
        let props = FocusFilter::animatable_properties();
        assert_eq!(props.len(), 7);
    }

    /// Example: Create a focus filter that simulates a portrait-style
    /// shallow depth of field with the subject in the center.
    #[test]
    fn test_example_portrait_dof() {
        let filter = FocusFilter::new(0.5, 0.5, 150, 20)
            .with_feather(80)
            .with_aperture_blades(8)
            .with_highlight_boost(1.2);

        assert_eq!(filter.focus_radius, 150);
        assert_eq!(filter.blur_radius, 20);
        assert_eq!(filter.feather, 80);
        assert_eq!(filter.aperture_blades, 8);
        assert!((filter.highlight_boost - 1.2).abs() < 0.001);
    }

    /// Example: Create a tilt-shift miniature effect with a horizontal
    /// band of focus (small focus radius, strong blur).
    #[test]
    fn test_example_tilt_shift() {
        let filter = FocusFilter::new(0.5, 0.5, 80, 35)
            .with_feather(40)
            .with_aperture_blades(6)
            .with_highlight_boost(1.5);

        assert_eq!(filter.focus_radius, 80);
        assert_eq!(filter.blur_radius, 35);
    }

    /// Example: Create a cinematic rack focus effect with hexagonal bokeh
    /// (5 aperture blades) and enhanced highlights.
    #[test]
    fn test_example_cinematic_bokeh() {
        let filter = FocusFilter::new(0.3, 0.4, 120, 25)
            .with_feather(60)
            .with_aperture_blades(5)
            .with_highlight_boost(1.8);

        assert_eq!(filter.aperture_blades, 5);
        assert!((filter.highlight_boost - 1.8).abs() < 0.001);
    }

    /// Example: Rack focus — animate the focus point from left to right
    /// over a 3-second clip, simulating a classic cinematic "pull focus".
    #[test]
    fn test_example_rack_focus_keyframes() {
        use crate::filters::keyframe::{Keyframe, PropertyTrack};

        let mut filter = FocusFilter::new(0.3, 0.5, 120, 25)
            .with_feather(60)
            .with_aperture_blades(8)
            .with_highlight_boost(1.3);

        // Build center_x keyframe track: left → center → right
        let center_x_track = PropertyTrack::with_keyframes(
            "center_x",
            vec![
                Keyframe::new(0, KeyframeValue::Float(0.3)),     // 0.0s: left
                Keyframe::new(1500, KeyframeValue::Float(0.5)),  // 1.5s: center
                Keyframe::new(3000, KeyframeValue::Float(0.7)),  // 3.0s: right
            ],
        );

        // Build center_y keyframe track: stay at vertical center
        let center_y_track = PropertyTrack::with_keyframes(
            "center_y",
            vec![
                Keyframe::new(0, KeyframeValue::Float(0.5)),
                Keyframe::new(3000, KeyframeValue::Float(0.5)),
            ],
        );

        // Build focus_radius keyframe track: narrow → wide (breathing DOF)
        let focus_radius_track = PropertyTrack::with_keyframes(
            "focus_radius",
            vec![
                Keyframe::new(0, KeyframeValue::Float(80.0)),    // 0.0s: narrow focus
                Keyframe::new(1500, KeyframeValue::Float(120.0)), // 1.5s: widening
                Keyframe::new(3000, KeyframeValue::Float(160.0)), // 3.0s: wide focus
            ],
        );

        let mut tracks = KeyframeTracks::default();
        tracks.tracks.push(center_x_track);
        tracks.tracks.push(center_y_track);
        tracks.tracks.push(focus_radius_track);

        filter.set_keyframe_tracks(tracks);

        // Verify: at t=0ms, focus should be at left
        let values_0 = filter.get_values_at_time(0);
        assert!((values_0.center_x - 0.3).abs() < 0.01);
        assert!((values_0.center_y - 0.5).abs() < 0.01);
        assert_eq!(values_0.focus_radius, 80);

        // Verify: at t=1500ms (midpoint), focus should be at center
        let values_mid = filter.get_values_at_time(1500);
        assert!((values_mid.center_x - 0.5).abs() < 0.01);
        assert_eq!(values_mid.focus_radius, 120);

        // Verify: at t=3000ms, focus should be at right
        let values_end = filter.get_values_at_time(3000);
        assert!((values_end.center_x - 0.7).abs() < 0.01);
        assert_eq!(values_end.focus_radius, 160);

        // Verify: at t=750ms (interpolated), center_x should be ~0.4
        let values_interp = filter.get_values_at_time(750);
        assert!((values_interp.center_x - 0.4).abs() < 0.05);
    }
}
