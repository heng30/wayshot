//! Image processor for LFM2.5-VL
//!
//! Accepts `image::RgbaImage` directly (not file paths / URLs).

use crate::lfm2vl::config::{Lfm2ImageConfig, Lfm2ProcessorConfig};
use crate::util::img_utils::{
    crop_img, find_closest_aspect_ratio, generate_target_ratios_sorted,
    img_smart_resize, img_transform,
};
use crate::util::round_by_factor;
use crate::error::Result;
use candle_core::{DType, Device, Tensor};
use image::DynamicImage;

pub struct Processor {
    dtype: DType,
    device: Device,
    image_config: Lfm2ImageConfig,
    max_num_patches: usize,
    total_factor: u32,
    max_pixel_num: usize,
    smart_resize_min_pixels: usize,
    smart_resize_max_pixels: usize,
    target_ratios: Vec<(u32, u32)>,
    img_mean: Tensor,
    img_std: Tensor,
    tokens_per_tile: usize,
    image_token: String,
    image_start_token: String,
    image_end_token: String,
    image_thumbnail_token: String,
}

#[allow(clippy::type_complexity)]
impl Processor {
    pub fn new(path: &str, dtype: DType, device: &Device, max_tiles_override: Option<usize>) -> Result<Self> {
        assert!(
            std::path::Path::new(path).exists(),
            "model path does not exist"
        );
        let processor_cfg_path = path.to_string() + "/processor_config.json";
        let processor_cfg =
            serde_json::from_slice::<Lfm2ProcessorConfig>(&std::fs::read(processor_cfg_path)?);

        let image_config = match processor_cfg {
            Ok(cfg) => cfg.image_processor,
            Err(_) => {
                let processor_cfg_path = path.to_string() + "/preprocessor_config.json";
                serde_json::from_slice::<Lfm2ImageConfig>(&std::fs::read(processor_cfg_path)?)?
            }
        };

        // Apply max_tiles override: generate target ratios with the capped value
        // and also cap smart_resize_max_pixels to avoid generating more tiles than needed.
        let effective_max_tiles = max_tiles_override.unwrap_or(image_config.max_tiles);
        let max_thumbnail_image_patches =
            image_config.max_image_tokens * image_config.downsample_factor.pow(2);
        let tile_size_patches = if image_config.do_image_splitting {
            (image_config.tile_size / image_config.encoder_patch_size).pow(2)
        } else {
            1
        };
        let max_num_patches = max_thumbnail_image_patches.max(tile_size_patches);
        let total_factor =
            (image_config.encoder_patch_size * image_config.downsample_factor) as u32;
        let token_pixels =
            image_config.encoder_patch_size.pow(2) * image_config.downsample_factor.pow(2);
        let max_pixel_num = ((image_config.max_image_tokens * token_pixels) as f64
            * image_config.max_pixels_tolerance) as usize;

        let smart_resize_min_pixels = image_config.min_image_tokens * token_pixels;
        let smart_resize_max_pixels = image_config.max_image_tokens * token_pixels;
        let target_ratios = generate_target_ratios_sorted(
            image_config.min_tiles as u32,
            effective_max_tiles as u32,
        );
        let img_mean =
            Tensor::from_slice(&image_config.image_mean, (3, 1, 1), device)?.to_dtype(dtype)?;
        let img_std =
            Tensor::from_slice(&image_config.image_std, (3, 1, 1), device)?.to_dtype(dtype)?;

        // Match HF's _compute_tokens_per_tile: num_patches = tile_size // patch_size,
        // then downsampled_patches = ceil(num_patches / downsample_factor)
        let num_patches_per_tile = image_config.tile_size / image_config.encoder_patch_size;
        let downsampled_patches = (num_patches_per_tile + image_config.downsample_factor - 1)
            / image_config.downsample_factor;
        let tokens_per_tile = downsampled_patches * downsampled_patches;

        Ok(Self {
            dtype,
            device: device.clone(),
            image_config,
            max_num_patches,
            total_factor,
            max_pixel_num,
            smart_resize_min_pixels,
            smart_resize_max_pixels,
            target_ratios,
            img_mean,
            img_std,
            tokens_per_tile,
            image_token: "<image>".to_string(),
            image_start_token: "<|image_start|>".to_string(),
            image_end_token: "<|image_end|>".to_string(),
            image_thumbnail_token: "<|img_thumbnail|>".to_string(),
        })
    }

