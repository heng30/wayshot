use derivative::Derivative;
use derive_setters::Setters;
use fast_image_resize::{PixelType, ResizeAlg, Resizer, images::Image as FastImage};
use image::{ImageBuffer, Luma, Rgba, RgbaImage};

use crate::{CameraError, CameraResult};

#[derive(Debug, Clone, Copy)]
pub enum Shape {
    Circle(ShapeCircle),
    Rectangle(ShapeRectangle),
}

#[derive(Debug, Clone, Copy, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct ShapeBase {
    /// Position in background_image [0, 1] range
    #[derivative(Default(value = "(0.5, 0.5)"))]
    pub pos: (f32, f32),

    /// Border width in pixels
    #[derivative(Default(value = "2"))]
    pub border_width: u32,

    /// Border color (RGBA)
    #[derivative(Default(value = "Rgba([0, 0, 0, 255])"))]
    pub border_color: Rgba<u8>,

    /// Zoom level (1.0 = original size, 2.0 = 2x zoom, etc.)
    #[derivative(Default(value = "1.0"))]
    pub zoom: f32,

    /// Clip position in source image [0, 1] range
    /// (0.5, 0.5) = center, (0.0, 0.0) = top-left, (1.0, 1.0) = bottom-right
    #[derivative(Default(value = "(0.5, 0.5)"))]
    pub clip_pos: (f32, f32),
}

#[derive(Debug, Clone, Copy, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct ShapeCircle {
    pub base: ShapeBase,

    #[derivative(Default(value = "50"))]
    pub radius: u32,
}

#[derive(Debug, Clone, Copy, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct ShapeRectangle {
    pub base: ShapeBase,

    /// Size (width, height) in pixels
    #[derivative(Default(value = "(100, 100)"))]
    pub size: (u32, u32),
}

pub fn mix_images(
    background_image: RgbaImage,
    camera_image: RgbaImage,
    mix_area: Shape,
) -> CameraResult<RgbaImage> {
    match mix_area {
        Shape::Circle(circle) => mix_images_circle(background_image, camera_image, circle),
        Shape::Rectangle(rect) => mix_images_rectangle(background_image, camera_image, rect),
    }
}

fn mix_images_circle(
    mut background: RgbaImage,
    camera_image: RgbaImage,
    circle: ShapeCircle,
) -> CameraResult<RgbaImage> {
    let (bg_width, bg_height) = background.dimensions();
    let radius = circle.radius;
    let diameter = (radius * 2) as u32;
    let center_x =
        ((circle.base.pos.0 * bg_width as f32) as u32).clamp(radius, bg_width as u32 - radius);
    let center_y =
        ((circle.base.pos.1 * bg_height as f32) as u32).clamp(radius, bg_height as u32 - radius);

    let croped_camera = crop_image(
        camera_image,
        diameter,
        diameter,
        circle.base.zoom,
        circle.base.clip_pos,
    )?;

    // Calculate the top-left position where we'll draw
    let start_x = (center_x - radius).max(0) as u32;
    let start_y = (center_y - radius).max(0) as u32;

    // Create circular mask
    let mut mask = ImageBuffer::new(diameter, diameter);
    let (cx, cy, r) = (radius as f32, radius as f32, radius as f32);

    for y in 0..diameter {
        for x in 0..diameter {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            let dist = dx * dx + dy * dy;

            if dist <= r * r {
                mask.put_pixel(x, y, Luma([255]));
            } else {
                mask.put_pixel(x, y, Luma([0]));
            }
        }
    }

    // Blend the camera image onto the background using the mask
    for y in 0..diameter {
        for x in 0..diameter {
            let bg_x = start_x + x;
            let bg_y = start_y + y;

            if bg_x >= bg_width || bg_y >= bg_height {
                continue;
            }

            let mask_val = mask.get_pixel(x, y)[0];
            if mask_val == 255 {
                let cam_pixel = croped_camera.get_pixel(x, y);
                background.put_pixel(bg_x, bg_y, *cam_pixel);
            }
        }
    }

    if circle.base.border_width > 0 {
        draw_circle_border(
            &mut background,
            center_x,
            center_y,
            radius,
            circle.base.border_width,
            circle.base.border_color,
        );
    }

    Ok(background)
}

fn mix_images_rectangle(
    mut background: RgbaImage,
    camera_image: RgbaImage,
    rect: ShapeRectangle,
) -> CameraResult<RgbaImage> {
    let (bg_width, bg_height) = background.dimensions();
    let x = (rect.base.pos.0 * bg_width as f32) as u32;
    let y = (rect.base.pos.1 * bg_height as f32) as u32;
    let (width, height) = rect.size;

    let croped_camera = crop_image(
        camera_image,
        width,
        height,
        rect.base.zoom,
        rect.base.clip_pos,
    )?;

    for cam_y in 0..height {
        for cam_x in 0..width {
            let bg_x = x + cam_x;
            let bg_y = y + cam_y;

            if bg_x >= bg_width || bg_y >= bg_height {
                continue;
            }

            let cam_pixel = croped_camera.get_pixel(cam_x, cam_y);
            background.put_pixel(bg_x, bg_y, *cam_pixel);
        }
    }

    if rect.base.border_width > 0 {
        draw_rectangle_border(
            &mut background,
            x,
            y,
            width,
            height,
            rect.base.border_width,
            rect.base.border_color,
        );
    }

    Ok(background)
}

