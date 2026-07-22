use crate::model::ModelConfig;
use ort::value::Tensor;

/// Preprocessed image data ready for the vision encoder.
///
/// Each tile is a separate batch item for the vision encoder:
/// - pixel_values: [num_tiles, max_num_patches, 768]
/// - pixel_attention_mask: [num_tiles, max_num_patches]
/// - spatial_shapes: [num_tiles, 2]
pub struct ImageInput {
    /// Per-tile pixel values: Vec of length num_tiles, each of size max_num_patches * 768
    pub pixel_values_tiles: Vec<Vec<f32>>,
    /// Per-tile attention mask: Vec of length num_tiles, each of size max_num_patches
    pub pixel_attention_mask_tiles: Vec<Vec<i64>>,
    /// Spatial shapes: flat [num_tiles * 2] — (height_patches, width_patches) per tile
    pub spatial_shapes: Vec<i64>,
    /// Number of tiles
    pub num_tiles: usize,
    /// Max patches per tile (=1024)
    pub max_num_patches: usize,
}

fn round_by_factor(x: usize, factor: usize) -> usize {
    let rounded = ((x as f64 / factor as f64).round() as usize).max(1) * factor;
    rounded.max(factor)
}

/// Smart resize: resize image dimensions so both are divisible by `total_factor`,
/// maintain aspect ratio, and keep total pixels within the allowed range.
fn smart_resize(height: usize, width: usize, config: &ModelConfig) -> (usize, usize) {
    let total_factor = config.patch_size * config.downsample_factor();
    let ps = config.patch_size;
    let df = config.downsample_factor();
    let min_pixels = config.min_image_tokens() * ps * ps * df * df;
    let max_pixels = config.max_image_tokens() * ps * ps * df * df;

    let mut h_bar = round_by_factor(height, total_factor);
    let mut w_bar = round_by_factor(width, total_factor);

    if h_bar * w_bar > max_pixels {
        let beta = ((height * width) as f64 / max_pixels as f64).sqrt();
        h_bar = (height as f64 / beta / total_factor as f64).floor() as usize * total_factor;
        w_bar = (width as f64 / beta / total_factor as f64).floor() as usize * total_factor;
        h_bar = h_bar.max(total_factor);
        w_bar = w_bar.max(total_factor);
    } else if h_bar * w_bar < min_pixels {
        let beta = min_pixels as f64 / (height * width) as f64;
        h_bar = (height as f64 * beta / total_factor as f64).ceil() as usize * total_factor;
        w_bar = (width as f64 * beta / total_factor as f64).ceil() as usize * total_factor;
        h_bar = h_bar.max(total_factor);
        w_bar = w_bar.max(total_factor);
    }

    (w_bar, h_bar)
}

fn is_image_too_large(height: usize, width: usize, config: &ModelConfig) -> bool {
    let total_factor = config.patch_size * config.downsample_factor();
    let max_pixels_tol = (config.max_image_tokens()
        * config.patch_size.pow(2)
        * config.downsample_factor().pow(2)) as f64
        * config.max_pixels_tolerance();
    let h_bar = round_by_factor(height, total_factor);
    let w_bar = round_by_factor(width, total_factor);
    h_bar * w_bar > max_pixels_tol as usize
}

/// Convert an image region to patches and pad to max_num_patches.
fn image_to_padded_patches(
    rgb: &image::RgbImage,
    x_offset: usize,
    y_offset: usize,
    region_w: usize,
    region_h: usize,
    patch_size: usize,
    max_num_patches: usize,
) -> (Vec<f32>, Vec<i64>) {
    let patch_dim = patch_size * patch_size * 3;
    let num_patches_h = region_h / patch_size;
    let num_patches_w = region_w / patch_size;
    let num_valid_patches = num_patches_h * num_patches_w;

    let mut pixel_values = vec![0.0f32; max_num_patches * patch_dim];

    for py in 0..num_patches_h {
        for px in 0..num_patches_w {
            let patch_offset = (py * num_patches_w + px) * patch_dim;
            for ph in 0..patch_size {
                for pw in 0..patch_size {
                    let img_x = x_offset + px * patch_size + pw;
                    let img_y = y_offset + py * patch_size + ph;
                    if img_x < rgb.width() as usize && img_y < rgb.height() as usize {
                        let pixel = rgb.get_pixel(img_x as u32, img_y as u32);
                        for c in 0..3usize {
                            let idx = patch_offset + (ph * patch_size + pw) * 3 + c;
                            pixel_values[idx] = (pixel[c] as f32 / 255.0 - 0.5) / 0.5;
                        }
                    }
                }
            }
        }
    }

    let mut attention_mask = vec![0i64; max_num_patches];
    for i in 0..num_valid_patches {
        attention_mask[i] = 1;
    }

    (pixel_values, attention_mask)
}

