use crate::{CameraError, CameraResult};
use derivative::Derivative;
use derive_setters::Setters;
use fast_image_resize::{PixelType, ResizeAlg, Resizer, images::Image as FastImage};
use image::{ImageBuffer, Luma, Pixel, RgbImage, Rgba, RgbaImage};

#[derive(Debug, Clone, Copy)]
pub enum Shape {
    Circle(ShapeCircle),
    Rectangle(ShapeRectangle),
}

enum BorderShape {
    Circle,
    Rectangle { width: u32, height: u32 },
}

#[derive(Debug, Clone, Copy)]
pub enum MixPositionWithPadding {
    TopLeft((u32, u32)),     // top padding and left padding
    TopRight((u32, u32)),    // top padding and right padding
    BottomLeft((u32, u32)),  // bottom padding and left padding
    BottomRight((u32, u32)), // bottom padding and right padding
}

#[derive(Debug, Clone, Copy, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct ShapeBase {
    /// Position of the shape on the background image
    /// Specifies padding from the background edges
    #[derivative(Default(value = "MixPositionWithPadding::BottomRight((32, 32))"))]
    pub pos: MixPositionWithPadding,

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

    #[derivative(Default(value = "(100, 100)"))]
    pub size: (u32, u32),
}

pub fn mix_images(
    background_image: RgbaImage,
    camera_image: RgbaImage,
    mix_area: Shape,
) -> CameraResult<RgbaImage> {
    mix_images_impl(background_image, camera_image, mix_area)
}

pub fn mix_images_rgb(
    background_image: RgbImage,
    camera_image: RgbImage,
    mix_area: Shape,
) -> CameraResult<RgbImage> {
    mix_images_impl(background_image, camera_image, mix_area)
}

fn mix_images_impl<P>(
    background: ImageBuffer<P, Vec<u8>>,
    camera_image: ImageBuffer<P, Vec<u8>>,
    mix_area: Shape,
) -> CameraResult<ImageBuffer<P, Vec<u8>>>
where
    P: Pixel<Subpixel = u8> + Copy,
{
    match mix_area {
        Shape::Circle(circle) => mix_images_circle_impl(background, camera_image, circle),
        Shape::Rectangle(rect) => mix_images_rectangle_impl(background, camera_image, rect),
    }
}

fn mix_images_circle_impl<P>(
    mut background: ImageBuffer<P, Vec<u8>>,
    camera_image: ImageBuffer<P, Vec<u8>>,
    circle: ShapeCircle,
) -> CameraResult<ImageBuffer<P, Vec<u8>>>
where
    P: Pixel<Subpixel = u8> + Copy,
{
    let (bg_width, bg_height) = background.dimensions();
    let radius = circle.radius;
    let diameter = (radius * 2) as u32;

    // Calculate center position based on MixPositionWithPadding
    let (center_x, center_y) = match circle.base.pos {
        MixPositionWithPadding::TopLeft((pad_top, pad_left)) => {
            let cx = (pad_left + radius).min(bg_width - radius);
            let cy = (pad_top + radius).min(bg_height - radius);
            (cx.max(radius), cy.max(radius))
        }
        MixPositionWithPadding::TopRight((pad_top, pad_right)) => {
            let cx = bg_width.saturating_sub(pad_right + radius).max(radius);
            let cy = (pad_top + radius).min(bg_height - radius);
            (cx.min(bg_width - radius), cy.max(radius))
        }
        MixPositionWithPadding::BottomLeft((pad_bottom, pad_left)) => {
            let cx = (pad_left + radius).min(bg_width - radius);
            let cy = bg_height.saturating_sub(pad_bottom + radius).max(radius);
            (cx.max(radius), cy.min(bg_height - radius))
        }
        MixPositionWithPadding::BottomRight((pad_bottom, pad_right)) => {
            let cx = bg_width.saturating_sub(pad_right + radius).max(radius);
            let cy = bg_height.saturating_sub(pad_bottom + radius).max(radius);
            (cx.min(bg_width - radius), cy.min(bg_height - radius))
        }
    };

    let cropped_camera = crop_image_by_pixel_type(
        camera_image,
        diameter,
        diameter,
        circle.base.zoom,
        circle.base.clip_pos,
    )?;

    let start_x = (center_x - radius).max(0) as u32;
    let start_y = (center_y - radius).max(0) as u32;

    // Create circular mask (shared logic)
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

    for y in 0..diameter {
        for x in 0..diameter {
            let bg_x = start_x + x;
            let bg_y = start_y + y;

            if bg_x >= bg_width || bg_y >= bg_height {
                continue;
            }

            let mask_val = mask.get_pixel(x, y)[0];
            if mask_val == 255 {
                let cam_pixel = cropped_camera.get_pixel(x, y);
                background.put_pixel(bg_x, bg_y, *cam_pixel);
            }
        }
    }

    if circle.base.border_width > 0 {
        draw_border_by_image_type(
            &mut background,
            center_x,
            center_y,
            radius,
            circle.base.border_width,
            circle.base.border_color,
            BorderShape::Circle,
        );
    }

    Ok(background)
}