    fn is_image_too_large(&self, height: u32, width: u32) -> bool {
        let h_bar = self
            .image_config
            .encoder_patch_size
            .max(round_by_factor(height, self.total_factor) as usize);
        let w_bar = self
            .image_config
            .encoder_patch_size
            .max(round_by_factor(width, self.total_factor) as usize);
        h_bar * w_bar > self.max_pixel_num
    }

    fn get_grid_layout(&self, height: u32, width: u32) -> (u32, u32) {
        let aspect_ratio = width as f64 / height as f64;
        let (grid_width, grid_height) = find_closest_aspect_ratio(
            aspect_ratio,
            &self.target_ratios,
            width,
            height,
            self.image_config.tile_size as u32,
        );
        (grid_width, grid_height)
    }

    fn crop_image_to_patches(
        &self,
        img: &DynamicImage,
        height: u32,
        width: u32,
        new_height: u32,
        new_width: u32,
    ) -> Result<(Vec<DynamicImage>, usize, usize)> {
        let (grid_width, grid_height) = self.get_grid_layout(height, width);
        let mut processed_images = crop_img(
            img,
            grid_height,
            grid_width,
            self.image_config.tile_size as u32,
        );
        if self.image_config.use_thumbnail && processed_images.len() != 1 {
            let thumbnail_img = img.resize_exact(
                new_width,
                new_height,
                image::imageops::FilterType::CatmullRom,
            );
            processed_images.push(thumbnail_img);
        }
        Ok((processed_images, grid_width as usize, grid_height as usize))
    }

    fn resize_and_split(
        &self,
        img: &DynamicImage,
    ) -> Result<(Vec<DynamicImage>, usize, usize, u32, u32)> {
        let height = img.height();
        let width = img.width();
        let is_image_large = self.is_image_too_large(height, width);

        // Convert to RGB before resizing to avoid alpha channel interference.
        // PIL / HF process in RGB; the image crate's resize on RGBA may
        // handle alpha differently (premultiplication etc.).
        let img = DynamicImage::ImageRgb8(img.to_rgb8());

        let (new_height, new_width) = img_smart_resize(
            height,
            width,
            self.total_factor,
            self.smart_resize_min_pixels as u32,
            self.smart_resize_max_pixels as u32,
        )?;
        let (images, num_cols, num_rows) = if is_image_large && self.image_config.do_image_splitting
        {
            self.crop_image_to_patches(&img, height, width, new_height, new_width)?
        } else {
            let img = img.resize_exact(
                new_width,
                new_height,
                image::imageops::FilterType::CatmullRom,
            );
            (vec![img], 1, 1)
        };

        Ok((images, num_cols, num_rows, new_height, new_width))
    }

    /// Process a list of `DynamicImage`s into model-ready tensors.
    ///
    /// Returns `(pixel_values, pixel_attention_mask, spatial_shapes,
    ///          num_cols_list, num_rows_list, image_size_list)`.
    pub fn process_imgs(
        &self,
        imgs: Vec<DynamicImage>,
    ) -> Result<(
        Tensor,
        Tensor,
        Tensor,
        Vec<usize>,
        Vec<usize>,
        Vec<(u32, u32)>,
    )> {
        let patch_size = self.image_config.encoder_patch_size;
        let mut images_list = vec![];
        let mut images_mask_list = vec![];
        let mut processed_spatial_shapes = vec![];
        let mut num_cols_list = vec![];
        let mut num_rows_list = vec![];
        let mut image_size_list = vec![];
        for img in &imgs {
            let (imgs, num_cols, num_rows, new_height, new_width) = self.resize_and_split(img)?;
            num_cols_list.push(num_cols);
            num_rows_list.push(num_rows);
            image_size_list.push((new_height, new_width));
            for img in imgs {
                let img_t = img_transform(
                    &img,
                    &self.img_mean,
                    &self.img_std,
                    &self.device,
                    self.dtype,
                )?;

                let (c, h, w) = img_t.dims3()?;
                let num_patches_height = h / patch_size;
                let num_patches_width = w / patch_size;
                // Patch the image into (num_patches, C*patch_size*patch_size).
                // The Siglip2 vision encoder uses nn.Linear (not Conv2d) for patch_embedding,
                // trained with channel-last layout from HF's convert_image_to_patches:
                // permute(1, 3, 2, 4, 0) → (num_h, num_w, patch, patch, C)
                let patched_image = img_t.reshape((
                    c,
                    num_patches_height,
                    patch_size,
                    num_patches_width,
                    patch_size,
                ))?;
                let patched_image = patched_image.permute((1, 3, 2, 4, 0))?;
                let patched_image =
                    patched_image.reshape((num_patches_height * num_patches_width, ()))?;

                let current_length = patched_image.dim(0)?;
                let padding_length = self.max_num_patches - current_length;
                let (patched_image, pixel_mask) = if self.image_config.do_pad
                    && padding_length > 0
                {
                    let mut pixel_mask =
                        Tensor::ones(current_length, DType::U32, &self.device)?;
                    let padding_image = patched_image.pad_with_zeros(0, 0, padding_length)?;
                    let pad = Tensor::zeros(padding_length, DType::U32, &self.device)?;
                    pixel_mask = Tensor::cat(&[&pixel_mask, &pad], 0)?;
                    (padding_image, pixel_mask)
                } else {
                    let pixel_mask = Tensor::ones(current_length, DType::U32, &self.device)?;
                    (patched_image, pixel_mask)
                };
                images_list.push(patched_image);
                images_mask_list.push(pixel_mask);
                processed_spatial_shapes
                    .push(vec![num_patches_height as u32, num_patches_width as u32]);
            }
        }
        let pixel_values = Tensor::stack(&images_list, 0)?;
        let pixel_attention_mask = Tensor::stack(&images_mask_list, 0)?;
        let spatial_shapes = Tensor::new(processed_spatial_shapes, &self.device)?;
        Ok((
            pixel_values,
            pixel_attention_mask,
            spatial_shapes,
            num_cols_list,
            num_rows_list,
            image_size_list,
        ))
    }

