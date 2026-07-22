use crate::{
    Result,
    filters::{
        progress_ratio_from_offset,
        traits::{EffectPosition, VideoData, VideoFilter},
    },
    tracks::video_frame_cache::VideoImage,
};
use image::{Rgba, RgbaImage, imageops};
use rayon::prelude::*;
use std::time::Duration;

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    num_enum::TryFromPrimitive,
    num_enum::IntoPrimitive,
)]
#[repr(u8)]
pub enum SlideDirection {
    FromLeft = 0, // 从左滑入
    FromRight,    // 从右滑入
    FromTop,      // 从上滑入
    FromBottom,   // 从下滑入
}

impl Default for SlideDirection {
    fn default() -> Self {
        Self::FromLeft
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SlideFilter {
    pub direction: SlideDirection,
    pub position: EffectPosition,
    pub duration: Duration,
}

impl Default for SlideFilter {
    fn default() -> Self {
        Self {
            direction: SlideDirection::FromLeft,
            position: EffectPosition::Start,
            duration: Duration::from_secs(1),
        }
    }
}

impl SlideFilter {
    pub const NAME: &'static str = "slide";

    pub fn new(direction: SlideDirection, position: EffectPosition, duration: Duration) -> Self {
        Self {
            direction,
            position,
            duration,
        }
    }
}

impl VideoFilter for SlideFilter {
    crate::impl_default_video_filter!(SlideFilter);

    fn take_effect_in_layer_frame(&self) -> bool {
        false // Operate on image_for_composite stream, consistent with transform
    }

    fn apply(&self, data: &mut VideoData) -> Result<()> {
        let canvas_width = data.config.output_width;
        let canvas_height = data.config.output_height;
        let frame_time_offset = data.relative_timeline_offset;
        let segment_duration = data.from_segment.duration;

        for frame in data.frames.iter_mut() {
            if let VideoImage::Image { buffer, .. } = frame {
                // Ensure buffer is canvas-sized (decoded frames may be smaller than canvas)
                if buffer.width() != canvas_width || buffer.height() != canvas_height {
                    let mut canvas = RgbaImage::new(canvas_width, canvas_height);
                    let x = (canvas_width.saturating_sub(buffer.width())) / 2;
                    let y = (canvas_height.saturating_sub(buffer.height())) / 2;
                    imageops::overlay(&mut canvas, buffer, x as i64, y as i64);
                    *buffer = canvas;
                }

                // Use canvas dimensions for animation calculations
                let width = canvas_width;
                let height = canvas_height;

                let original = buffer.clone();
                let mut result = RgbaImage::new(width, height);

                match self.position {
                    EffectPosition::Start => {
                        // Slide in effect at segment start
                        // Skip if not in effect time range
                        if frame_time_offset > self.duration {
                            continue;
                        }

                        // ratio from 0.0 (effect start) to 1.0 (effect end)
                        let ratio = progress_ratio_from_offset(frame_time_offset, self.duration);

                        match self.direction {
                            SlideDirection::FromLeft => {
                                // Slide from left: at ratio=0 image is at x=-width (hidden), at ratio=1 image is at x=0 (fully visible)
                                let offset = (width as f32 * (1.0 - ratio)).round() as i32;
                                result.par_enumerate_pixels_mut().for_each(|(x, y, pixel)| {
                                    let src_x = x as i32 + offset;
                                    if src_x < 0 || src_x >= width as i32 {
                                        *pixel = Rgba([0, 0, 0, 0]);
                                    } else {
                                        *pixel = *original.get_pixel(src_x as u32, y);
                                    }
                                });
                            }
                            SlideDirection::FromRight => {
                                // Slide from right: at ratio=0 image is at x=+width (hidden), at ratio=1 image is at x=0 (fully visible)
                                let offset = (width as f32 * (1.0 - ratio)).round() as i32;
                                result.par_enumerate_pixels_mut().for_each(|(x, y, pixel)| {
                                    let src_x = x as i32 - offset;
                                    if src_x < 0 || src_x >= width as i32 {
                                        *pixel = Rgba([0, 0, 0, 0]);
                                    } else {
                                        *pixel = *original.get_pixel(src_x as u32, y);
                                    }
                                });
                            }
                            SlideDirection::FromTop => {
                                // Slide from top: at ratio=0 image is at y=-height (hidden), at ratio=1 image is at y=0 (fully visible)
                                let offset = (height as f32 * (1.0 - ratio)).round() as i32;
                                result.par_enumerate_pixels_mut().for_each(|(x, y, pixel)| {
                                    let src_y = y as i32 + offset;
                                    if src_y < 0 || src_y >= height as i32 {
                                        *pixel = Rgba([0, 0, 0, 0]);
                                    } else {
                                        *pixel = *original.get_pixel(x, src_y as u32);
                                    }
                                });
                            }
                            SlideDirection::FromBottom => {
                                // Slide from bottom: at ratio=0 image is at y=+height (hidden), at ratio=1 image is at y=0 (fully visible)
                                let offset = (height as f32 * (1.0 - ratio)).round() as i32;
                                result.par_enumerate_pixels_mut().for_each(|(x, y, pixel)| {
                                    let src_y = y as i32 - offset;
                                    if src_y < 0 || src_y >= height as i32 {
                                        *pixel = Rgba([0, 0, 0, 0]);
                                    } else {
                                        *pixel = *original.get_pixel(x, src_y as u32);
                                    }
                                });
                            }
                        }
                    }
                    EffectPosition::End => {
                        // Slide out effect at segment end
                        let time_until_end = segment_duration.saturating_sub(frame_time_offset);

                        // Skip if not in effect time range
                        if time_until_end > self.duration {
                            continue;
                        }

                        // ratio from 1.0 (effect start, image visible) to 0.0 (effect end, image hidden)
                        let ratio = progress_ratio_from_offset(time_until_end, self.duration);

                        // Force complete slide out near segment end to avoid incomplete final frames
                        let is_last_frame = time_until_end < Duration::from_millis(100);

                        match self.direction {
                            SlideDirection::FromLeft => {
                                // Slide out to left: at ratio=1 image is fully visible, at ratio=0 image slides to x=-width (hidden)
                                // offset increases as ratio decreases
                                let offset = if is_last_frame {
                                    width as i32
                                } else {
                                    (width as f32 * (1.0 - ratio)).round() as i32
                                };
                                result.par_enumerate_pixels_mut().for_each(|(x, y, pixel)| {
                                    let src_x = x as i32 + offset;
                                    if src_x < 0 || src_x >= width as i32 {
                                        *pixel = Rgba([0, 0, 0, 0]);
                                    } else {
                                        *pixel = *original.get_pixel(src_x as u32, y);
                                    }
                                });
                            }
                            SlideDirection::FromRight => {
                                // Slide out to right: at ratio=1 image is fully visible, at ratio=0 image slides to x=+width (hidden)
                                let offset = if is_last_frame {
                                    width as i32
                                } else {
                                    (width as f32 * (1.0 - ratio)).round() as i32
                                };
                                result.par_enumerate_pixels_mut().for_each(|(x, y, pixel)| {
                                    let src_x = x as i32 - offset;
                                    if src_x < 0 || src_x >= width as i32 {
                                        *pixel = Rgba([0, 0, 0, 0]);
                                    } else {
                                        *pixel = *original.get_pixel(src_x as u32, y);
                                    }
                                });
                            }
                            SlideDirection::FromTop => {
                                // Slide out to top: at ratio=1 image is fully visible, at ratio=0 image slides to y=-height (hidden)
                                let offset = if is_last_frame {
                                    height as i32
                                } else {
                                    (height as f32 * (1.0 - ratio)).round() as i32
                                };
                                result.par_enumerate_pixels_mut().for_each(|(x, y, pixel)| {
                                    let src_y = y as i32 + offset;
                                    if src_y < 0 || src_y >= height as i32 {
                                        *pixel = Rgba([0, 0, 0, 0]);
                                    } else {
                                        *pixel = *original.get_pixel(x, src_y as u32);
                                    }
                                });
                            }
                            SlideDirection::FromBottom => {
                                // Slide out to bottom: at ratio=1 image is fully visible, at ratio=0 image slides to y=+height (hidden)
                                let offset = if is_last_frame {
                                    height as i32
                                } else {
                                    (height as f32 * (1.0 - ratio)).round() as i32
                                };
                                result.par_enumerate_pixels_mut().for_each(|(x, y, pixel)| {
                                    let src_y = y as i32 - offset;
                                    if src_y < 0 || src_y >= height as i32 {
                                        *pixel = Rgba([0, 0, 0, 0]);
                                    } else {
                                        *pixel = *original.get_pixel(x, src_y as u32);
                                    }
                                });
                            }
                        }
                    }
                }

                *buffer = result;
            }
        }

        Ok(())
    }
}