fn mix_images_rectangle_impl<P>(
    mut background: ImageBuffer<P, Vec<u8>>,
    camera_image: ImageBuffer<P, Vec<u8>>,
    rect: ShapeRectangle,
) -> CameraResult<ImageBuffer<P, Vec<u8>>>
where
    P: Pixel<Subpixel = u8> + Copy,
{
    let (bg_width, bg_height) = background.dimensions();
    let (width, height) = rect.size;

    // Calculate top-left position based on MixPositionWithPadding
    let (x, y) = match rect.base.pos {
        MixPositionWithPadding::TopLeft((pad_top, pad_left)) => (
            pad_left.min(bg_width.saturating_sub(width)),
            pad_top.min(bg_height.saturating_sub(height)),
        ),
        MixPositionWithPadding::TopRight((pad_top, pad_right)) => {
            let x = bg_width.saturating_sub(width + pad_right);
            let y = pad_top.min(bg_height.saturating_sub(height));
            (x, y)
        }
        MixPositionWithPadding::BottomLeft((pad_bottom, pad_left)) => {
            let x = pad_left.min(bg_width.saturating_sub(width));
            let y = bg_height.saturating_sub(height + pad_bottom);
            (x, y)
        }
        MixPositionWithPadding::BottomRight((pad_bottom, pad_right)) => {
            let x = bg_width.saturating_sub(width + pad_right);
            let y = bg_height.saturating_sub(height + pad_bottom);
            (x, y)
        }
    };

    let cropped_camera = crop_image_by_pixel_type(
        camera_image,
        width,
        height,
        rect.base.zoom,
        rect.base.clip_pos,
    )?;

    let width = width.min(bg_width.saturating_sub(x));
    let height = height.min(bg_height.saturating_sub(y));

    for cam_y in 0..height {
        for cam_x in 0..width {
            let bg_x = x + cam_x;
            let bg_y = y + cam_y;

            let cam_pixel = cropped_camera.get_pixel(cam_x, cam_y);
            background.put_pixel(bg_x, bg_y, *cam_pixel);
        }
    }

    if rect.base.border_width > 0 {
        draw_border_by_image_type(
            &mut background,
            x,
            y,
            width,
            rect.base.border_width,
            rect.base.border_color,
            BorderShape::Rectangle { width, height },
        );
    }

    Ok(background)
}

fn crop_image_by_pixel_type<P>(
    image: ImageBuffer<P, Vec<u8>>,
    target_width: u32,
    target_height: u32,
    zoom: f32,
    clip_pos: (f32, f32),
) -> CameraResult<ImageBuffer<P, Vec<u8>>>
where
    P: Pixel<Subpixel = u8> + Copy,
{
    let pixel_type = if P::CHANNEL_COUNT == 3 {
        PixelType::U8x3
    } else if P::CHANNEL_COUNT == 4 {
        PixelType::U8x4
    } else {
        return Err(CameraError::ImageError(
            "Unsupported pixel type".to_string(),
        ));
    };

    crop_image_generic(
        image,
        target_width,
        target_height,
        zoom,
        clip_pos,
        pixel_type,
    )
}