/// Scale and crop an image with zoom level using fast_image_resize
/// First scales by zoom factor, then crops from clip_pos to target size
/// Does NOT maintain aspect ratio - crops to exact target dimensions
fn crop_image(
    image: RgbaImage,
    target_width: u32,
    target_height: u32,
    zoom: f32,
    clip_pos: (f32, f32), // [0.0, 1.0]
) -> CameraResult<RgbaImage> {
    let (img_width, img_height) = image.dimensions();
    let scaled_width = ((img_width as f32) * zoom).round() as u32;
    let scaled_height = ((img_height as f32) * zoom).round() as u32;

    if scaled_width == target_width && scaled_height == target_height && zoom == 1.0 {
        return Ok(image);
    }

    let mut resized_src = vec![0u8; (scaled_width * scaled_height * 4) as usize];
    let fast_image =
        FastImage::from_vec_u8(img_width, img_height, image.into_raw(), PixelType::U8x4)?;

    let mut resized_image = FastImage::from_slice_u8(
        scaled_width,
        scaled_height,
        &mut resized_src,
        PixelType::U8x4,
    )?;

    let resize_options = fast_image_resize::ResizeOptions::new().resize_alg(
        ResizeAlg::Convolution(fast_image_resize::FilterType::Lanczos3),
    );

    Resizer::new().resize(&fast_image, &mut resized_image, &resize_options)?;

    let max_crop_x = if scaled_width > target_width {
        scaled_width - target_width
    } else {
        0
    };

    let max_crop_y = if scaled_height > target_height {
        scaled_height - target_height
    } else {
        0
    };

    let clip_x = clip_pos.0.max(0.0).min(1.0);
    let clip_y = clip_pos.1.max(0.0).min(1.0);
    let crop_x = (scaled_width as f32 * clip_x).clamp(0.0, max_crop_x as f32) as u32;
    let crop_y = (scaled_height as f32 * clip_y).clamp(0.0, max_crop_y as f32) as u32;

    let actual_crop_width = target_width.min(scaled_width);
    let actual_crop_height = target_height.min(scaled_height);
    let mut cropped_buffer = vec![0u8; (actual_crop_width * actual_crop_height * 4) as usize];

    let src_stride = scaled_width as usize * 4;
    let dst_stride = actual_crop_width as usize * 4;
    let crop_x_offset = crop_x as usize * 4;
    let crop_y_offset = crop_y as usize;

    for y in 0..actual_crop_height as usize {
        let src_row_start = (crop_y_offset + y) * src_stride + crop_x_offset;
        let dst_row_start = y * dst_stride;
        let row_bytes = dst_stride;

        cropped_buffer[dst_row_start..dst_row_start + row_bytes]
            .copy_from_slice(&resized_image.buffer()[src_row_start..src_row_start + row_bytes]);
    }

    // If the cropped image is smaller than target, pad with black
    if actual_crop_width < target_width || actual_crop_height < target_height {
        let mut result = ImageBuffer::new(target_width, target_height);
        for y in 0..target_height {
            for x in 0..target_width {
                if x < actual_crop_width && y < actual_crop_height {
                    let idx = ((y * actual_crop_width + x) * 4) as usize;
                    let r = cropped_buffer[idx];
                    let g = cropped_buffer[idx + 1];
                    let b = cropped_buffer[idx + 2];
                    let a = cropped_buffer[idx + 3];
                    result.put_pixel(x, y, Rgba([r, g, b, a]));
                } else {
                    result.put_pixel(x, y, Rgba([0, 0, 0, 255]));
                }
            }
        }
        Ok(result)
    } else {
        RgbaImage::from_raw(actual_crop_width, actual_crop_height, cropped_buffer)
            .ok_or(CameraError::ImageError("New RgbaImage failed".to_string()))
    }
}

fn draw_circle_border(
    image: &mut RgbaImage,
    center_x: u32,
    center_y: u32,
    radius: u32,
    border_width: u32,
    border_color: Rgba<u8>,
) {
    let (width, height) = image.dimensions();

    for r_offset in 0..border_width {
        let current_radius = radius - r_offset;
        if current_radius <= 0 {
            break;
        }

        for angle in 0..360 {
            let rad = (angle as f32 * std::f32::consts::PI) / 180.0;
            let x = center_x as i32 + (current_radius as f32 * rad.cos()).round() as i32;
            let y = center_y as i32 + (current_radius as f32 * rad.sin()).round() as i32;

            if x >= 0 && y >= 0 && (x as u32) < width && (y as u32) < height {
                image.put_pixel(x as u32, y as u32, border_color);
            }
        }
    }
}

fn draw_rectangle_border(
    image: &mut RgbaImage,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    border_width: u32,
    border_color: Rgba<u8>,
) {
    let (img_width, img_height) = image.dimensions();

    for bw in 0..border_width {
        // Top edge
        for bx in x..=(x + width - 1).min(img_width - 1) {
            let by = y + bw;
            if by < img_height {
                image.put_pixel(bx, by, border_color);
            }
        }
        // Bottom edge
        for bx in x..=(x + width - 1).min(img_width - 1) {
            if bw < height {
                let by = y + height - 1 - bw;
                if by < img_height && by > 0 {
                    image.put_pixel(bx, by, border_color);
                }
            }
        }
        // Left edge
        for by in y..=(y + height - 1).min(img_height - 1) {
            let bx = x + bw;
            if bx < img_width {
                image.put_pixel(bx, by, border_color);
            }
        }
        // Right edge
        for by in y..=(y + height - 1).min(img_height - 1) {
            if bw < width {
                let bx = x + width - 1 - bw;
                if bx < img_width && bx > 0 {
                    image.put_pixel(bx, by, border_color);
                }
            }
        }
    }
}
