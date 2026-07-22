use crate::{
    Result,
    filters::traits::{VideoData, VideoFilter},
    tracks::video_frame_cache::VideoImage,
};
use image::{RgbaImage, imageops};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum FlipDirection {
    Horizontal, // 水平翻转（左右镜像）
    Vertical,   // 垂直翻转（上下镜像）
    Both,       // 同时水平和垂直翻转
}

impl Default for FlipDirection {
    fn default() -> Self {
        Self::Horizontal
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FlipFilter {
    pub direction: FlipDirection,
}

impl Default for FlipFilter {
    fn default() -> Self {
        Self {
            direction: FlipDirection::Horizontal,
        }
    }
}

impl FlipFilter {
    pub const NAME: &'static str = "flip";

    pub fn new(direction: FlipDirection) -> Self {
        Self { direction }
    }

    fn apply_flip(&self, image: &mut RgbaImage) {
        match self.direction {
            FlipDirection::Horizontal => imageops::flip_horizontal_in_place(image),
            FlipDirection::Vertical => imageops::flip_vertical_in_place(image),
            FlipDirection::Both => {
                imageops::flip_horizontal_in_place(image);
                imageops::flip_vertical_in_place(image);
            }
        }
    }
}

impl VideoFilter for FlipFilter {
    crate::impl_default_video_filter!(FlipFilter);

    fn apply(&self, data: &mut VideoData) -> Result<()> {
        for frame in &mut data.frames {
            if let VideoImage::Image { buffer, .. } = frame {
                self.apply_flip(buffer);
            }
        }
        Ok(())
    }
}
