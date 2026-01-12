use crate::{Error, Model, Result};
use fast_image_resize::{PixelType, ResizeOptions, Resizer, images::Image as FrImage};
use image::{GrayImage, ImageBuffer, RgbImage, Rgba, RgbaImage};
use ndarray::Array;
use ort::{session::Session, value::TensorRef};
use std::path::Path;

#[derive(Debug)]
#[non_exhaustive]
pub struct BackgroundRemover {
    input_size: (u32, u32),
    session: Session,
    input_name: String,
    output_names: Vec<String>,
}

impl BackgroundRemover {
    pub fn new<P: AsRef<Path>>(model: Model, model_path: P) -> Result<Self> {
        let model_path = model_path.as_ref();

        if !model_path.exists() {
            return Err(Error::ModelNotFound(model_path.to_path_buf()));
        }

        log::info!("Loading ONNX model from: {}", model_path.display());

        let session = Session::builder()?.commit_from_file(model_path)?;
        let input_name = Self::get_input_name(&session);
        let output_names: Vec<String> = session
            .outputs()
            .iter()
            .map(|output| output.name().to_string())
            .collect();

        Ok(Self {
            input_size: model.to_input_size(),
            session,
            input_name,
            output_names,
        })
    }

    pub fn input_size(&self) -> (u32, u32) {
        self.input_size
    }

    // mask is grayscale (0=background, 255=foreground)
    pub fn get_mask(&mut self, image: &RgbImage) -> Result<GrayImage> {
        let target_width = (self.input_size.0 / 2) * 2; // Ensure even
        let target_height = (self.input_size.1 / 2) * 2; // Ensure even

        let resized = self.fast_resize(image, target_width, target_height)?;
        let input_array = self.preprocess_image(&resized)?;
        let outputs = self.run_inference_inner(input_array)?;
        let mask = self.extract_mask(&outputs)?;

        self.fast_resize_mask(&mask, image.width(), image.height())
    }

    pub fn remove(&mut self, image: &RgbImage) -> Result<RgbaImage> {
        let mask = self.get_mask(image)?;
        Self::remove_background(image, &mask)
    }

    pub fn remove_with_mask(&mut self, image: &RgbImage) -> Result<(RgbaImage, GrayImage)> {
        let mask = self.get_mask(image)?;
        let result = Self::remove_background(image, &mask)?;
        Ok((result, mask))
    }

    pub fn remove_background(image: &RgbImage, mask: &GrayImage) -> Result<RgbaImage> {
        let (width, height) = image.dimensions();
        let mut result = RgbaImage::new(width, height);

        for y in 0..height {
            for x in 0..width {
                let pixel = image.get_pixel(x, y);
                let mask_pixel = mask.get_pixel(x, y);
                result.put_pixel(x, y, Rgba([pixel[0], pixel[1], pixel[2], mask_pixel[0]]));
            }
        }

        Ok(result)
    }

    fn fast_resize(
        &self,
        image: &RgbImage,
        target_width: u32,
        target_height: u32,
    ) -> Result<RgbImage> {
        let (width, height) = image.dimensions();
        if width == target_width && height == target_height {
            return Ok(RgbImage::from_raw(width, height, image.as_raw().clone())
                .ok_or_else(|| Error::ImageProcessing("Failed to clone RGB image".to_string()))?);
        }

        let src_image =
            FrImage::from_vec_u8(width, height, image.as_raw().clone(), PixelType::U8x3)?;
        let mut dst_image = FrImage::new(target_width, target_height, PixelType::U8x3);
        Resizer::new().resize(&src_image, &mut dst_image, &ResizeOptions::new())?;

        let rgb_image = RgbImage::from_raw(target_width, target_height, dst_image.into_vec())
            .ok_or_else(|| Error::ImageProcessing("Failed to create resized image".to_string()))?;

        Ok(rgb_image)
    }

    pub fn fast_resize_mask(
        &self,
        mask: &GrayImage,
        target_width: u32,
        target_height: u32,
    ) -> Result<GrayImage> {
        let (width, height) = mask.dimensions();
        if width == target_width && height == target_height {
            return Ok(mask.clone());
        }

        let src_image = FrImage::from_vec_u8(
            width,
            height,
            mask.pixels()
                .flat_map(|&p| vec![p[0], p[0], p[0], 255])
                .collect::<Vec<u8>>(),
            PixelType::U8x4,
        )?;
        let mut dst_image = FrImage::new(target_width, target_height, PixelType::U8x4);
        Resizer::new().resize(&src_image, &mut dst_image, &ResizeOptions::new())?;

        // Convert back to GrayImage (take only first channel)
        let dst_vec = dst_image.into_vec();
        let mut gray_data = Vec::with_capacity((target_width * target_height) as usize);
        for i in 0..dst_vec.len() / 4 {
            gray_data.push(dst_vec[i * 4]);
        }

        let gray_image = GrayImage::from_raw(target_width, target_height, gray_data)
            .ok_or_else(|| Error::ImageProcessing("Failed to create resized mask".to_string()))?;

        Ok(gray_image)
    }

