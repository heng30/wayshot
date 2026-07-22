use crate::{
    Result,
    filters::{
        interpolation::get_float_at_time,
        keyframe::{AnimatableProperty, KeyframeTracks, KeyframeValue},
        traits::{VideoData, VideoFilter},
    },
    tracks::video_frame_cache::VideoImage,
};
use image::{Rgba, RgbaImage};
use rayon::prelude::*;

/// Wave type for the distortion effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WaveType {
    /// Horizontal waves (waves propagate along X axis)
    #[default]
    Horizontal,
    /// Vertical waves (waves propagate along Y axis)
    Vertical,
    /// Radial waves (waves radiate from center point outward)
    Radial,
    /// Concentric waves (pond ripple effect, circular waves from center)
    Concentric,
}

impl WaveType {
    pub fn name(&self) -> &'static str {
        match self {
            WaveType::Horizontal => "horizontal",
            WaveType::Vertical => "vertical",
            WaveType::Radial => "radial",
            WaveType::Concentric => "concentric",
        }
    }

    pub fn all_types() -> &'static [WaveType] {
        &[
            WaveType::Horizontal,
            WaveType::Vertical,
            WaveType::Radial,
            WaveType::Concentric,
        ]
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "horizontal" => Some(WaveType::Horizontal),
            "vertical" => Some(WaveType::Vertical),
            "radial" => Some(WaveType::Radial),
            "concentric" => Some(WaveType::Concentric),
            _ => None,
        }
    }
}

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
#[serde(default)]
#[non_exhaustive]
pub struct WaveFilter {
    /// Wave amplitude in pixels (displacement amount). Range: 0.0-100.0.
    #[derivative(Default(value = "10.0"))]
    pub amplitude: f32,

    /// Wave frequency (cycles per normalized unit). Range: 0.1-10.0.
    /// Higher values create denser waves.
    #[derivative(Default(value = "2.0"))]
    pub frequency: f32,

    /// Wave animation speed. Range: 0.0-10.0.
    /// 0.0 = static waves, higher values animate faster.
    #[derivative(Default(value = "1.0"))]
    pub speed: f32,

    /// Initial phase offset in degrees (0.0-360.0).
    #[derivative(Default(value = "0.0"))]
    pub phase: f32,

    /// Wave type (horizontal/vertical/radial/concentric).
    #[derivative(Default(value = "WaveType::Horizontal"))]
    pub wave_type: WaveType,

    /// Center X position for radial/concentric waves (normalized 0.0-1.0).
    #[derivative(Default(value = "0.5"))]
    pub center_x: f32,

    /// Center Y position for radial/concentric waves (normalized 0.0-1.0).
    #[derivative(Default(value = "0.5"))]
    pub center_y: f32,

    /// Keyframe tracks for animatable properties.
    #[serde(skip_serializing_if = "KeyframeTracks::is_empty", default)]
    pub keyframe_tracks: KeyframeTracks,
}

impl WaveFilter {
    pub const NAME: &'static str = "wave";

    pub fn new(amplitude: f32, frequency: f32, wave_type: WaveType) -> Self {
        Self {
            amplitude: amplitude.clamp(0.0, 100.0),
            frequency: frequency.clamp(0.1, 10.0),
            speed: 1.0,
            phase: 0.0,
            wave_type,
            center_x: 0.5,
            center_y: 0.5,
            keyframe_tracks: KeyframeTracks::default(),
        }
    }

    pub fn animatable_properties() -> Vec<AnimatableProperty> {
        vec![
            AnimatableProperty::float("amplitude", "Amplitude", 0.0, 100.0, 10.0),
            AnimatableProperty::float("frequency", "Frequency", 0.1, 10.0, 2.0),
            AnimatableProperty::float("speed", "Speed", 0.0, 10.0, 1.0),
            AnimatableProperty::float("center_x", "Center X", 0.0, 1.0, 0.5),
            AnimatableProperty::float("center_y", "Center Y", 0.0, 1.0, 0.5),
        ]
    }

