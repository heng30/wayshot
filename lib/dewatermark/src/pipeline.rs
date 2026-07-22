use image::{DynamicImage, GenericImageView, GrayImage, RgbImage};
use ndarray::Array4;
use ort::session::Session;
use thiserror::Error;

use crate::mask::{self, MaskInput, WatermarkRegion};
use crate::model::{self, ModelError};

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
}

pub const MODEL_INPUT_SIZE: u32 = 512;

fn image_to_chw_tensor(img: &RgbImage) -> Array4<f32> {
    let (w, h) = img.dimensions();
    let mut tensor = Array4::<f32>::zeros((1, 3, h as usize, w as usize));

    for y in 0..h {
        for x in 0..w {
            let pixel = img.get_pixel(x, y);
            tensor[[0, 0, y as usize, x as usize]] = pixel[0] as f32 / 255.0;
            tensor[[0, 1, y as usize, x as usize]] = pixel[1] as f32 / 255.0;
            tensor[[0, 2, y as usize, x as usize]] = pixel[2] as f32 / 255.0;
        }
    }

    tensor
}

fn chw_tensor_to_image(tensor: &Array4<f32>) -> RgbImage {
    let h = tensor.shape()[2];
    let w = tensor.shape()[3];

    let sample_size = 1000.min(tensor.len() / 3);
    let mut max_val: f32 = 0.0;
    for i in 0..sample_size {
        max_val = max_val.max(tensor.as_slice().unwrap()[i].abs());
    }
    let is_normalized = max_val <= 2.0;

    let mut img = RgbImage::new(w as u32, h as u32);

    for y in 0..h {
        for x in 0..w {
            let mut r = tensor[[0, 0, y, x]];
            let mut g = tensor[[0, 1, y, x]];
            let mut b = tensor[[0, 2, y, x]];

            if is_normalized {
                r *= 255.0;
                g *= 255.0;
                b *= 255.0;
            }

            img.put_pixel(
                x as u32,
                y as u32,
                image::Rgb([
                    r.round().clamp(0.0, 255.0) as u8,
                    g.round().clamp(0.0, 255.0) as u8,
                    b.round().clamp(0.0, 255.0) as u8,
                ]),
            );
        }
    }

    img
}

fn compose(original: &DynamicImage, inpainted: &RgbImage, region: &WatermarkRegion) -> DynamicImage {
    let (orig_w, orig_h) = original.dimensions();
    let (proc_w, proc_h) = inpainted.dimensions();

    let scale_x = proc_w as f64 / orig_w as f64;
    let scale_y = proc_h as f64 / orig_h as f64;

    let proc_region = WatermarkRegion {
        x: (region.x as f64 * scale_x).round() as u32,
        y: (region.y as f64 * scale_y).round() as u32,
        width: (region.width as f64 * scale_x).round() as u32,
        height: (region.height as f64 * scale_y).round() as u32,
    };

    let inpainted_dynamic = DynamicImage::ImageRgb8(inpainted.clone());
    let cropped = inpainted_dynamic.crop_imm(
        proc_region.x,
        proc_region.y,
        proc_region.width,
        proc_region.height,
    );

    let scaled = cropped.resize_exact(
        region.width,
        region.height,
        image::imageops::FilterType::Lanczos3,
    );

    let mut result = original.to_rgb8();
    image::imageops::overlay(
        &mut result,
        &scaled.to_rgb8(),
        region.x as i64,
        region.y as i64,
    );

    DynamicImage::ImageRgb8(result)
}

fn compose_pixels(
    original: &DynamicImage,
    inpainted: &RgbImage,
    mask: &GrayImage,
) -> DynamicImage {
    let (orig_w, orig_h) = original.dimensions();

    let resized_mask = image::imageops::resize(mask, orig_w, orig_h, image::imageops::FilterType::Nearest);
    let resized_inpainted = DynamicImage::ImageRgb8(inpainted.clone()).resize_exact(
        orig_w,
        orig_h,
        image::imageops::FilterType::Lanczos3,
    );

    let mut result = original.to_rgb8();
    let inpainted_rgb = resized_inpainted.to_rgb8();

    for y in 0..orig_h {
        for x in 0..orig_w {
            if resized_mask.get_pixel(x, y).0[0] > 0 {
                result.put_pixel(x, y, *inpainted_rgb.get_pixel(x, y));
            }
        }
    }

    DynamicImage::ImageRgb8(result)
}

/// Run the inpainting pipeline on an image, removing the content in the given mask region.
pub fn process(
    original: &DynamicImage,
    session: &mut Session,
    mask_input: &MaskInput,
    mut progress: impl FnMut(&str),
) -> Result<DynamicImage, PipelineError> {
    let (orig_w, orig_h) = original.dimensions();
    progress(&format!("Image: {}x{}", orig_w, orig_h));

    progress("Resizing for model...");
    let resized = original.resize_exact(
        MODEL_INPUT_SIZE,
        MODEL_INPUT_SIZE,
        image::imageops::FilterType::Lanczos3,
    );
    let resized_rgb = resized.to_rgb8();

    progress("Preprocessing...");
    let image_tensor = image_to_chw_tensor(&resized_rgb);

    let scaled_mask_input = match mask_input {
        MaskInput::Rect(region) => {
            let scale_x = MODEL_INPUT_SIZE as f64 / orig_w as f64;
            let scale_y = MODEL_INPUT_SIZE as f64 / orig_h as f64;
            MaskInput::Rect(WatermarkRegion {
                x: (region.x as f64 * scale_x).round() as u32,
                y: (region.y as f64 * scale_y).round() as u32,
                width: (region.width as f64 * scale_x).round() as u32,
                height: (region.height as f64 * scale_y).round() as u32,
            })
        }
        MaskInput::Pixels(gray) => MaskInput::Pixels(gray.clone()),
    };
    let mask_tensor = mask::generate_mask_from_input(MODEL_INPUT_SIZE, MODEL_INPUT_SIZE, &scaled_mask_input);

    progress("Running AI inference...");
    let output_tensor = model::run_inference(session, image_tensor, mask_tensor)?;

    progress("Composing final image...");
    let inpainted = chw_tensor_to_image(&output_tensor);
    let final_image = match mask_input {
        MaskInput::Rect(region) => compose(original, &inpainted, region),
        MaskInput::Pixels(gray) => compose_pixels(original, &inpainted, gray),
    };

    Ok(final_image)
}