    fn preprocess_image(&self, image: &RgbImage) -> Result<Array<f32, ndarray::Ix4>> {
        let (width, height) = image.dimensions();

        // Create array in NCHW format: (1, 3, H, W)
        let mut array = Array::zeros((1, 3, height as usize, width as usize));

        for y in 0..height {
            for x in 0..width {
                let pixel = image.get_pixel(x, y);
                // Normalize to [0, 1] and convert to CHW format
                array[[0, 0, y as usize, x as usize]] = pixel[0] as f32 / 255.0;
                array[[0, 1, y as usize, x as usize]] = pixel[1] as f32 / 255.0;
                array[[0, 2, y as usize, x as usize]] = pixel[2] as f32 / 255.0;
            }
        }

        Ok(array)
    }

    fn run_inference_inner(
        &mut self,
        input: Array<f32, ndarray::Ix4>,
    ) -> Result<ndarray::Array<f32, ndarray::IxDyn>> {
        let input_tensor = TensorRef::from_array_view(input.view())?;
        let outputs = self
            .session
            .run(ort::inputs! { &self.input_name => input_tensor })?;
        let mut output_array = None;
        let common_output_names = ["output", "mask", "foreground", "alpha"];

        for name in &common_output_names {
            if outputs.contains_key(name) {
                match outputs[*name].try_extract_array::<f32>() {
                    Ok(arr) => {
                        output_array = Some(arr.into_dyn());
                        break;
                    }
                    Err(_) => continue,
                }
            }
        }

        // fallback
        if output_array.is_none() {
            for output_name in &self.output_names {
                match outputs[output_name.as_str()].try_extract_array::<f32>() {
                    Ok(arr) => {
                        output_array = Some(arr.into_dyn());
                        break;
                    }
                    Err(_) => continue,
                }
            }
        }

        let output_array = output_array.ok_or_else(|| {
            Error::InvalidOutput("Failed to extract any output from model".to_string())
        })?;

        Ok(output_array.to_owned())
    }

    fn get_input_name(session: &Session) -> String {
        let common_names = vec!["input", "input.1", "image", "x"];
        let model_inputs: Vec<String> = session
            .inputs()
            .iter()
            .map(|input| input.name().to_string())
            .collect();

        for common_name in &common_names {
            if model_inputs.iter().any(|name| name == common_name) {
                return common_name.to_string();
            }
        }

        model_inputs
            .first()
            .cloned()
            .unwrap_or_else(|| "input".to_string())
    }

    fn extract_mask(
        &self,
        output_array: &ndarray::Array<f32, ndarray::IxDyn>,
    ) -> Result<GrayImage> {
        let shape = output_array.shape();
        let data = output_array.as_slice().unwrap();

        let (width, height) = match shape.len() {
            4 => (shape[3] as u32, shape[2] as u32), // Format: (1, 1, H, W) or (1, C, H, W)
            3 => (shape[2] as u32, shape[1] as u32), // Format: (1, H, W) or (C, H, W)
            2 => (shape[1] as u32, shape[0] as u32), // Format: (H, W)
            _ => {
                return Err(Error::InvalidOutput(format!(
                    "Unsupported output shape: {shape:?}",
                )));
            }
        };

        let mut mask = vec![0u8; (height * width) as usize];
        for i in 0..mask.len() {
            let clamped = data[i].max(0.0).min(1.0); // Clamp to [0, 1] and convert to 0-255
            mask[i] = (clamped * 255.0) as u8; // foreground (high) -> 255, background (low) -> 0
        }

        let mask_image: GrayImage = ImageBuffer::from_raw(width, height, mask)
            .ok_or_else(|| Error::ImageProcessing("Failed to create mask image".to_string()))?;

        Ok(mask_image)
    }

    /// Create binary mask image (0 = background, 255 = foreground) from grayscale mask
    pub fn create_binary_mask(mask: &GrayImage, threshold: u8) -> GrayImage {
        let (width, height) = mask.dimensions();
        let mut binary = GrayImage::new(width, height);

        for y in 0..height {
            for x in 0..width {
                let pixel = mask.get_pixel(x, y);
                let value = if pixel[0] > threshold { 255 } else { 0 };
                binary.put_pixel(x, y, image::Luma([value]));
            }
        }

        binary
    }
}
