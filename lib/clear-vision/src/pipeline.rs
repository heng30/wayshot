use crate::model::{ModelError, TILE_PAD, TILE_SIZE, run_inference};
use image::{DynamicImage, RgbImage};
use ndarray::Array4;
use ort::session::Session;
use thiserror::Error;

const SCALE: usize = 4;

#[derive(Debug, Error)]
pub enum PipelineError {
    #[error("Failed to open image {path}")]
    OpenImage {
        #[source]
        source: image::ImageError,
        path: String,
    },
    #[error("Failed to save image {path}")]
    SaveImage {
        #[source]
        source: image::ImageError,
        path: String,
    },
    #[error("Inference failed")]
    Inference(#[from] ModelError),
    #[error("Processing cancelled")]
    Cancelled,
}

pub fn process(
    original: &DynamicImage,
    session: &mut Session,
    mut progress: impl FnMut(f32),
    cancelled: impl Fn() -> bool,
) -> Result<DynamicImage, PipelineError> {
    let (width, height) = (original.width() as usize, original.height() as usize);
    progress(0.0);

    let img_rgb = original.to_rgb8();
    let img_raw = img_rgb.as_raw();

    let out_w = width * SCALE;
    let out_h = height * SCALE;
    let mut out_img = RgbImage::new(out_w as u32, out_h as u32);

    let tile_inner = TILE_SIZE - 2 * TILE_PAD;
    let tiles_x = (width + tile_inner - 1) / tile_inner;
    let tiles_y = (height + tile_inner - 1) / tile_inner;
    let total = tiles_x * tiles_y;

    let step = tile_inner;
    let tile_px = TILE_SIZE * TILE_SIZE;

    for ty in 0..tiles_y {
        for tx in 0..tiles_x {
            if cancelled() {
                return Err(PipelineError::Cancelled);
            }
            let x0 = if tx == 0 { 0 } else { tx * step - TILE_PAD };
            let y0 = if ty == 0 { 0 } else { ty * step - TILE_PAD };

            let x0 = x0.min(width.saturating_sub(TILE_SIZE));
            let y0 = y0.min(height.saturating_sub(TILE_SIZE));

            let mut buf = vec![0.0f32; 3 * tile_px];
            let (r_plane, rest) = buf.split_at_mut(tile_px);
            let (g_plane, b_plane) = rest.split_at_mut(tile_px);

            for y in 0..TILE_SIZE {
                let py = y0 + y;
                let mirror_y = if py < height { py } else { 2 * height - 1 - py };
                let src_row = mirror_y * width;

                for x in 0..TILE_SIZE {
                    let px = x0 + x;
                    let mirror_x = if px < width { px } else { 2 * width - 1 - px };
                    let src_idx = (src_row + mirror_x) * 3;

                    let idx = y * TILE_SIZE + x;
                    r_plane[idx] = img_raw[src_idx] as f32 / 255.0;
                    g_plane[idx] = img_raw[src_idx + 1] as f32 / 255.0;
                    b_plane[idx] = img_raw[src_idx + 2] as f32 / 255.0;
                }
            }

            let input_data = Array4::from_shape_vec((1, 3, TILE_SIZE, TILE_SIZE), buf)
                .expect("tile shape is always valid");
            let output_tensor = run_inference(session, input_data)?;

            let oh = output_tensor.shape()[2];
            let ow = output_tensor.shape()[3];

            let pad_left = if tx == 0 { 0 } else { TILE_PAD * SCALE };
            let pad_top = if ty == 0 { 0 } else { TILE_PAD * SCALE };
            let pad_right = if (x0 + TILE_SIZE) >= width {
                0
            } else {
                TILE_PAD * SCALE
            };
            let pad_bottom = if (y0 + TILE_SIZE) >= height {
                0
            } else {
                TILE_PAD * SCALE
            };

            let copy_x_start = pad_left;
            let copy_y_start = pad_top;
            let copy_x_end = ow - pad_right;
            let copy_y_end = oh - pad_bottom;

            let dst_x_start = (x0 + pad_left / SCALE) * SCALE;
            let dst_y_start = (y0 + pad_top / SCALE) * SCALE;

            let copy_w = copy_x_end - copy_x_start;
            for y in copy_y_start..copy_y_end {
                let dst_y = dst_y_start + (y - copy_y_start);
                if dst_y >= out_h {
                    break;
                }
                for xi in 0..copy_w {
                    let x = copy_x_start + xi;
                    let _idx = y * ow + x;
                    let dst_xx = dst_x_start + xi;
                    if dst_xx >= out_w {
                        break;
                    }
                    let r = (output_tensor[[0, 0, y, x]] * 255.0).clamp(0.0, 255.0) as u8;
                    let g = (output_tensor[[0, 1, y, x]] * 255.0).clamp(0.0, 255.0) as u8;
                    let b = (output_tensor[[0, 2, y, x]] * 255.0).clamp(0.0, 255.0) as u8;
                    out_img.put_pixel(dst_xx as u32, dst_y as u32, image::Rgb([r, g, b]));
                }
            }

            progress(((ty * tiles_x + tx + 1) as f32) / (total as f32));
        }
    }

    Ok(DynamicImage::ImageRgb8(out_img))
}
