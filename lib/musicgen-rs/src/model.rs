const HF_BASE_URL: &str = "https://huggingface.co/gabotechs/music_gen/resolve/main";

/// A single file in a model's download set: `(remote_path, local_filename)`.
type FileSpec = (&'static str, &'static str);

/// Supported MusicGen model variants.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Model {
    Small,
    SmallFp16,
    SmallQuant,
    Medium,
    MediumFp16,
    MediumQuant,
    Large,
}

impl Model {
    /// List all available model variants.
    pub fn all_models() -> Vec<Self> {
        vec![
            Self::Small,
            Self::SmallFp16,
            Self::SmallQuant,
            Self::Medium,
            Self::MediumFp16,
            Self::MediumQuant,
            Self::Large,
        ]
    }

    /// Returns the file specifications for this model variant.
    ///
    /// Each spec is a `(remote_path, local_filename)` pair. The remote path
    /// is relative to the HuggingFace repository root.
    pub fn file_spec(&self) -> Vec<FileSpec> {
        match self {
            Model::Small => vec![
                ("small/config.json", "config.json"),
                ("small/tokenizer.json", "tokenizer.json"),
                ("small_fp32/text_encoder.onnx", "text_encoder.onnx"),
                (
                    "small_fp32/decoder_model_merged.onnx",
                    "decoder_model_merged.onnx",
                ),
                ("small_fp32/encodec_decode.onnx", "encodec_decode.onnx"),
            ],
            Model::SmallFp16 => vec![
                ("small/config.json", "config.json"),
                ("small/tokenizer.json", "tokenizer.json"),
                ("small_fp16/text_encoder.onnx", "text_encoder.onnx"),
                (
                    "small_fp16/decoder_model_merged.onnx",
                    "decoder_model_merged.onnx",
                ),
                ("small_fp16/encodec_decode.onnx", "encodec_decode.onnx"),
            ],
            Model::SmallQuant => vec![
                ("small/config.json", "config.json"),
                ("small/tokenizer.json", "tokenizer.json"),
                ("small_fp32/text_encoder.onnx", "text_encoder.onnx"),
                (
                    "small_i8/decoder_model_merged.onnx",
                    "decoder_model_merged.onnx",
                ),
                ("small_fp32/encodec_decode.onnx", "encodec_decode.onnx"),
            ],
            Model::Medium => vec![
                ("medium/config.json", "config.json"),
                ("medium/tokenizer.json", "tokenizer.json"),
                ("medium_fp32/text_encoder.onnx", "text_encoder.onnx"),
                (
                    "medium_fp32/decoder_model_merged.onnx",
                    "decoder_model_merged.onnx",
                ),
                (
                    "medium_fp32/decoder_model_merged.onnx_data",
                    "decoder_model_merged.onnx_data",
                ),
                ("medium_fp32/encodec_decode.onnx", "encodec_decode.onnx"),
            ],
            Model::MediumFp16 => vec![
                ("medium/config.json", "config.json"),
                ("medium/tokenizer.json", "tokenizer.json"),
                ("medium_fp16/text_encoder.onnx", "text_encoder.onnx"),
                (
                    "medium_fp16/decoder_model_merged.onnx",
                    "decoder_model_merged.onnx",
                ),
                (
                    "medium_fp16/decoder_model_merged.onnx_data",
                    "decoder_model_merged.onnx_data",
                ),
                ("medium_fp16/encodec_decode.onnx", "encodec_decode.onnx"),
            ],
            Model::MediumQuant => vec![
                ("medium/config.json", "config.json"),
                ("medium/tokenizer.json", "tokenizer.json"),
                ("medium_fp32/text_encoder.onnx", "text_encoder.onnx"),
                (
                    "medium_i8/decoder_model_merged.onnx",
                    "decoder_model_merged.onnx",
                ),
                ("medium_fp32/encodec_decode.onnx", "encodec_decode.onnx"),
            ],
            Model::Large => vec![
                ("large/config.json", "config.json"),
                ("large/tokenizer.json", "tokenizer.json"),
                ("large_fp32/text_encoder.onnx", "text_encoder.onnx"),
                (
                    "large_fp32/decoder_model_merged.onnx",
                    "decoder_model_merged.onnx",
                ),
                (
                    "large_fp32/decoder_model_merged.onnx_data",
                    "decoder_model_merged.onnx_data",
                ),
                ("large_fp32/encodec_decode.onnx", "encodec_decode.onnx"),
            ],
        }
    }

