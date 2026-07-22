use crate::{
    Result,
    filters::{
        interpolation::get_float2_at_time,
        keyframe::{AnimatableProperty, KeyframeTracks, KeyframeValue},
        traits::{VideoData, VideoFilter},
    },
    tracks::video_frame_cache::VideoImage,
};
use image::RgbaImage;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MosaicFilter {
    pub left: f32,       // 左边缘偏移(0, 1)
    pub top: f32,        // 上边缘偏移(0, 1)
    pub width: f32,      // 马赛克区域宽度（0, 1）
    pub height: f32,     // 马赛克区域高度（0, 1）
    pub block_size: u32, // 马赛克块大小（像素）
    #[serde(skip_serializing_if = "KeyframeTracks::is_empty", default)]
    pub keyframe_tracks: KeyframeTracks,
}

impl Default for MosaicFilter {
    fn default() -> Self {
        Self {
            left: 0.0,
            top: 0.0,
            width: 1.0,
            height: 1.0,
            block_size: 10,
            keyframe_tracks: KeyframeTracks::default(),
        }
    }
}

impl MosaicFilter {
    pub const NAME: &'static str = "mosaic";

    pub fn new(left: f32, top: f32, width: f32, height: f32, block_size: u32) -> Self {
        Self {
            left,
            top,
            width,
            height,
            block_size: block_size.max(1),
            keyframe_tracks: KeyframeTracks::default(),
        }
    }

    /// Get the animatable properties for this filter
    pub fn animatable_properties() -> Vec<AnimatableProperty> {
        vec![
            AnimatableProperty::float2("region", "Position", 0.0, 1.0, 0.0, 0.0), // left, top
            AnimatableProperty::float2("size", "Size", 0.0, 1.0, 1.0, 1.0),       // width, height
        ]
    }

    /// Get interpolated values at a specific time
    fn get_values_at_time(&self, time_ms: i64) -> MosaicValues {
        let (left, top) = self
            .keyframe_tracks
            .get_track("region")
            .map(|track| get_float2_at_time(track, time_ms, self.left, self.top))
            .unwrap_or((self.left, self.top));

        let (width, height) = self
            .keyframe_tracks
            .get_track("size")
            .map(|track| get_float2_at_time(track, time_ms, self.width, self.height))
            .unwrap_or((self.width, self.height));

        MosaicValues {
            left: left.clamp(0.0, 1.0),
            top: top.clamp(0.0, 1.0),
            width: width.clamp(0.0, 1.0),
            height: height.clamp(0.0, 1.0),
            block_size: self.block_size,
        }
    }

    fn apply_mosaic_with_values(values: &MosaicValues, image: &mut RgbaImage) {
        let img_width = image.width();
        let img_height = image.height();

        // Convert normalized values to pixel values
        let px_left = (values.left * img_width as f32).round() as u32;
        let px_top = (values.top * img_height as f32).round() as u32;
        let px_width = (values.width * img_width as f32).round() as u32;
        let px_height = (values.height * img_height as f32).round() as u32;

        // Clamp values
        let px_left = px_left.min(img_width);
        let px_top = px_top.min(img_height);
        let px_right = (px_left + px_width).min(img_width);
        let px_bottom = (px_top + px_height).min(img_height);

        if px_right <= px_left || px_bottom <= px_top || values.block_size == 0 {
            return;
        }

        let block_size = values
            .block_size
            .min(px_right - px_left)
            .min(px_bottom - px_top)
            .max(1);

        // Apply mosaic effect
        for y in (px_top..px_bottom).step_by(block_size as usize) {
            for x in (px_left..px_right).step_by(block_size as usize) {
                let block_right = (x + block_size).min(px_right);
                let block_bottom = (y + block_size).min(px_bottom);

                // Calculate average color for this block
                let mut sum_r = 0u32;
                let mut sum_g = 0u32;
                let mut sum_b = 0u32;
                let mut count = 0u32;

                for by in y..block_bottom {
                    for bx in x..block_right {
                        let pixel = image.get_pixel(bx, by);
                        sum_r += pixel.0[0] as u32;
                        sum_g += pixel.0[1] as u32;
                        sum_b += pixel.0[2] as u32;
                        count += 1;
                    }
                }

                if count > 0 {
                    let avg_r = (sum_r / count) as u8;
                    let avg_g = (sum_g / count) as u8;
                    let avg_b = (sum_b / count) as u8;

                    // Fill block with average color
                    for by in y..block_bottom {
                        for bx in x..block_right {
                            let pixel = image.get_pixel_mut(bx, by);
                            pixel.0[0] = avg_r;
                            pixel.0[1] = avg_g;
                            pixel.0[2] = avg_b;
                        }
                    }
                }
            }
        }
    }
}

/// Interpolated mosaic values at a specific time
struct MosaicValues {
    left: f32,
    top: f32,
    width: f32,
    height: f32,
    block_size: u32,
}

impl VideoFilter for MosaicFilter {
    crate::impl_default_video_filter!(MosaicFilter);

    fn apply(&self, data: &mut VideoData) -> Result<()> {
        // Calculate current time in milliseconds relative to segment start
        let time_ms = data.relative_timeline_offset.as_millis() as i64;

        // Get interpolated values at current time
        let values = self.get_values_at_time(time_ms);

        for frame in &mut data.frames {
            if let VideoImage::Image { buffer } = frame {
                Self::apply_mosaic_with_values(&values, buffer);
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

        if let Some(track) = tracks.get_track("region")
            && track.keyframes.iter().any(|k| k.time_ms == time_ms)
        {
            tracks.update_keyframe_value(
                "region",
                time_ms,
                KeyframeValue::Float2(self.left, self.top),
            );
            updated = true;
        }

        if let Some(track) = tracks.get_track("size")
            && track.keyframes.iter().any(|k| k.time_ms == time_ms)
        {
            tracks.update_keyframe_value(
                "size",
                time_ms,
                KeyframeValue::Float2(self.width, self.height),
            );
            updated = true;
        }

        updated
    }
}

