//! PaddleOCR-RS: A Rust library for PaddleOCR-VL1.5 inference
//!
//! This library provides a simple API for OCR recognition using PaddleOCR-VL1.5 model.
//!
//! # Example
//!
//! ```rust,no_run
//! use paddle_ocr_rs::{PaddleOCR, OcrTask};
//!
//! # fn main() -> Result<(), paddle_ocr_rs::Error> {
//! // Load model
//! let mut ocr = PaddleOCR::new("path/to/model")?;
//!
//! // Perform OCR on an image (text only)
//! let text = ocr.ocr("path/to/image.jpg")?;
//! println!("OCR result: {}", text);
//!
//! // Perform OCR with position information
//! let result = ocr.ocr_with_task("path/to/image.jpg", OcrTask::Spotting)?;
//! for block in &result.blocks {
//!     if let Some(bbox) = block.bbox {
//!         println!("{}: {:?}", block.text, bbox);
//!     }
//! }
//! # Ok(())
//! # }
//! ```

pub mod models;
pub mod position_embed;
pub mod tokenizer;
pub mod utils;

use candle_core::{D, DType, Device, IndexOp, Tensor};

use image::DynamicImage;
use models::{
    common::{
        MultiModalData,
        generate::{GenerationContext, generate_generic_text, generate_generic_tokens},
    },
    paddleocr_vl::{
        config::PaddleOCRVLPreprocessorConfig,
        model::PaddleOCRVLModel,
        processor::{PaddleOCRVLProcessor, load_paddleocr_vl_model},
    },
};
use tokenizer::mod_tokenizer::TokenizerModel;
use utils::{
    img_utils::get_image,
    loc_parser::{BBox, TextBlock as LocTextBlock, parse_spotting_text},
    tensor_utils::get_equal_mask,
};

/// Error type for PaddleOCR operations
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to open image file: {0}")]
    ImageOpen(#[source] image::ImageError),
    #[error("failed to decode image: {0}")]
    ImageDecode(#[source] image::ImageError),
    #[error("image aspect ratio must be smaller than 200, got {0}")]
    ImageAspectRatioTooLarge(u32),
    #[error("tensor operation error: {0}")]
    Tensor(#[source] candle_core::Error),
    #[error("tokenizer error: {0}")]
    Tokenizer(String),
    #[error("tokenizer file not found: {0}")]
    TokenizerFileNotFound(String),
    #[error("IO error: {0}")]
    Io(#[source] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[source] serde_json::Error),
    #[error("invalid LOC token: {0}")]
    InvalidLocToken(u32),
    #[error("invalid tensor rank: expected {expected}, got {got}")]
    InvalidRank { expected: usize, got: usize },
    #[error("grid_thw expected exactly 3 elements")]
    InvalidGridThw,
    #[error(
        "multimodal data error: must have pixel_values, image_grid_thw, image_mask, cache_position"
    )]
    InvalidMultiModalData,
    #[error(
        "masked_scatter_dim0: original batch size {original_bs} or mask batch size {mask_bs} not equal to 1"
    )]
    InvalidBatchSize { original_bs: usize, mask_bs: usize },
    #[error("interpolation: {0} must be > 0")]
    InvalidInterpSize(String),
    #[error("interpolation: input rank must have 4 dimensions [b, c, h, w]")]
    InvalidInterpRank,
    #[error("index select: t and index rank must be equal to 2")]
    InvalidIndexSelectRank,
    #[error("GPU error: {0}")]
    Gpu(String),
    #[error("sampling error: {0}")]
    Sampling(String),
}

impl From<candle_core::Error> for Error {
    fn from(e: candle_core::Error) -> Self {
        Error::Tensor(e)
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::Json(e)
    }
}

/// OCR task type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OcrTask {
    /// Pure text recognition (no position information)
    Text,
    /// Text spotting with position information (bounding boxes)
    Spotting,
}

impl Default for OcrTask {
    fn default() -> Self {
        OcrTask::Text
    }
}

impl OcrTask {
    /// Get the prompt text for this task
    pub fn prompt_text(&self) -> &'static str {
        match self {
            OcrTask::Text => "OCR:",
            OcrTask::Spotting => "spotting",
        }
    }
}

/// OCR result containing text and optional position information
#[derive(Debug, Clone)]
pub struct OcrResult {
    /// Full recognized text
    pub text: String,
    /// Text blocks with optional position information
    pub blocks: Vec<TextBlock>,
}

/// Single text block with optional bounding box
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextBlock {
    /// Recognized text content
    pub text: String,
    /// Bounding box coordinates (x1, y1, x2, y2) in thousandths of image dimensions
    pub bbox: Option<BBox>,
}

impl TextBlock {
    /// Create a text block with position information
    pub fn with_bbox(text: String, bbox: BBox) -> Self {
        Self {
            text,
            bbox: Some(bbox),
        }
    }

    /// Create a text block without position information
    pub fn text_only(text: String) -> Self {
        Self { text, bbox: None }
    }

