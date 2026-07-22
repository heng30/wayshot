use super::{
    codecs::{BitrateMode, CodecConfig, ExportPreset, QualityPreset},
    progress::CancellationToken,
};
use std::path::PathBuf;
pub use video_encoder::{CompressionPreset, Tune};

#[derive(Debug, Clone, derive_setters::Setters, derivative::Derivative)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct Mp4ExportConfig {
    #[derivative(Default(value = "std::path::PathBuf::from(\"output.mp4\")"))]
    pub output_path: std::path::PathBuf,

    pub cancellation_token: Option<CancellationToken>,

    pub width: Option<u32>,
    pub height: Option<u32>,
    pub fps: Option<u32>,

    #[derivative(Default(value = "true"))]
    pub use_crf: bool,
    pub crf: Option<u8>, // lower = better quality (0-51) default: 23
    pub tune: Option<Tune>,

    #[derivative(Default(value = "Some(CompressionPreset::Medium)"))]
    pub compression_preset: Option<CompressionPreset>,

    #[derivative(Default(value = "128_000"))]
    pub audio_bitrate: u32,
    pub audio_channels: Option<u16>,
    pub audio_sample_rate: Option<u32>,

    #[derivative(Default(value = "true"))]
    pub burn_subtitles: bool,

    pub low_memory_mode: bool,
}

impl Mp4ExportConfig {
    pub fn new_with_export_preset(preset: ExportPreset) -> Self {
        let config: CodecConfig = preset.config();

        let use_crf = matches!(config.bitrate_mode, BitrateMode::VBR { .. });
        let (crf, compression_preset) = match config.quality_preset {
            QualityPreset::Low => (Some(28), CompressionPreset::Faster),
            QualityPreset::Medium => (Some(23), CompressionPreset::Medium),
            QualityPreset::High => (Some(18), CompressionPreset::Slow),
            QualityPreset::Ultra => (Some(15), CompressionPreset::Slower),
        };

        Mp4ExportConfig {
            output_path: PathBuf::from("output.mp4"),
            cancellation_token: None,
            width: config.width,
            height: config.height,
            fps: config.fps,
            audio_bitrate: 128_000,
            audio_channels: None,
            audio_sample_rate: None,
            burn_subtitles: true,
            crf,
            compression_preset: Some(compression_preset),
            tune: None,
            use_crf,
            low_memory_mode: false,
        }
    }
}
