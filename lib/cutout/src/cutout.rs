use crate::{CutoutError, manager::ModelManager, model::Model};
use image::{GenericImageView, GrayImage, ImageBuffer, Luma, Rgb, RgbImage, Rgba, RgbaImage};
use imageproc::{
    contrast::{ThresholdType, threshold},
    distance_transform::{Norm, distance_transform},
};
use ndarray::{Array4, Axis};

pub struct CutoutResult {
    pub image: RgbaImage,
    pub mask: RgbImage,
}

impl CutoutResult {
    pub fn image(&self) -> &RgbaImage {
        &self.image
    }

    pub fn mask(&self) -> &RgbImage {
        &self.mask
    }

    pub fn into_parts(self) -> (RgbaImage, RgbImage) {
        (self.image, self.mask)
    }
}

#[derive(Debug, Clone, derive_setters::Setters)]
#[setters(prefix = "with_")]
pub struct CutoutOptions {
    /// Threshold for alpha matting (0–255).
    /// Higher values = more aggressive background removal.
    /// - 76–102: Soft edges with semi-transparency (≈0.3–0.4)
    /// - 128: Balanced (default, ≈0.5)
    /// - 153–179: Stronger cutout, cleaner edges (≈0.6–0.7)
    pub threshold: u8,

    /// If true, creates hard cutout without semi-transparency.
    /// If false, allows soft edges for more natural blending.
    pub binary: bool,

    /// Sticker border color. None = no sticker border.
    /// Use a fully transparent color (alpha = 0) to skip, any other color to draw.
    pub sticker: Option<Rgba<u8>>,

    /// Optional region mask for selective background removal.
    /// White (255) = keep model prediction, Black (0) = force background.
    /// Must have the same dimensions as the input image.
    pub mask: Option<GrayImage>,
}

impl Default for CutoutOptions {
    fn default() -> Self {
        Self {
            threshold: 160,
            binary: false,
            sticker: None,
            mask: None,
        }
    }
}