    /// Convert bbox to pixel coordinates
    pub fn bbox_pixels(&self, width: u32, height: u32) -> Option<(u32, u32, u32, u32)> {
        self.bbox.map(|b| b.to_pixels(width, height))
    }

    /// Convert bbox to normalized coordinates (0.0-1.0)
    pub fn bbox_normalized(&self) -> Option<(f64, f64, f64, f64)> {
        self.bbox.map(|b| b.to_normalized())
    }
}

impl From<LocTextBlock> for TextBlock {
    fn from(block: LocTextBlock) -> Self {
        TextBlock {
            text: block.text,
            bbox: block.bbox,
        }
    }
}

/// PaddleOCR struct for OCR recognition
pub struct PaddleOCR {
    model: PaddleOCRVLModel,
    tokenizer: TokenizerModel,
    processor: PaddleOCRVLProcessor,
    cfg: models::paddleocr_vl::config::PaddleOCRVLConfig,
    device: Device,
    #[allow(dead_code)]
    dtype: DType,
    default_prompt: String,
}

impl PaddleOCR {
    /// Load PaddleOCR-VL1.5 model from path
    ///
    /// # Arguments
    /// * `model_path` - Path to the model directory
    ///
    /// # Example
    /// ```rust,no_run
    /// use paddle_ocr_rs::PaddleOCR;
    /// # fn main() -> Result<(), paddle_ocr_rs::Error> {
    /// let mut ocr = PaddleOCR::new("~/.cache/paddleocr-vl1.5")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn new(model_path: &str) -> Result<Self, Error> {
        Self::new_with_options(model_path, None, None)
    }

    /// Load PaddleOCR-VL1.5 model with optional device and dtype
    ///
    /// # Arguments
    /// * `model_path` - Path to the model directory
    /// * `device` - Optional device (cuda/metal/cpu)
    /// * `dtype` - Optional data type (f16/bf16/f32)
    pub fn new_with_options(
        model_path: &str,
        device: Option<Device>,
        dtype: Option<DType>,
    ) -> Result<Self, Error> {
        let (model, cfg, device, dtype) =
            load_paddleocr_vl_model(model_path, device.as_ref(), dtype)?;

        // Load tokenizer
        let tokenizer = TokenizerModel::init(model_path)?;

        // Load processor config
        let processor_cfg_path = model_path.to_string() + "/preprocessor_config.json";
        let processor_cfg: PaddleOCRVLPreprocessorConfig =
            serde_json::from_slice(&std::fs::read(processor_cfg_path)?)?;
        let processor = PaddleOCRVLProcessor::new(processor_cfg, &device, dtype)?;

        let default_prompt = "OCR:";

        Ok(Self {
            model,
            tokenizer,
            processor,
            cfg,
            device,
            dtype,
            default_prompt: default_prompt.to_string(),
        })
    }

    /// Perform OCR on an image, returning recognized text
    ///
    /// # Arguments
    /// * `image_path` - Path to the image file
    ///
    /// # Example
    /// ```rust,no_run
    /// # fn main() -> Result<(), paddle_ocr_rs::Error> {
    /// # let mut ocr = paddle_ocr_rs::PaddleOCR::new("model")?;
    /// let text = ocr.ocr("image.jpg")?;
    /// println!("{}", text);
    /// # Ok(())
    /// # }
    /// ```
    pub fn ocr(&mut self, image_path: &str) -> Result<String, Error> {
        let prompt = self.default_prompt.clone();
        self.ocr_with_prompt(image_path, &prompt)
    }

    /// Perform OCR on an image with custom prompt
    ///
    /// # Arguments
    /// * `image_path` - Path to the image file
    /// * `prompt` - Custom prompt for OCR
    pub fn ocr_with_prompt(&mut self, image_path: &str, prompt: &str) -> Result<String, Error> {
        // Load image
        let img = get_image(image_path)?;
        self.ocr_with_prompt_inner(img, prompt)
    }

    pub fn ocr_with_prompt_inner(
        &mut self,
        img: DynamicImage,
        prompt: &str,
    ) -> Result<String, Error> {
        // Build prompt following chat template format
        let full_prompt = format!(
            "<|begin_of_sentence|>User: <|IMAGE_START|><|IMAGE_PLACEHOLDER|><|IMAGE_END|>{prompt}\nAssistant:\n"
        );

        // Process image
        let (text, pixel_values, image_grid_thw) =
            self.processor
                .process_image_for_ocr(&img, &full_prompt, self.cfg.image_token_id)?;

        // Encode text
        let input_ids = self.tokenizer.text_encode(text, &self.device)?;

        // Create image mask
        let image_mask = get_equal_mask(&input_ids, self.cfg.image_token_id)?;

        // Create cache position
        let cache_position = Tensor::ones_like(&input_ids.i(0)?)?
            .to_dtype(DType::F64)?
            .cumsum(D::Minus1)?
            .to_dtype(DType::U32)?
            .broadcast_sub(&Tensor::new(vec![1_u32], input_ids.device())?)?;

        // Create generation context
        let mut ctx = GenerationContext::new(
            Some(0.1), // temperature
            Some(0.9), // top_p
            None,      // top_k
            None,      // repeat_penalty
            None,      // repeat_last_n
            42,        // seed
            input_ids.dim(1)?,
            1024, // max_tokens
            self.device.clone(),
        );

        // Create multimodal data
        let data_vec = vec![
            Some(pixel_values),
            Some(image_grid_thw),
            Some(image_mask),
            Some(cache_position),
        ];
        let data = MultiModalData::new(data_vec);

        // Generate text
        let result =
            generate_generic_text(&mut self.model, &self.tokenizer, input_ids, data, &mut ctx)?;

        Ok(result)
    }

    /// Set custom default prompt for OCR
    pub fn set_default_prompt(&mut self, prompt: &str) {
        self.default_prompt = prompt.to_string();
    }

    /// Perform OCR with position information (spotting task)
    ///
    /// # Arguments
    /// * `image_path` - Path to the image file
    ///
    /// # Returns
    /// OcrResult containing text blocks with bounding boxes
    ///
    /// # Example
    /// ```rust,no_run
    /// # fn main() -> Result<(), paddle_ocr_rs::Error> {
    /// # let mut ocr = paddle_ocr_rs::PaddleOCR::new("model")?;
    /// let result = ocr.ocr_with_positions("image.jpg")?;
    /// for block in &result.blocks {
    ///     if let Some(bbox) = block.bbox {
    ///         println!("{} at ({}, {})", block.text, bbox.x1, bbox.y1);
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn ocr_with_positions(&mut self, image_path: &str) -> Result<OcrResult, Error> {
        self.ocr_with_task(image_path, OcrTask::Spotting)
    }

    /// Perform OCR with specified task type
    ///
    /// # Arguments
    /// * `image_path` - Path to the image file
    /// * `task` - OCR task type (Text or Spotting)
    ///
    /// # Returns
    /// OcrResult containing text and optional position information
    pub fn ocr_with_task(&mut self, image_path: &str, task: OcrTask) -> Result<OcrResult, Error> {
        let img = get_image(image_path)?;
        self.ocr_with_task_inner(img, task)
    }

    pub fn ocr_with_task_inner(
        &mut self,
        img: DynamicImage,
        task: OcrTask,
    ) -> Result<OcrResult, Error> {
        let prompt = task.prompt_text();

        // Build prompt following chat template format
        let full_prompt = format!(
            "<|begin_of_sentence|>User: <|IMAGE_START|><|IMAGE_PLACEHOLDER|><|IMAGE_END|>{prompt}\nAssistant:\n"
        );

        // Process image
        let (text, pixel_values, image_grid_thw) =
            self.processor
                .process_image_for_ocr(&img, &full_prompt, self.cfg.image_token_id)?;

        // Encode text
        let input_ids = self.tokenizer.text_encode(text, &self.device)?;

        // Create image mask
        let image_mask = get_equal_mask(&input_ids, self.cfg.image_token_id)?;

        // Create cache position
        let cache_position = Tensor::ones_like(&input_ids.i(0)?)?
            .to_dtype(DType::F64)?
            .cumsum(D::Minus1)?
            .to_dtype(DType::U32)?
            .broadcast_sub(&Tensor::new(vec![1_u32], input_ids.device())?)?;

        // Create generation context
        let mut ctx = GenerationContext::new(
            Some(0.1), // temperature
            Some(0.9), // top_p
            None,      // top_k
            None,      // repeat_penalty
            None,      // repeat_last_n
            42,        // seed
            input_ids.dim(1)?,
            1024, // max_tokens
            self.device.clone(),
        );

        // Create multimodal data
        let data_vec = vec![
            Some(pixel_values),
            Some(image_grid_thw),
            Some(image_mask),
            Some(cache_position),
        ];
        let data = MultiModalData::new(data_vec);

        // Generate tokens
        let tokens =
            generate_generic_tokens(&mut self.model, &self.tokenizer, input_ids, data, &mut ctx)?;

        // Process result based on task type
        match task {
            OcrTask::Text => {
                let text = self.tokenizer.token_decode(tokens)?;
                Ok(OcrResult {
                    text: text.clone(),
                    blocks: vec![TextBlock::text_only(text)],
                })
            }
            OcrTask::Spotting => {
                let raw_text = self.tokenizer.token_decode_with_special(tokens)?;
                let parsed = parse_spotting_text(&raw_text);
                let blocks: Vec<TextBlock> = parsed
                    .blocks
                    .into_iter()
                    .map(|b| TextBlock {
                        text: b.text,
                        bbox: b.bbox,
                    })
                    .collect();

                Ok(OcrResult {
                    text: parsed.full_text,
                    blocks,
                })
            }
        }
    }
}