    fn get_value_at_time(&self, time_ms: i64, property: &str, default: f32) -> f32 {
        self.keyframe_tracks
            .get_track(property)
            .map(|track| get_float_at_time(track, time_ms, default))
            .unwrap_or(default)
    }

    fn get_center_at_time(&self, time_ms: i64) -> (f32, f32) {
        let center_x = self
            .keyframe_tracks
            .get_track("center_x")
            .map(|track| get_float_at_time(track, time_ms, self.center_x))
            .unwrap_or(self.center_x);

        let center_y = self
            .keyframe_tracks
            .get_track("center_y")
            .map(|track| get_float_at_time(track, time_ms, self.center_y))
            .unwrap_or(self.center_y);

        (center_x, center_y)
    }

    /// Sample pixel from source image using bilinear interpolation.
    fn sample_bilinear(buffer: &RgbaImage, x: f32, y: f32) -> Rgba<u8> {
        let width = buffer.width();
        let height = buffer.height();

        // Clamp coordinates to valid range with a small margin for interpolation
        let x = x.clamp(0.0, (width - 1) as f32);
        let y = y.clamp(0.0, (height - 1) as f32);

        // Get integer coordinates for the 4 corners
        let x0 = x.floor() as u32;
        let y0 = y.floor() as u32;
        let x1 = (x0 + 1).min(width - 1);
        let y1 = (y0 + 1).min(height - 1);

        // Get interpolation weights
        let dx = x - x0 as f32;
        let dy = y - y0 as f32;

        // Get pixels at 4 corners
        let p00 = buffer.get_pixel(x0, y0);
        let p01 = buffer.get_pixel(x1, y0);
        let p10 = buffer.get_pixel(x0, y1);
        let p11 = buffer.get_pixel(x1, y1);

        // Bilinear interpolation for each channel
        fn interpolate(v00: u8, v01: u8, v10: u8, v11: u8, dx: f32, dy: f32) -> u8 {
            let v0 = v00 as f32 * (1.0 - dx) + v01 as f32 * dx;
            let v1 = v10 as f32 * (1.0 - dx) + v11 as f32 * dx;
            ((v0 * (1.0 - dy) + v1 * dy).clamp(0.0, 255.0)) as u8
        }

        Rgba([
            interpolate(p00[0], p01[0], p10[0], p11[0], dx, dy),
            interpolate(p00[1], p01[1], p10[1], p11[1], dx, dy),
            interpolate(p00[2], p01[2], p10[2], p11[2], dx, dy),
            interpolate(p00[3], p01[3], p10[3], p11[3], dx, dy),
        ])
    }

    fn apply_wave(
        wave_type: WaveType,
        amplitude: f32,
        frequency: f32,
        phase_offset: f32,
        center_x: f32,
        center_y: f32,
        buffer: &mut RgbaImage,
    ) -> Result<()> {
        let width = buffer.width() as f32;
        let height = buffer.height() as f32;

        // Create a copy of the original image for sampling
        let source = buffer.clone();

        // Process rows in parallel
        let rows: Vec<Vec<[u8; 4]>> = (0..buffer.height())
            .into_par_iter()
            .map(|y| {
                (0..buffer.width())
                    .map(|x| {
                        let px = x as f32;
                        let py = y as f32;

                        // Calculate source position with wave displacement
                        let (src_x, src_y) = Self::calculate_displacement_static(
                            wave_type,
                            px,
                            py,
                            width,
                            height,
                            amplitude,
                            frequency,
                            phase_offset,
                            center_x,
                            center_y,
                        );

                        // Sample from source using bilinear interpolation
                        Self::sample_bilinear(&source, src_x, src_y).0
                    })
                    .collect()
            })
            .collect();

        // Apply results back to buffer
        for (y_idx, row) in rows.iter().enumerate() {
            for (x_idx, pixel_data) in row.iter().enumerate() {
                let pixel = buffer.get_pixel_mut(x_idx as u32, y_idx as u32);
                pixel.0 = *pixel_data;
            }
        }

        Ok(())
    }