fn crop_image_generic<P>(
    image: ImageBuffer<P, Vec<u8>>,
    target_width: u32,
    target_height: u32,
    zoom: f32,
    clip_pos: (f32, f32),
    pixel_type: PixelType,
) -> CameraResult<ImageBuffer<P, Vec<u8>>>
where
    P: Pixel<Subpixel = u8> + Copy,
{
    let (img_width, img_height) = image.dimensions();
    let scaled_width = ((img_width as f32) * zoom).round() as u32;
    let scaled_height = ((img_height as f32) * zoom).round() as u32;

    if scaled_width == target_width && scaled_height == target_height && zoom == 1.0 {
        return Ok(image);
    }

    let channel_count = P::CHANNEL_COUNT as usize;
    let mut resized_src = vec![0u8; (scaled_width * scaled_height * channel_count as u32) as usize];
    let fast_image = FastImage::from_vec_u8(img_width, img_height, image.into_raw(), pixel_type)?;

    let mut resized_image =
        FastImage::from_slice_u8(scaled_width, scaled_height, &mut resized_src, pixel_type)?;

    let resize_options = fast_image_resize::ResizeOptions::new().resize_alg(
        ResizeAlg::Convolution(fast_image_resize::FilterType::Lanczos3),
    );

    Resizer::new().resize(&fast_image, &mut resized_image, &resize_options)?;

    let max_crop_x = scaled_width.saturating_sub(target_width);
    let max_crop_y = scaled_height.saturating_sub(target_height);

    let clip_x = clip_pos.0.clamp(0.0, 1.0);
    let clip_y = clip_pos.1.clamp(0.0, 1.0);
    let crop_x = (scaled_width as f32 * clip_x).clamp(0.0, max_crop_x as f32) as u32;
    let crop_y = (scaled_height as f32 * clip_y).clamp(0.0, max_crop_y as f32) as u32;

    let actual_crop_width = target_width.min(scaled_width);
    let actual_crop_height = target_height.min(scaled_height);
    let mut cropped_buffer =
        vec![0u8; (actual_crop_width * actual_crop_height * channel_count as u32) as usize];

    let src_stride = scaled_width as usize * channel_count;
    let dst_stride = actual_crop_width as usize * channel_count;
    let crop_x_offset = crop_x as usize * channel_count;
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

        // Calculate offset position in target canvas based on clip_pos
        let offset_x =
            ((target_width - actual_crop_width) as f32 * clip_pos.0.clamp(0.0, 1.0)).round() as u32;
        let offset_y = ((target_height - actual_crop_height) as f32 * clip_pos.1.clamp(0.0, 1.0))
            .round() as u32;

        for y in 0..target_height {
            for x in 0..target_width {
                // Check if current pixel is within the image area (offset-adjusted)
                let rel_x = x.checked_sub(offset_x);
                let rel_y = y.checked_sub(offset_y);

                if let (Some(rx), Some(ry)) = (rel_x, rel_y) {
                    if rx < actual_crop_width && ry < actual_crop_height {
                        let idx = (ry * actual_crop_width + rx) as usize * channel_count;
                        let mut channels = [0u8; 4];
                        for c in 0..channel_count {
                            channels[c] = cropped_buffer[idx + c];
                        }
                        // Fill remaining channels with 255 (alpha) or 0
                        for c in channel_count..4 {
                            channels[c] = if c == 3 { 255 } else { 0 };
                        }
                        let pixel = P::from_slice(&channels[..channel_count]);
                        result.put_pixel(x, y, *pixel);
                        continue;
                    }
                }

                // Pad with black
                let black = [0u8; 4];
                let pixel = P::from_slice(&black[..channel_count]);
                result.put_pixel(x, y, *pixel);
            }
        }
        Ok(result)
    } else {
        ImageBuffer::from_raw(actual_crop_width, actual_crop_height, cropped_buffer).ok_or(
            CameraError::ImageError("New ImageBuffer failed".to_string()),
        )
    }
}

