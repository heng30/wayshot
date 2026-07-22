use ort::session::Session;
use std::path::Path;

const ROOT_URL: &str = "https://huggingface.co/LiquidAI/LFM2.5-VL-450M-ONNX/resolve/main";
const BROWSE_URL: &str = "https://huggingface.co/LiquidAI/LFM2.5-VL-450M-ONNX/tree/main";

const VISION_ENCODER_FILENAME: &str = "vision_encoder.onnx";
const VISION_ENCODER_FP16_FILENAME: &str = "vision_encoder_fp16.onnx";
const VISION_ENCODER_Q4_FILENAME: &str = "vision_encoder_q4.onnx";
const VISION_ENCODER_Q8_FILENAME: &str = "vision_encoder_q8.onnx";

const EMBED_TOKENS_FILENAME: &str = "embed_tokens.onnx";
const EMBED_TOKENS_FP16_FILENAME: &str = "embed_tokens_fp16.onnx";

const DECODER_FILENAME: &str = "decoder_model_merged.onnx";
const DECODER_FP16_FILENAME: &str = "decoder_model_merged_fp16.onnx";
const DECODER_Q4_FILENAME: &str = "decoder_model_merged_q4.onnx";
const DECODER_Q8_FILENAME: &str = "decoder_model_merged_q8.onnx";

// External data files (.onnx_data) — all .onnx files except fp32 embed_tokens use external data
const VISION_ENCODER_DATA_FILENAME: &str = "vision_encoder.onnx_data";
const VISION_ENCODER_FP16_DATA_FILENAME: &str = "vision_encoder_fp16.onnx_data";
const VISION_ENCODER_Q4_DATA_FILENAME: &str = "vision_encoder_q4.onnx_data";
const VISION_ENCODER_Q8_DATA_FILENAME: &str = "vision_encoder_q8.onnx_data";
const EMBED_TOKENS_FP16_DATA_FILENAME: &str = "embed_tokens_fp16.onnx_data";
const DECODER_DATA_FILENAME: &str = "decoder_model_merged.onnx_data";
const DECODER_FP16_DATA_FILENAME: &str = "decoder_model_merged_fp16.onnx_data";
const DECODER_Q4_DATA_FILENAME: &str = "decoder_model_merged_q4.onnx_data";
const DECODER_Q8_DATA_FILENAME: &str = "decoder_model_merged_q8.onnx_data";

// Shared config files (in repo root, not in onnx/ subdirectory)
const CONFIG_FILENAME: &str = "config.json";
const TOKENIZER_FILENAME: &str = "tokenizer.json";
const TOKENIZER_CONFIG_FILENAME: &str = "tokenizer_config.json";
const GENERATION_CONFIG_FILENAME: &str = "generation_config.json";
const CHAT_TEMPLATE_FILENAME: &str = "chat_template.jinja";
const PREPROCESSOR_CONFIG_FILENAME: &str = "preprocessor_config.json";
const PROCESSOR_CONFIG_FILENAME: &str = "processor_config.json";

/// Model precision variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Precision {
    Fp32,
    Fp16,
    Q4,
    Q8,
}

impl Precision {
    pub fn all_precisions() -> Vec<Self> {
        vec![Self::Fp32, Self::Fp16, Self::Q4, Self::Q8]
    }

