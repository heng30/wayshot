use crate::metadata::{Metadata, MetadataType};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MediaType {
    Video,
    Audio,
    Image,
    Subtitle,
}

impl MediaType {
    pub fn from_metadata(metadata: &Metadata) -> Self {
        match metadata.get_type() {
            MetadataType::Video => MediaType::Video,
            MetadataType::Audio => MediaType::Audio,
            MetadataType::Subtitle => MediaType::Subtitle,
            MetadataType::Image => MediaType::Image,
            MetadataType::None => MediaType::Video,
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            MediaType::Video => "video",
            MediaType::Audio => "audio",
            MediaType::Image => "image",
            MediaType::Subtitle => "subtitle",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "video" => Some(MediaType::Video),
            "audio" => Some(MediaType::Audio),
            "image" => Some(MediaType::Image),
            "subtitle" => Some(MediaType::Subtitle),
            _ => None,
        }
    }

    pub fn as_u8(&self) -> u8 {
        match self {
            MediaType::Video => 0,
            MediaType::Audio => 1,
            MediaType::Image => 2,
            MediaType::Subtitle => 3,
        }
    }
}