fn draw_border_by_image_type<P>(
    image: &mut ImageBuffer<P, Vec<u8>>,
    x_or_center_x: u32,
    y_or_center_y: u32,
    radius_or_width: u32,
    border_width: u32,
    border_color: Rgba<u8>,
    shape: BorderShape,
) where
    P: Pixel<Subpixel = u8> + Copy,
{
    match shape {
        BorderShape::Circle => {
            let radius = radius_or_width;
            let center_x = x_or_center_x;
            let center_y = y_or_center_y;
            let (width, height) = image.dimensions();

            // Anti-aliased circle border using supersampling distance field
            let outer_r = radius as f32;
            let inner_r = (radius.saturating_sub(border_width)) as f32;

            // Expand bounding box for anti-aliasing
            let aa_offset = border_width.max(2);
            let start_x = center_x.saturating_sub(radius + aa_offset);
            let start_y = center_y.saturating_sub(radius + aa_offset);
            let end_x = (center_x + radius + aa_offset + 1).min(width);
            let end_y = (center_y + radius + aa_offset + 1).min(height);

            let samples = 4.0;
            let step = 1.0 / samples;

            for y in start_y..end_y {
                for x in start_x..end_x {
                    // Supersample with 4x4 grid for smooth anti-aliasing
                    let mut coverage = 0.0;

                    for sy in 0..4 {
                        for sx in 0..4 {
                            let sample_x = x as f32 + (sx as f32 + 0.5) * step;
                            let sample_y = y as f32 + (sy as f32 + 0.5) * step;

                            let dx = sample_x - center_x as f32;
                            let dy = sample_y - center_y as f32;
                            let dist = (dx * dx + dy * dy).sqrt();

                            // Check if sample point is in border ring
                            if dist <= outer_r && dist >= inner_r {
                                coverage += 1.0;
                            }
                        }
                    }

                    let alpha = coverage / (samples * samples);

                    if alpha > 0.01 {
                        let existing = *image.get_pixel(x, y);
                        let border = create_border_pixel::<P>(border_color);
                        let blended = blend_pixels::<P>(existing, border, alpha);
                        image.put_pixel(x, y, blended);
                    }
                }
            }
        }
        BorderShape::Rectangle {
            width: rect_width,
            height: rect_height,
        } => {
            let x = x_or_center_x;
            let y = y_or_center_y;
            let (img_width, img_height) = image.dimensions();
            let border_pixel = create_border_pixel::<P>(border_color);

            for bw in 0..border_width {
                // Top edge
                for bx in x..=(x + rect_width - 1).min(img_width - 1) {
                    let by = y + bw;
                    if by < img_height {
                        image.put_pixel(bx, by, border_pixel);
                    }
                }
                // Bottom edge
                for bx in x..=(x + rect_width - 1).min(img_width - 1) {
                    if bw < rect_height {
                        let by = y + rect_height - 1 - bw;
                        if by < img_height && by > 0 {
                            image.put_pixel(bx, by, border_pixel);
                        }
                    }
                }
                // Left edge
                for by in y..=(y + rect_height - 1).min(img_height - 1) {
                    let bx = x + bw;
                    if bx < img_width {
                        image.put_pixel(bx, by, border_pixel);
                    }
                }
                // Right edge
                for by in y..=(y + rect_height - 1).min(img_height - 1) {
                    if bw < rect_width {
                        let bx = x + rect_width - 1 - bw;
                        if bx < img_width && bx > 0 {
                            image.put_pixel(bx, by, border_pixel);
                        }
                    }
                }
            }
        }
    }
}

fn create_border_pixel<P>(color: Rgba<u8>) -> P
where
    P: Pixel<Subpixel = u8> + Copy,
{
    let channels = P::CHANNEL_COUNT;
    let mut buffer = [0u8; 4];
    buffer[0] = color[0];
    buffer[1] = color[1];
    buffer[2] = color[2];
    if channels == 4 {
        buffer[3] = color[3];
    }
    *P::from_slice(&buffer[..channels as usize])
}

fn blend_pixels<P>(existing: P, new: P, alpha: f32) -> P
where
    P: Pixel<Subpixel = u8> + Copy,
{
    let channels = P::CHANNEL_COUNT;
    let existing_channels = existing.channels();
    let new_channels = new.channels();

    let mut buffer = [0u8; 4];
    for c in 0..channels as usize {
        let e = existing_channels[c] as f32;
        let n = new_channels[c] as f32;
        buffer[c] = (e * (1.0 - alpha) + n * alpha).round() as u8;
    }

    *P::from_slice(&buffer[..channels as usize])
}