    pub fn to_name(&self) -> &'static str {
        match self {
            Self::Fp32 => "fp32",
            Self::Fp16 => "fp16",
            Self::Q4 => "q4",
            Self::Q8 => "q8",
        }
    }

    pub fn try_from_name(name: &str) -> Option<Self> {
        match name {
            "fp32" => Some(Self::Fp32),
            "fp16" => Some(Self::Fp16),
            "q4" => Some(Self::Q4),
            "q8" => Some(Self::Q8),
            _ => None,
        }
    }

    /// Returns the (vision_encoder, embed_tokens, decoder) ONNX filenames for this precision.
    pub fn onnx_filenames(&self) -> (&'static str, &'static str, &'static str) {
        match self {
            Self::Fp32 => (
                VISION_ENCODER_FILENAME,
                EMBED_TOKENS_FILENAME,
                DECODER_FILENAME,
            ),
            Self::Fp16 => (
                VISION_ENCODER_FP16_FILENAME,
                EMBED_TOKENS_FP16_FILENAME,
                DECODER_FP16_FILENAME,
            ),
            Self::Q4 => (
                VISION_ENCODER_Q4_FILENAME,
                EMBED_TOKENS_FP16_FILENAME,
                DECODER_Q4_FILENAME,
            ),
            Self::Q8 => (
                VISION_ENCODER_Q8_FILENAME,
                EMBED_TOKENS_FP16_FILENAME,
                DECODER_Q8_FILENAME,
            ),
        }
    }

    /// Returns all filenames needed for download, including .onnx_data external data files.
    /// fp32 embed_tokens.onnx has no external data (weights are inline).
    pub fn all_download_filenames(&self) -> Vec<&'static str> {
        let (ve, et, dec) = self.onnx_filenames();
        let mut files = vec![ve, et, dec];

        // Add .onnx_data files for each .onnx that uses external data
        match self {
            Self::Fp32 => {
                files.extend_from_slice(&[
                    VISION_ENCODER_DATA_FILENAME,
                    // no embed_tokens.onnx_data for fp32
                    DECODER_DATA_FILENAME,
                ]);
            }
            Self::Fp16 => {
                files.extend_from_slice(&[
                    VISION_ENCODER_FP16_DATA_FILENAME,
                    EMBED_TOKENS_FP16_DATA_FILENAME,
                    DECODER_FP16_DATA_FILENAME,
                ]);
            }
            Self::Q4 => {
                files.extend_from_slice(&[
                    VISION_ENCODER_Q4_DATA_FILENAME,
                    EMBED_TOKENS_FP16_DATA_FILENAME,
                    DECODER_Q4_DATA_FILENAME,
                ]);
            }
            Self::Q8 => {
                files.extend_from_slice(&[
                    VISION_ENCODER_Q8_DATA_FILENAME,
                    EMBED_TOKENS_FP16_DATA_FILENAME,
                    DECODER_Q8_DATA_FILENAME,
                ]);
            }
        }

        files
    }

    /// Returns all download URLs needed for this precision variant, including .onnx_data files.
    pub fn download_urls(&self) -> Vec<String> {
        self.all_download_filenames()
            .iter()
            .map(|f| format!("{ROOT_URL}/onnx/{f}"))
            .collect()
    }

    /// Returns all ONNX filenames needed for this precision variant (without .onnx_data).
    pub fn all_filenames(&self) -> Vec<&'static str> {
        let (ve, et, dec) = self.onnx_filenames();
        vec![ve, et, dec]
    }

    /// Returns all download URLs needed for this precision variant, including .onnx_data files.
    pub fn all_download_urls(&self) -> Vec<String> {
        self.download_urls()
    }

    /// Returns shared config filenames that all precisions need (in repo root, not onnx/ dir).
    pub fn shared_config_filenames() -> Vec<&'static str> {
        vec![
            CONFIG_FILENAME,
            TOKENIZER_FILENAME,
            TOKENIZER_CONFIG_FILENAME,
            GENERATION_CONFIG_FILENAME,
            CHAT_TEMPLATE_FILENAME,
            PREPROCESSOR_CONFIG_FILENAME,
            PROCESSOR_CONFIG_FILENAME,
        ]
    }

    /// Returns download URLs for shared config files (in repo root).
    pub fn shared_config_urls() -> Vec<String> {
        Self::shared_config_filenames()
            .iter()
            .map(|f| format!("{ROOT_URL}/{f}"))
            .collect()
    }

    /// Returns the base browse URL (for opening in browser).
    pub fn browser_url() -> &'static str {
        BROWSE_URL
    }

    /// Check if the model directory contains all required files for this precision.
    /// Returns a list of missing filenames (empty = all present).
    pub fn missing_files(&self, model_dir: &Path) -> Vec<String> {
        let mut missing = Vec::new();

        // Check shared config files in model root
        for filename in Self::shared_config_filenames() {
            if !model_dir.join(filename).exists() {
                missing.push(filename.to_string());
            }
        }

        // Check onnx files in onnx/ subdirectory
        let onnx_dir = model_dir.join("onnx");
        for filename in self.all_download_filenames() {
            if !onnx_dir.join(filename).exists() {
                missing.push(format!("onnx/{filename}"));
            }
        }

        missing
    }

    /// Try to match a download URL to a specific ONNX file and return its filename.
    pub fn try_filename_from_url(url: &str) -> Option<&'static str> {
        for prec in Self::all_precisions() {
            for filename in prec.all_filenames() {
                if url.ends_with(filename) {
                    return Some(filename);
                }
            }
        }
        None
    }
}

