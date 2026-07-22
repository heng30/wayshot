
use candle_core::{DType, Device, IndexOp, Tensor};
use candle_nn::VarBuilder;
use image::DynamicImage;

use crate::{
    models::paddleocr_vl::config::PaddleOCRVLPreprocessorConfig,
    utils::{
        img_utils::{img_smart_resize, img_transform},
    },
};

/// PaddleOCR-VL image processor
pub struct PaddleOCRVLProcessor {
    process_cfg: PaddleOCRVLPreprocessorConfig,
    device: Device,
    dtype: DType,
    image_token: String,
}

impl PaddleOCRVLProcessor {
    pub fn new(
        config: PaddleOCRVLPreprocessorConfig,
        device: &Device,
        dtype: DType,
    ) -> Result<Self, crate::Error> {
        let image_token = "<|IMAGE_PLACEHOLDER|>".to_string();
        Ok(Self {
            process_cfg: config,
            device: device.clone(),
            dtype,
            image_token,
        })
    }

    /// Process single image
    pub fn process_img(
        &self,
        img: &DynamicImage,
        img_mean: &Tensor,
        img_std: &Tensor,
    ) -> Result<Tensor, crate::Error> {
        let img_h = img.height();
        let img_w = img.width();
        // Resize h,w to multiples of 32
        let (resize_h, resize_w) = img_smart_resize(
            img_h,
            img_w,
            (self.process_cfg.patch_size * self.process_cfg.merge_size) as u32,
            self.process_cfg.min_pixels,
            self.process_cfg.max_pixels,
        )?;
        let img = img.resize_exact(resize_w, resize_h, image::imageops::FilterType::CatmullRom);
        let img_tensor = img_transform(&img, img_mean, img_std, &self.device, self.dtype)?;
        // (c, h, w) => (1, c, h, w)
        let img_tensor = img_tensor.unsqueeze(0)?;
        Ok(img_tensor)
    }

    /// Process vision tensor
    pub fn process_vision_tensor(&self, img_tensor: &Tensor) -> Result<(Tensor, Tensor), crate::Error> {
        let channel = img_tensor.dim(1)?;
        // img_tensor.dim[0] = 1, temporal_patch_size = 1, grid_t = 1
        let grid_t = img_tensor.dim(0)? / self.process_cfg.temporal_patch_size;
        let grid_h = img_tensor.dim(2)? / self.process_cfg.patch_size;
        let grid_w = img_tensor.dim(3)? / self.process_cfg.patch_size;
        let shape = candle_core::Shape::from(vec![
            grid_t,
            self.process_cfg.temporal_patch_size,
            channel,
            grid_h,
            self.process_cfg.patch_size,
            grid_w,
            self.process_cfg.patch_size,
        ]);
        let img_tensor = img_tensor.reshape(shape)?;
        let img_tensor = img_tensor.permute(vec![0, 3, 5, 2, 1, 4, 6])?;
        let img_tensor = img_tensor
            .reshape((
                grid_t * grid_h * grid_w,
                channel,
                self.process_cfg.patch_size,
                self.process_cfg.patch_size,
            ))?
            .contiguous()?;
        let grid_thw = Tensor::from_vec(
            vec![grid_t as u32, grid_h as u32, grid_w as u32],
            (1, 3),
            &self.device,
        )?;
        Ok((img_tensor, grid_thw))
    }

    /// Process multiple images
    pub fn process_images(
        &self,
        imgs: &Vec<DynamicImage>,
        img_mean: &Tensor,
        img_std: &Tensor,
    ) -> Result<(Tensor, Tensor), crate::Error> {
        let mut pixel_values_vec = Vec::new();
        let mut vision_grid_thws_vec = Vec::new();
        for img in imgs {
            let img_tensor = self.process_img(img, img_mean, img_std)?;
            let (img_tensor, grid_thw) = self.process_vision_tensor(&img_tensor)?;
            pixel_values_vec.push(img_tensor);
            vision_grid_thws_vec.push(grid_thw);
        }
        let pixel_values = Tensor::cat(&pixel_values_vec, 0)?;
        let vision_grid_thws = Tensor::cat(&vision_grid_thws_vec, 0)?;
        Ok((pixel_values, vision_grid_thws))
    }

    /// Process image with OCR prompt
    pub fn process_image_for_ocr(
        &self,
        img: &DynamicImage,
        prompt: &str,
        _cfg_image_token_id: u32,
    ) -> Result<(String, Tensor, Tensor), crate::Error> {
        let img_mean = Tensor::from_slice(&self.process_cfg.image_mean, (3, 1, 1), &self.device)?
            .to_dtype(self.dtype)?;
        let img_std = Tensor::from_slice(&self.process_cfg.image_std, (3, 1, 1), &self.device)?
            .to_dtype(self.dtype)?;

        let (pixel_values, image_grid_thw) = self.process_images(&vec![img.clone()], &img_mean, &img_std)?;

        // Build text with image placeholder
        let merge_length = self.process_cfg.merge_size.pow(2);
        let mut text = prompt.to_string();

        // Replace image placeholder with tokens
        if image_grid_thw.i(0)?.to_vec1::<u32>()?.first().is_some() {
            let grid_thw = image_grid_thw.i(0)?.to_vec1::<u32>()?;
            let repeat_num =
                grid_thw.iter().product::<u32>() as usize / merge_length;
            let replace = "<|placeholder|>".repeat(repeat_num);
            text = text.replace(&self.image_token, &replace);
            text = text.replace("<|placeholder|>", &self.image_token);
        }

        Ok((text, pixel_values, image_grid_thw))
    }
}

/// Load PaddleOCR-VL model
pub fn load_paddleocr_vl_model(
    model_path: &str,
    device: Option<&Device>,
    dtype: Option<DType>,
) -> Result<(crate::models::paddleocr_vl::model::PaddleOCRVLModel, crate::models::paddleocr_vl::config::PaddleOCRVLConfig, Device, DType), crate::Error> {
    use crate::utils::{get_device, get_dtype, find_type_files};

    let config_path = model_path.to_string() + "/config.json";
    let cfg: crate::models::paddleocr_vl::config::PaddleOCRVLConfig =
        serde_json::from_slice(&std::fs::read(config_path)?)?;
    let device = get_device(device);
    let cfg_dtype = cfg.torch_dtype.as_str();
    let dtype = get_dtype(dtype, cfg_dtype);

    let model_list = find_type_files(model_path, "safetensors")?;
    let vb = unsafe { VarBuilder::from_mmaped_safetensors(&model_list, dtype, &device)? };
    let model = crate::models::paddleocr_vl::model::PaddleOCRVLModel::new(cfg.clone(), vb, vec![2])?;

    Ok((model, cfg, device, dtype))
}