    /// Calculate the displacement for a given position based on wave type (static version).
    fn calculate_displacement_static(
        wave_type: WaveType,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        amplitude: f32,
        frequency: f32,
        phase_offset: f32,
        center_x: f32,
        center_y: f32,
    ) -> (f32, f32) {
        // Normalize coordinates for frequency calculation
        let nx = x / width;
        let ny = y / height;

        match wave_type {
            WaveType::Horizontal => {
                // Waves propagate along X axis, displacement along Y axis
                let wave_value = (frequency * nx * std::f32::consts::TAU + phase_offset).sin();
                let offset_y = amplitude * wave_value;
                (x, y + offset_y)
            }
            WaveType::Vertical => {
                // Waves propagate along Y axis, displacement along X axis
                let wave_value = (frequency * ny * std::f32::consts::TAU + phase_offset).sin();
                let offset_x = amplitude * wave_value;
                (x + offset_x, y)
            }
            WaveType::Radial => {
                // Radial waves: displacement based on angle from center
                let cx = center_x * width;
                let cy = center_y * height;
                let dx = x - cx;
                let dy = y - cy;

                if dx == 0.0 && dy == 0.0 {
                    (x, y)
                } else {
                    let angle = dy.atan2(dx);
                    let dist = (dx * dx + dy * dy).sqrt();

                    // Wave propagates outward from center
                    let wave_value = (frequency * dist / width + phase_offset).sin();
                    let displacement = amplitude * wave_value;

                    // Displacement along the radial direction
                    let offset_x = displacement * angle.cos();
                    let offset_y = displacement * angle.sin();

                    (x + offset_x, y + offset_y)
                }
            }
            WaveType::Concentric => {
                // Concentric waves (pond ripple): circular displacement from center
                let cx = center_x * width;
                let cy = center_y * height;
                let dx = x - cx;
                let dy = y - cy;

                let dist = (dx * dx + dy * dy).sqrt();

                // Wave propagates outward from center, creating concentric rings
                let wave_value = (frequency * dist / width + phase_offset).sin();
                let displacement = amplitude * wave_value;

                if dist == 0.0 {
                    (x, y)
                } else {
                    // Normalize direction
                    let nx_dir = dx / dist;
                    let ny_dir = dy / dist;

                    // Apply displacement along radial direction
                    (x + displacement * nx_dir, y + displacement * ny_dir)
                }
            }
        }
    }
}

impl VideoFilter for WaveFilter {
    crate::impl_default_video_filter!(WaveFilter);

    fn apply(&self, data: &mut VideoData) -> Result<()> {
        let time_ms = data.relative_timeline_offset.as_millis() as i64;

        // Get animated values
        let amplitude = self
            .get_value_at_time(time_ms, "amplitude", self.amplitude)
            .clamp(0.0, 100.0);
        let frequency = self
            .get_value_at_time(time_ms, "frequency", self.frequency)
            .clamp(0.1, 10.0);
        let speed = self
            .get_value_at_time(time_ms, "speed", self.speed)
            .clamp(0.0, 10.0);

        // Get animated center position
        let (center_x, center_y) = self.get_center_at_time(time_ms);

        // Calculate phase offset with animated speed
        // We need to recalculate phase with the animated speed value
        let static_phase = self.phase * std::f32::consts::PI / 180.0;
        let animation_phase = speed * (time_ms as f32 / 1000.0) * std::f32::consts::TAU;
        let phase_offset = static_phase + animation_phase;

        // Skip if amplitude is zero (no effect)
        if amplitude < 0.1 {
            return Ok(());
        }

        for frame in data.frames.iter_mut() {
            if let VideoImage::Image { buffer, .. } = frame {
                Self::apply_wave(
                    self.wave_type,
                    amplitude,
                    frequency,
                    phase_offset,
                    center_x,
                    center_y,
                    buffer,
                )?;
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
            ("amplitude", self.amplitude),
            ("frequency", self.frequency),
            ("speed", self.speed),
            ("center_x", self.center_x),
            ("center_y", self.center_y),
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