/// Preprocess an image for the vision encoder.
pub fn preprocess_image(
    img: &image::DynamicImage,
    config: &ModelConfig,
) -> crate::Result<ImageInput> {
    let (img_w, img_h) = (img.width() as usize, img.height() as usize);
    let (new_w, new_h) = smart_resize(img_h, img_w, config);

    if config.do_image_splitting() && is_image_too_large(img_h, img_w, config) {
        preprocess_with_tiling(img, config, new_w, new_h)
    } else {
        preprocess_single(img, config, new_w, new_h)
    }
}

fn preprocess_single(
    img: &image::DynamicImage,
    config: &ModelConfig,
    new_w: usize,
    new_h: usize,
) -> crate::Result<ImageInput> {
    let patch_size = config.patch_size;
    let max_num_patches = config.max_num_patches();

    let resized = img.resize_exact(
        new_w as u32,
        new_h as u32,
        image::imageops::FilterType::Lanczos3,
    );
    let rgb = resized.to_rgb8();

    let num_patches_h = new_h / patch_size;
    let num_patches_w = new_w / patch_size;

    let (pixel_values, attention_mask) =
        image_to_padded_patches(&rgb, 0, 0, new_w, new_h, patch_size, max_num_patches);

    Ok(ImageInput {
        pixel_values_tiles: vec![pixel_values],
        pixel_attention_mask_tiles: vec![attention_mask],
        spatial_shapes: vec![num_patches_h as i64, num_patches_w as i64],
        num_tiles: 1,
        max_num_patches,
    })
}

/// Preprocess with tiling for large images.
///
/// Each tile (including thumbnail) becomes a separate batch item,
/// each independently padded to max_num_patches. The thumbnail uses
/// a square spatial shape [32,32] matching grid tiles, because the
/// ONNX vision encoder's positional embedding uses ReduceMax across
/// spatial_shapes — non-uniform shapes would cause max_spatial to
/// exceed max_num_patches.
fn preprocess_with_tiling(
    img: &image::DynamicImage,
    config: &ModelConfig,
    new_w: usize,
    new_h: usize,
) -> crate::Result<ImageInput> {
    let tile_size = config.tile_size;
    let patch_size = config.patch_size;
    let max_num_patches = config.max_num_patches();
    let patches_per_tile_side = tile_size / patch_size;

    let aspect_ratio = new_w as f64 / new_h as f64;
    let target_ratios = compute_target_ratios(config.min_tiles(), config.max_tiles());
    let (grid_w, grid_h) =
        find_closest_aspect_ratio(aspect_ratio, &target_ratios, new_w, new_h, tile_size);

    let resized = img.resize_exact(
        (tile_size * grid_w) as u32,
        (tile_size * grid_h) as u32,
        image::imageops::FilterType::Lanczos3,
    );
    let grid_rgb = resized.to_rgb8();

    let mut pixel_values_tiles: Vec<Vec<f32>> = Vec::new();
    let mut attention_mask_tiles: Vec<Vec<i64>> = Vec::new();
    let mut spatial_shapes: Vec<i64> = Vec::new();

    // Thumbnail tile (if grid has more than 1 tile)
    if grid_w * grid_h > 1 && config.use_thumbnail() {
        let thumb = img.resize_exact(
            new_w as u32,
            new_h as u32,
            image::imageops::FilterType::Lanczos3,
        );
        let mut thumb_canvas = image::RgbImage::new(tile_size as u32, tile_size as u32);
        image::imageops::overlay(&mut thumb_canvas, &thumb.to_rgb8(), 0, 0);

        let (pv, am) = image_to_padded_patches(
            &thumb_canvas,
            0,
            0,
            tile_size,
            tile_size,
            patch_size,
            max_num_patches,
        );
        pixel_values_tiles.push(pv);
        attention_mask_tiles.push(am);
        spatial_shapes.push(patches_per_tile_side as i64);
        spatial_shapes.push(patches_per_tile_side as i64);
    }

    // Grid tiles
    for ty in 0..grid_h {
        for tx in 0..grid_w {
            let (pv, am) = image_to_padded_patches(
                &grid_rgb,
                tx * tile_size,
                ty * tile_size,
                tile_size,
                tile_size,
                patch_size,
                max_num_patches,
            );
            pixel_values_tiles.push(pv);
            attention_mask_tiles.push(am);
            spatial_shapes.push(patches_per_tile_side as i64);
            spatial_shapes.push(patches_per_tile_side as i64);
        }
    }

    Ok(ImageInput {
        num_tiles: pixel_values_tiles.len(),
        pixel_values_tiles,
        pixel_attention_mask_tiles: attention_mask_tiles,
        spatial_shapes,
        max_num_patches,
    })
}