pub fn cutout(
    manager: &mut ModelManager,
    model: Model,
    image: image::DynamicImage,
    options: &CutoutOptions,
) -> Result<CutoutResult, CutoutError> {
    let (std_r, std_g, std_b) = model.std();
    let (mean_r, mean_g, mean_b) = model.mean();
    let (target_width, target_height) = model.to_input_size();
    let (original_width, original_height) = image.dimensions();

    let preprocessed = {
        let rgb_img = image.to_rgb8();

        let resized = image::imageops::resize(
            &rgb_img,
            target_width,
            target_height,
            image::imageops::FilterType::Lanczos3,
        );

        let mut array = Array4::<f32>::zeros((1, 3, target_height as usize, target_width as usize));

        for (x, y, pixel) in resized.enumerate_pixels() {
            let [r, g, b] = pixel.0;
            array[[0, 0, y as usize, x as usize]] = (r as f32 / 255.0 - mean_r) / std_r;
            array[[0, 1, y as usize, x as usize]] = (g as f32 / 255.0 - mean_g) / std_g;
            array[[0, 2, y as usize, x as usize]] = (b as f32 / 255.0 - mean_b) / std_b;
        }

        array
    };

    let mask_output: Array4<f32> = manager.run_inference(&preprocessed)?;

    // Postprocess: build heatmap mask (for visualization)
    let mask = {
        use image::imageops::FilterType;

        let temp_axis = mask_output.index_axis(Axis(0), 0);
        let shape = temp_axis.shape();
        let (model_height, model_width) = (shape[1], shape[2]);

        let mut heat = RgbImage::new(model_width as u32, model_height as u32);

        if model == Model::U2NetClothSeg {
            // Multi-class segmentation: argmax over class channels
            let num_classes = temp_axis.shape()[0];
            for (x, y, pixel) in heat.enumerate_pixels_mut() {
                let mut max_val = f32::NEG_INFINITY;
                let mut max_class = 0usize;
                for c in 0..num_classes {
                    let v = temp_axis[[c, y as usize, x as usize]];
                    if v > max_val {
                        max_val = v;
                        max_class = c;
                    }
                }
                // class 0 = background, class 1/2/3 = cloth
                let s = if max_class > 0 { 1.0 } else { 0.0 };
                let (r, g, b) = colormap(s);
                *pixel = Rgb([r, g, b]);
            }
        } else {
            let gamma: f32 = 0.5;
            let g = gamma.clamp(0.2, 5.0);
            let mut lut = [(0u8, 0u8, 0u8); 256];
            for i in 0..256 {
                let t = (i as f32 / 255.0).powf(g);
                lut[i] = colormap(t);
            }

            let mask_data = temp_axis.index_axis(Axis(0), 0);
            for (x, y, pixel) in heat.enumerate_pixels_mut() {
                let v = mask_data[[y as usize, x as usize]];
                let s = 1.0 / (1.0 + (-v).exp());
                let idx = (s * 255.0).round() as usize;
                let (r, g, b) = lut[idx.min(255)];
                *pixel = Rgb([r, g, b]);
            }
        }

        image::imageops::resize(&heat, original_width, original_height, FilterType::Lanczos3)
    };

    // Postprocess: build grayscale mask and apply to image
    let result_image = {
        let rgba_img = image.to_rgba8();
        let (width, height) = rgba_img.dimensions();

        if mask_output.ndim() != 4 {
            return Err(CutoutError::PreprocessingError(format!(
                "Unexpected mask shape: {:?}",
                mask_output.shape()
            )));
        }

        let temp_axis = mask_output.index_axis(Axis(0), 0);
        let shape = temp_axis.shape();
        let (model_h, model_w) = (shape[1], shape[2]);

        let need_resize = (model_w as u32 != width) || (model_h as u32 != height);
        let mut mask_gray: ImageBuffer<Luma<u8>, Vec<u8>> =
            ImageBuffer::new(model_w as u32, model_h as u32);

        if model == Model::U2NetClothSeg {
            // Multi-class segmentation: argmax, class 0 = background, 1/2/3 = cloth
            let num_classes = temp_axis.shape()[0];
            for (x, y, pixel) in mask_gray.enumerate_pixels_mut() {
                let mut max_val = f32::NEG_INFINITY;
                let mut max_class = 0usize;
                for c in 0..num_classes {
                    let v = temp_axis[[c, y as usize, x as usize]];
                    if v > max_val {
                        max_val = v;
                        max_class = c;
                    }
                }
                pixel.0[0] = if max_class > 0 { 255 } else { 0 };
            }
        } else {
            let mask_data = temp_axis.index_axis(Axis(0), 0);
            for (x, y, pixel) in mask_gray.enumerate_pixels_mut() {
                let v = mask_data[[y as usize, x as usize]];
                let s = 1.0 / (1.0 + (-v).exp());
                pixel.0[0] = (s * 255.0).clamp(0.0, 255.0) as u8;
            }
        }

        let mask_resized = if need_resize {
            image::imageops::resize(
                &mask_gray,
                width,
                height,
                image::imageops::FilterType::Lanczos3,
            )
        } else {
            mask_gray
        };

        // Apply user region mask if provided
        let final_mask = if let Some(ref user_mask) = options.mask {
            if user_mask.dimensions() != (width, height) {
                return Err(CutoutError::InvalidInput(format!(
                    "User mask dimensions {:?} do not match input image dimensions {:?}",
                    user_mask.dimensions(),
                    (width, height)
                )));
            }

            let mut combined = GrayImage::new(width, height);
            for (x, y, pixel) in combined.enumerate_pixels_mut() {
                let model_val = mask_resized.get_pixel(x, y).0[0] as f32;
                let user_val = user_mask.get_pixel(x, y).0[0] as f32;
                pixel.0[0] = (model_val * (user_val / 255.0)).clamp(0.0, 255.0).round() as u8;
            }
            combined
        } else {
            mask_resized
        };

        let mut result = RgbaImage::new(width, height);
        let thr_u8 = options.threshold;
        let thr_f = thr_u8 as f32;

        let smooth_scale = if thr_u8 < 255 {
            Some(255.0 / (255.0 - thr_f))
        } else {
            None
        };

        for (x, y, src) in rgba_img.enumerate_pixels() {
            let mask_value = final_mask.get_pixel(x, y).0[0];

            let alpha: u8 = if options.binary {
                if mask_value >= thr_u8 { 255 } else { 0 }
            } else {
                match smooth_scale {
                    Some(scale) => {
                        let mv = mask_value as f32;
                        ((mv - thr_f) * scale * 255.0).clamp(0.0, 255.0).round() as u8
                    }
                    None => {
                        if mask_value == 255 {
                            255
                        } else {
                            0
                        }
                    }
                }
            };

            result.put_pixel(x, y, Rgba([src.0[0], src.0[1], src.0[2], alpha]));
        }

        if let Some(color) = options.sticker {
            if color.0[3] > 0 {
                result = clean_sticker_border(&result, color);
            }
        }

        result
    };

    Ok(CutoutResult {
        image: result_image,
        mask,
    })
}