    /// Returns the directory name for this model variant (e.g. "small_fp32").
    pub fn dir_name(&self) -> &'static str {
        match self {
            Model::Small => "small_fp32",
            Model::SmallFp16 => "small_fp16",
            Model::SmallQuant => "small_i8",
            Model::Medium => "medium_fp32",
            Model::MediumFp16 => "medium_fp16",
            Model::MediumQuant => "medium_i8",
            Model::Large => "large_fp32",
        }
    }

    /// Returns the HuggingFace tree URL for browsing model files.
    pub fn browse_url(&self) -> String {
        format!(
            "https://huggingface.co/gabotechs/music_gen/tree/main/{}",
            self.dir_name()
        )
    }

    /// Returns the full download URL for a given file spec.
    pub fn download_url(&self, file_spec: &FileSpec) -> String {
        format!("{}/{}", HF_BASE_URL, file_spec.0)
    }

    /// Returns the local filenames that this model requires.
    pub fn local_filenames(&self) -> Vec<&'static str> {
        self.file_spec().iter().map(|(_, local)| *local).collect()
    }

    /// Try to identify a model variant from a local filename.
    ///
    /// This matches against the decoder ONNX filename which is unique per variant.
    pub fn try_from_local_filename(filename: &str) -> Option<Self> {
        // The decoder file is the most distinctive — all models share
        // config.json, tokenizer.json, and encodec_decode.onnx.
        // We match on the remote path prefix embedded in the download spec.
        for model in Self::all_models() {
            for (remote, _local) in model.file_spec() {
                // Match on the unique remote path component
                if remote.contains("decoder_model_merged")
                    && filename == "decoder_model_merged.onnx"
                    && !matches!(
                        model,
                        Model::Small
                            | Model::SmallFp16
                            | Model::SmallQuant
                            | Model::Medium
                            | Model::MediumFp16
                            | Model::MediumQuant
                            | Model::Large
                    )
                {
                    continue;
                }
                if remote.ends_with(filename) && remote.contains("decoder") {
                    // Use the remote path prefix to disambiguate
                    let prefix = remote.split('/').next()?;
                    let matched = match prefix {
                        "small_fp32" => Some(Model::Small),
                        "small_fp16" => Some(Model::SmallFp16),
                        "small_i8" => Some(Model::SmallQuant),
                        "medium_fp32" => Some(Model::Medium),
                        "medium_fp16" => Some(Model::MediumFp16),
                        "medium_i8" => Some(Model::MediumQuant),
                        "large_fp32" => Some(Model::Large),
                        _ => None,
                    };
                    if matched.is_some() {
                        return matched;
                    }
                }
            }
        }
        None
    }

    /// Try to identify a model variant from a HuggingFace download URL.
    pub fn try_from_url(url: &str) -> Option<Self> {
        if !url.starts_with(HF_BASE_URL) {
            return None;
        }
        let path = url.strip_prefix(HF_BASE_URL)?.strip_prefix('/')?;
        let prefix = path.split('/').next()?;
        match prefix {
            "small_fp32" => Some(Model::Small),
            "small_fp16" => Some(Model::SmallFp16),
            "small_i8" => Some(Model::SmallQuant),
            "medium_fp32" => Some(Model::Medium),
            "medium_fp16" => Some(Model::MediumFp16),
            "medium_i8" => Some(Model::MediumQuant),
            "large_fp32" => Some(Model::Large),
            _ => None,
        }
    }

    /// Whether this model uses fp16 (half-precision) decoder weights.
    pub fn is_fp16(&self) -> bool {
        matches!(self, Model::SmallFp16 | Model::MediumFp16)
    }

    /// Whether this model uses quantized (int8) decoder weights.
    pub fn is_quantized(&self) -> bool {
        matches!(self, Model::SmallQuant | Model::MediumQuant)
    }

    /// Whether this model requires an external `.onnx_data` companion file.
    pub fn has_onnx_data(&self) -> bool {
        matches!(self, Model::Medium | Model::MediumFp16 | Model::Large)
    }

    /// Returns the model size family name: "small", "medium", or "large".
    pub fn size_family(&self) -> &'static str {
        match self {
            Model::Small | Model::SmallFp16 | Model::SmallQuant => "small",
            Model::Medium | Model::MediumFp16 | Model::MediumQuant => "medium",
            Model::Large => "large",
        }
    }
}

impl std::fmt::Display for Model {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Model::Small => write!(f, "MusicGen Small"),
            Model::SmallFp16 => write!(f, "MusicGen Small Fp16"),
            Model::SmallQuant => write!(f, "MusicGen Small Quantized"),
            Model::Medium => write!(f, "MusicGen Medium"),
            Model::MediumFp16 => write!(f, "MusicGen Medium Fp16"),
            Model::MediumQuant => write!(f, "MusicGen Medium Quantized"),
            Model::Large => write!(f, "MusicGen Large"),
        }
    }
}