fn compute_target_ratios(min_tiles: usize, max_tiles: usize) -> Vec<(usize, usize)> {
    let mut ratios = Vec::new();
    for n in min_tiles..=max_tiles {
        for w in 1..=n {
            for h in 1..=n {
                if min_tiles <= w * h && w * h <= max_tiles {
                    ratios.push((w, h));
                }
            }
        }
    }
    ratios.sort_by_key(|x| x.0 * x.1);
    ratios.dedup();
    ratios
}

fn find_closest_aspect_ratio(
    aspect_ratio: f64,
    target_ratios: &[(usize, usize)],
    width: usize,
    height: usize,
    tile_size: usize,
) -> (usize, usize) {
    let mut best_ratio = target_ratios[0];
    let mut best_diff = f64::MAX;

    for &(w, h) in target_ratios {
        let diff = (w as f64 / h as f64 - aspect_ratio).abs();
        if diff < best_diff {
            best_diff = diff;
            best_ratio = (w, h);
        } else if diff == best_diff {
            let target_area = width * height;
            let ratio_area = w * h * tile_size * tile_size;
            let current_area = best_ratio.0 * best_ratio.1 * tile_size * tile_size;
            if (ratio_area as i64 - target_area as i64).unsigned_abs()
                < (current_area as i64 - target_area as i64).unsigned_abs()
            {
                best_ratio = (w, h);
            }
        }
    }

    best_ratio
}

/// Create the ONNX tensors for the vision encoder from preprocessed image data.
pub fn create_vision_inputs(
    input: &ImageInput,
) -> crate::Result<(
    ort::value::Tensor<f32>,
    ort::value::Tensor<i64>,
    ort::value::Tensor<i64>,
)> {
    let num_tiles = input.num_tiles;
    let max_num_patches = input.max_num_patches;

    // pixel_values: [num_tiles, max_num_patches, 768]
    let mut all_pixel_values = vec![0.0f32; num_tiles * max_num_patches * 768];
    for (i, tile_pv) in input.pixel_values_tiles.iter().enumerate() {
        let offset = i * max_num_patches * 768;
        all_pixel_values[offset..offset + tile_pv.len()].copy_from_slice(tile_pv);
    }
    let pixel_values_tensor = Tensor::from_array((
        vec![num_tiles as i64, max_num_patches as i64, 768i64],
        all_pixel_values.into_boxed_slice(),
    ))?;

    // pixel_attention_mask: [num_tiles, max_num_patches]
    let mut all_masks = vec![0i64; num_tiles * max_num_patches];
    for (i, tile_mask) in input.pixel_attention_mask_tiles.iter().enumerate() {
        let offset = i * max_num_patches;
        all_masks[offset..offset + tile_mask.len()].copy_from_slice(tile_mask);
    }
    let attention_mask_tensor = Tensor::from_array((
        vec![num_tiles as i64, max_num_patches as i64],
        all_masks.into_boxed_slice(),
    ))?;

    // spatial_shapes: [num_tiles, 2]
    let spatial_shapes_tensor = Tensor::from_array((
        vec![num_tiles as i64, 2i64],
        input.spatial_shapes.clone().into_boxed_slice(),
    ))?;

    Ok((
        pixel_values_tensor,
        attention_mask_tensor,
        spatial_shapes_tensor,
    ))
}