#[inline]
fn lerp(a: (u8, u8, u8), b: (u8, u8, u8), t: f32) -> (u8, u8, u8) {
    let (ar, ag, ab) = a;
    let (br, bg, bb) = b;
    let r = ar as f32 + (br as f32 - ar as f32) * t;
    let g = ag as f32 + (bg as f32 - ag as f32) * t;
    let b = ab as f32 + (bb as f32 - ab as f32) * t;
    (r.round() as u8, g.round() as u8, b.round() as u8)
}

static STOPS: &[(f32, (u8, u8, u8))] = &[
    (0.00, (0, 0, 0)),
    (0.15, (0, 0, 64)),
    (0.30, (0, 0, 255)),
    (0.45, (128, 0, 192)),
    (0.60, (255, 0, 0)),
    (0.75, (255, 128, 0)),
    (0.90, (255, 255, 0)),
    (1.00, (255, 255, 255)),
];

#[inline]
fn colormap(t: f32) -> (u8, u8, u8) {
    let t = t.clamp(0.0, 1.0);
    for w in STOPS.windows(2) {
        let (t0, c0) = (w[0].0, w[0].1);
        let (t1, c1) = (w[1].0, w[1].1);
        if t <= t1 {
            let local = if t1 > t0 { (t - t0) / (t1 - t0) } else { 0.0 };
            return lerp(c0, c1, local);
        }
    }
    STOPS.last().unwrap().1
}

/// Cleans border by adding a soft outline OUTSIDE only (≈6px, feather ≈1.5px)
/// and keeps sticker RGB/alpha intact. No half-transparent pixel on outer arcs.
/// `border_color` specifies the RGBA color of the outline.
pub fn clean_sticker_border(img: &RgbaImage, border_color: Rgba<u8>) -> RgbaImage {
    let (w, h) = img.dimensions();

    // 1) Binary mask of the sticker (alpha >= 16 is inside)
    let alpha = GrayImage::from_fn(w, h, |x, y| Luma([img.get_pixel(x, y)[3]]));
    let mask = threshold(&alpha, 16, ThresholdType::Binary);

    // 2) Distance from OUTSIDE to nearest inside pixel (round metric for smooth arcs)
    //    distance_transform returns per-pixel distance in pixel units as u32.
    //    Use L2 to avoid chessboard artifacts on curves.
    let dist_out = distance_transform(&mask, Norm::L2);

    // Stroke params
    let stroke: f32 = 6.0; // full strength up to 6px
    let feather: f32 = 1.5; // soft falloff outside (6..7.5px)
    let max_outline_alpha: f32 = border_color.0[3] as f32;

    #[inline]
    fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
        let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    }

    // 3) Composite: draw outline BEHIND the sticker only where mask==0
    let mut out = RgbaImage::new(w, h);
    let border_r = border_color.0[0] as f32;
    let border_g = border_color.0[1] as f32;
    let border_b = border_color.0[2] as f32;
    for y in 0..h {
        for x in 0..w {
            let top = img.get_pixel(x, y);
            let inside = mask.get_pixel(x, y)[0] != 0;

            // background/outline
            let mut br = 0f32;
            let mut bg = 0f32;
            let mut bb = 0f32;
            let mut ba = 0f32;

            if !inside {
                let d = dist_out.get_pixel(x, y)[0] as f32;
                // outline alpha profile: full inside [0, stroke], smooth fade in (stroke..stroke+feather)
                let a01 = if d <= stroke {
                    1.0
                } else {
                    1.0 - smoothstep(stroke, stroke + feather, d)
                };
                let a = (a01 * max_outline_alpha).round().clamp(0.0, 255.0) as u8;

                // Kill tiny tails completely to avoid single half-transparent pixels outside.
                let a = if a < 3 { 0 } else { a };
                br = border_r;
                bg = border_g;
                bb = border_b;
                ba = a as f32 / 255.0;
            }

            // standard "over" compositing: top over outline
            let ta = top[3] as f32 / 255.0;
            let oa = ta + ba * (1.0 - ta);
            let (r, g, b) = if oa > 0.0 {
                (
                    (top[0] as f32 * ta + br * (1.0 - ta) * ba) / oa,
                    (top[1] as f32 * ta + bg * (1.0 - ta) * ba) / oa,
                    (top[2] as f32 * ta + bb * (1.0 - ta) * ba) / oa,
                )
            } else {
                (0.0, 0.0, 0.0)
            };

            out.put_pixel(
                x,
                y,
                Rgba([
                    r.round() as u8,
                    g.round() as u8,
                    b.round() as u8,
                    (oa * 255.0).round() as u8,
                ]),
            );
        }
    }

    out
}