/// Static model configuration derived from config.json.
#[derive(Debug, Clone)]
pub struct ModelConfig {
    pub hidden_size: usize,
    pub num_kv_heads: usize,
    pub head_dim: usize,
    pub vocab_size: usize,
    pub image_token_id: u32,
    pub eos_token_id: u32,
    pub tile_size: usize,
    pub patch_size: usize,
    pub num_layers: usize,
    pub conv_layers: Vec<usize>,
    pub attn_layers: Vec<usize>,
    downsample_factor: usize,
    min_tiles_val: usize,
    max_tiles_val: usize,
    use_thumbnail_val: bool,
    do_image_splitting_val: bool,
    min_image_tokens_val: usize,
    max_image_tokens_val: usize,
    max_pixels_tolerance_val: f64,
    max_num_patches_val: usize,
}

impl ModelConfig {
    pub fn downsample_factor(&self) -> usize {
        self.downsample_factor
    }
    pub fn min_tiles(&self) -> usize {
        self.min_tiles_val
    }
    pub fn max_tiles(&self) -> usize {
        self.max_tiles_val
    }
    pub fn use_thumbnail(&self) -> bool {
        self.use_thumbnail_val
    }
    pub fn do_image_splitting(&self) -> bool {
        self.do_image_splitting_val
    }
    pub fn min_image_tokens(&self) -> usize {
        self.min_image_tokens_val
    }
    pub fn max_image_tokens(&self) -> usize {
        self.max_image_tokens_val
    }
    pub fn max_pixels_tolerance(&self) -> f64 {
        self.max_pixels_tolerance_val
    }
    pub fn max_num_patches(&self) -> usize {
        self.max_num_patches_val
    }
}

impl Default for ModelConfig {
    fn default() -> Self {
        let encoder_patch_size = 16;
        let downsample_factor = 2;
        let max_image_tokens = 256;
        let tile_size_patches = ((512 / encoder_patch_size) as usize).pow(2); // 1024
        let max_thumbnail_patches = max_image_tokens * downsample_factor * downsample_factor; // 1024
        let max_num_patches = tile_size_patches.max(max_thumbnail_patches);

        Self {
            hidden_size: 1024,
            num_kv_heads: 8,
            head_dim: 64,
            vocab_size: 65536,
            image_token_id: 396,
            eos_token_id: 7,
            tile_size: 512,
            patch_size: encoder_patch_size,
            num_layers: 16,
            conv_layers: vec![0, 1, 3, 4, 6, 7, 9, 11, 13, 15],
            attn_layers: vec![2, 5, 8, 10, 12, 14],
            downsample_factor,
            min_tiles_val: 2,
            max_tiles_val: 10,
            use_thumbnail_val: true,
            do_image_splitting_val: true,
            min_image_tokens_val: 64,
            max_image_tokens_val: 256,
            max_pixels_tolerance_val: 2.0,
            max_num_patches_val: max_num_patches,
        }
    }
}

/// The LFM2.5-VL-450M model loaded from ONNX files.
pub struct LfmVlModel {
    pub vision_encoder: Session,
    pub embed_tokens: Session,
    pub decoder: Session,
    pub config: ModelConfig,
}

impl LfmVlModel {
    /// Load the model from the given model directory with the specified precision.
    pub fn load(model_dir: &Path, precision: Precision) -> crate::Result<Self> {
        let onnx_dir = model_dir.join("onnx");

        let (vision_encoder_name, embed_tokens_name, decoder_name) = precision.onnx_filenames();

        let vision_encoder =
            Session::builder()?.commit_from_file(onnx_dir.join(vision_encoder_name))?;
        let embed_tokens =
            Session::builder()?.commit_from_file(onnx_dir.join(embed_tokens_name))?;
        let decoder = Session::builder()?.commit_from_file(onnx_dir.join(decoder_name))?;

        Ok(Self {
            vision_encoder,
            embed_tokens,
            decoder,
            config: ModelConfig::default(),
        })
    }
}
