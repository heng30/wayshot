#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VideoCodec {
    #[default]
    H264,
    H265,
    AV1,
    ProRes,
    VP9,
}

impl VideoCodec {
    pub fn name(&self) -> &str {
        match self {
            Self::H264 => "H.264",
            Self::H265 => "H.265",
            Self::AV1 => "AV1",
            Self::ProRes => "ProRes",
            Self::VP9 => "VP9",
        }
    }

    pub fn extension(&self) -> &str {
        match self {
            Self::H264 | Self::H265 | Self::AV1 => ".mp4",
            Self::ProRes => ".mov",
            Self::VP9 => ".webm",
        }
    }

    pub fn ffmpeg_codec_id(&self) -> ffmpeg_next::codec::Id {
        match self {
            Self::H264 => ffmpeg_next::codec::Id::H264,
            Self::H265 => ffmpeg_next::codec::Id::HEVC,
            Self::AV1 => ffmpeg_next::codec::Id::AV1,
            Self::ProRes => ffmpeg_next::codec::Id::PRORES,
            Self::VP9 => ffmpeg_next::codec::Id::VP9,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BitrateMode {
    CBR(u32),                      // Constant Bitrate
    VBR { target: u32, max: u32 }, // Variable Bitrate
}

impl Default for BitrateMode {
    fn default() -> Self {
        BitrateMode::VBR {
            target: 5_000_000,
            max: 8_000_000,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum QualityPreset {
    Low,
    #[default]
    Medium,
    High,
    Ultra,
}

impl QualityPreset {
    pub fn bitrate_multiplier(&self) -> f64 {
        match self {
            Self::Low => 1.0,
            Self::Medium => 1.5,
            Self::High => 3.0,
            Self::Ultra => 6.0,
        }
    }
}

#[derive(Debug, Clone, derive_setters::Setters, derivative::Derivative)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct CodecConfig {
    #[derivative(Default(value = "VideoCodec::H264"))]
    pub codec: VideoCodec,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub fps: Option<u32>,
    pub bitrate_mode: BitrateMode,
    pub quality_preset: QualityPreset,
}

#[derive(Debug, Clone, Default)]
pub enum ExportPreset {
    #[default]
    YouTube1080p,
    YouTube720p,
    YouTube480p,
    Vimeo1080p,
    Vimeo720p,
    Twitter1080p,
    Twitter720p,
    Instagram1080p,
    Instagram720p,
    Custom {
        name: String,
        config: CodecConfig,
    },
}

impl ExportPreset {
    pub fn name(&self) -> &str {
        match self {
            Self::YouTube1080p => "YouTube 1080p",
            Self::YouTube720p => "YouTube 720p",
            Self::YouTube480p => "YouTube 480p",
            Self::Vimeo1080p => "Vimeo 1080p",
            Self::Vimeo720p => "Vimeo 720p",
            Self::Twitter1080p => "Twitter 1080p",
            Self::Twitter720p => "Twitter 720p",
            Self::Instagram1080p => "Instagram 1080p",
            Self::Instagram720p => "Instagram 720p",
            Self::Custom { name, .. } => name,
        }
    }

    pub fn config(&self) -> CodecConfig {
        match self {
            Self::YouTube1080p => CodecConfig {
                codec: VideoCodec::H264,
                width: Some(1920),
                height: Some(1080),
                fps: Some(30),
                bitrate_mode: BitrateMode::VBR {
                    target: 8_000_000,
                    max: 12_000_000,
                },
                quality_preset: QualityPreset::High,
            },
            Self::YouTube720p => CodecConfig {
                codec: VideoCodec::H264,
                width: Some(1280),
                height: Some(720),
                fps: Some(30),
                bitrate_mode: BitrateMode::VBR {
                    target: 5_000_000,
                    max: 8_000_000,
                },
                quality_preset: QualityPreset::High,
            },
            Self::YouTube480p => CodecConfig {
                codec: VideoCodec::H264,
                width: Some(854),
                height: Some(480),
                fps: Some(30),
                bitrate_mode: BitrateMode::VBR {
                    target: 2_000_000,
                    max: 3_000_000,
                },
                quality_preset: QualityPreset::Medium,
            },
            Self::Vimeo1080p => CodecConfig {
                codec: VideoCodec::H264,
                width: Some(1920),
                height: Some(1080),
                fps: Some(30),
                bitrate_mode: BitrateMode::VBR {
                    target: 10_000_000,
                    max: 15_000_000,
                },
                quality_preset: QualityPreset::High,
            },
            Self::Vimeo720p => CodecConfig {
                codec: VideoCodec::H264,
                width: Some(1280),
                height: Some(720),
                fps: Some(30),
                bitrate_mode: BitrateMode::VBR {
                    target: 6_000_000,
                    max: 10_000_000,
                },
                quality_preset: QualityPreset::High,
            },
            Self::Twitter1080p => CodecConfig {
                codec: VideoCodec::H264,
                width: Some(1920),
                height: Some(1080),
                fps: Some(30),
                bitrate_mode: BitrateMode::CBR(8_000_000),
                quality_preset: QualityPreset::High,
            },
            Self::Twitter720p => CodecConfig {
                codec: VideoCodec::H264,
                width: Some(1280),
                height: Some(720),
                fps: Some(30),
                bitrate_mode: BitrateMode::CBR(5_000_000),
                quality_preset: QualityPreset::High,
            },
            Self::Instagram1080p => CodecConfig {
                codec: VideoCodec::H264,
                width: Some(1920),
                height: Some(1080),
                fps: Some(30),
                bitrate_mode: BitrateMode::VBR {
                    target: 10_000_000,
                    max: 15_000_000,
                },
                quality_preset: QualityPreset::High,
            },
            Self::Instagram720p => CodecConfig {
                codec: VideoCodec::H264,
                width: Some(1280),
                height: Some(720),
                fps: Some(30),
                bitrate_mode: BitrateMode::VBR {
                    target: 6_000_000,
                    max: 10_000_000,
                },
                quality_preset: QualityPreset::High,
            },
            Self::Custom { config, .. } => config.clone(),
        }
    }
}

pub fn get_all_presets() -> Vec<ExportPreset> {
    vec![
        ExportPreset::YouTube1080p,
        ExportPreset::YouTube720p,
        ExportPreset::YouTube480p,
        ExportPreset::Vimeo1080p,
        ExportPreset::Vimeo720p,
        ExportPreset::Twitter1080p,
        ExportPreset::Twitter720p,
        ExportPreset::Instagram1080p,
        ExportPreset::Instagram720p,
    ]
}

pub fn get_preset_by_name(name: &str) -> Option<ExportPreset> {
    match name {
        "YouTube 1080p" => Some(ExportPreset::YouTube1080p),
        "YouTube 720p" => Some(ExportPreset::YouTube720p),
        "YouTube 480p" => Some(ExportPreset::YouTube480p),
        "Vimeo 1080p" => Some(ExportPreset::Vimeo1080p),
        "Vimeo 720p" => Some(ExportPreset::Vimeo720p),
        "Twitter 1080p" => Some(ExportPreset::Twitter1080p),
        "Twitter 720p" => Some(ExportPreset::Twitter720p),
        "Instagram 1080p" => Some(ExportPreset::Instagram1080p),
        "Instagram 720p" => Some(ExportPreset::Instagram720p),
        _ => None,
    }
}
