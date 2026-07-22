use candle_core::{DType, Device, Tensor};
use image::{DynamicImage, ImageReader};

use crate::utils::{ceil_by_factor, floor_by_factor, round_by_factor};
use crate::Error;

/// Load image from local file path
pub fn get_image(file: &str) -> Result<DynamicImage, Error> {
    Ok(
        ImageReader::open(file)
            .map_err(|e| Error::ImageOpen(image::ImageError::IoError(e.into())))?
            .decode()
            .map_err(Error::ImageDecode)?,
    )
}

/// Transform image to tensor with normalization
pub fn img_transform(
    img: &DynamicImage,
    mean: &Tensor,
    std: &Tensor,
    device: &Device,
    dtype: DType,
) -> Result<Tensor, Error> {
    let img_h = img.height();
    let img_w = img.width();
    let img_vec = img.to_rgb8().into_raw();
    // (h, w, c) => (c, h, w)
    let img_tensor = Tensor::from_slice(&img_vec, (img_h as usize, img_w as usize, 3), device)?
        .permute((2, 0, 1))?
        .to_dtype(DType::F32)?;
    // 0-255 rescale to 0-1
    let img_tensor = img_tensor.affine(1.0 / 255.0, 0.)?;
    // normalize
    let img_tensor = img_tensor
        .broadcast_sub(&mean.to_dtype(DType::F32)?)?
        .broadcast_div(&std.to_dtype(DType::F32)?)?
        .to_dtype(dtype)?;
    Ok(img_tensor)
}

/// Smart resize image to be divisible by factor, within min/max pixels range
pub fn img_smart_resize(
    img_h: u32,
    img_w: u32,
    factor: u32,
    min_pixels: u32,
    max_pixels: u32,
) -> Result<(u32, u32), Error> {
    if std::cmp::max(img_h, img_w) / std::cmp::min(img_h, img_w) > 200 {
        return Err(Error::ImageAspectRatioTooLarge(
            std::cmp::max(img_h, img_w) / std::cmp::min(img_h, img_w),
        ));
    }
    let image_factor = factor;
    let mut h_bar = std::cmp::max(image_factor, round_by_factor(img_h, image_factor));
    let mut w_bar = std::cmp::max(image_factor, round_by_factor(img_w, image_factor));

    if h_bar * w_bar > max_pixels {
        let beta = ((img_h * img_w) as f32 / max_pixels as f32).sqrt();
        h_bar = std::cmp::max(
            image_factor,
            floor_by_factor(img_h as f32 / beta, image_factor),
        );
        w_bar = std::cmp::max(
            image_factor,
            floor_by_factor(img_w as f32 / beta, image_factor),
        );
    } else if h_bar * w_bar < min_pixels {
        let beta = (min_pixels as f32 / (img_h * img_w) as f32).sqrt();
        h_bar = ceil_by_factor(img_h as f32 * beta, image_factor);
        w_bar = ceil_by_factor(img_w as f32 * beta, image_factor);
    }
    Ok((h_bar, w_bar))
}