    fn build_image_tokens(&self, rows: usize, cols: usize, tokens_for_image: usize) -> String {
        let mut parts = String::new();
        parts += &self.image_start_token;
        if rows > 1 || cols > 1 {
            for row in 0..rows {
                for col in 0..cols {
                    parts += &format!("<|img_row_{}_col_{}|>", row + 1, col + 1);
                    parts += &self.image_token.repeat(self.tokens_per_tile);
                }
            }
            if self.image_config.use_thumbnail {
                parts += &self.image_thumbnail_token;
                parts += &self.image_token.repeat(tokens_for_image);
            }
        } else {
            parts += &self.image_token.repeat(tokens_for_image);
        }
        parts += &self.image_end_token;
        parts
    }

    /// Expand text template with image placeholder tokens.
    pub fn expand_text_with_placeholders(
        &self,
        text: &str,
        num_cols_list: Vec<usize>,
        num_rows_list: Vec<usize>,
        image_size_list: Vec<(u32, u32)>,
    ) -> String {
        let text_parts: Vec<&str> = text.split(&self.image_token).collect();
        let mut result_parts = String::new();
        for i in 0..num_cols_list.len() {
            result_parts += text_parts[i];
            let rows = num_rows_list[i];
            let cols = num_cols_list[i];
            let image_size = image_size_list[i];
            let (h, w) = image_size;
            let tokens_for_image = {
                // Match HF's _compute_tokens_for_image: integer division first,
                // then ceil division for downsample factor
                let patches_h = h as usize / self.image_config.encoder_patch_size;
                let patches_w = w as usize / self.image_config.encoder_patch_size;
                let ds_h = (patches_h + self.image_config.downsample_factor - 1)
                    / self.image_config.downsample_factor;
                let ds_w = (patches_w + self.image_config.downsample_factor - 1)
                    / self.image_config.downsample_factor;
                ds_h * ds_w
            };
            let sub_str = self.build_image_tokens(rows, cols, tokens_for_image);
            result_parts += &sub_str;
        }
        if text_parts.len() > num_cols_list.len() {
            result_parts += text_parts[text_parts.len() - 1];
        }
        result_parts
    }

    /// Full pipeline: images → tensors + expanded text.
    ///
    /// `text` should contain `<image>` placeholders (one per image).
    pub fn process_info(
        &self,
        imgs: Vec<DynamicImage>,
        text: &str,
    ) -> Result<(Tensor, Tensor, Tensor, String)> {
        let (
            pixel_values,
            pixel_attention_mask,
            spatial_shapes,
            num_cols_list,
            num_rows_list,
            image_size_list,
        ) = self.process_imgs(imgs)?;
        let text =
            self.expand_text_with_placeholders(text, num_cols_list, num_rows_list, image_size_list);
        Ok((pixel_values, pixel_attention_mask, spatial_shapes, text))
    }
}
