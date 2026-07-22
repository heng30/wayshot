use crate::{
    Result,
    metadata::{AudioMetadata, Metadata, SubtitleMetadata, VideoMetadata},
};
use ffmpeg_next as ffmpeg;
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoMetadataData {
    pub index: usize,
    pub codec_name: String,
    pub pix_fmt_name: String,
    pub width: u32,
    pub height: u32,
    pub fps: f32,
    pub language: Option<String>,
    pub duration_secs: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioMetadataData {
    pub index: usize,
    pub codec_name: String,
    pub sample_fmt_name: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub language: Option<String>,
    pub duration_secs: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubtitleMetadataData {
    pub index: usize,
    pub codec_name: String,
    pub language: Option<String>,
    pub duration_secs: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataData {
    pub path: PathBuf,
    pub size: u64,
    pub bitrate: u64,
    pub duration_secs: f64,
    pub format: Vec<String>,
    pub videos: Vec<VideoMetadataData>,
    pub audios: Vec<AudioMetadataData>,
    pub subtitles: Vec<SubtitleMetadataData>,
}

impl From<&VideoMetadata> for VideoMetadataData {
    fn from(meta: &VideoMetadata) -> Self {
        Self {
            index: meta.index,
            codec_name: codec_id_to_name(meta.codec_id),
            pix_fmt_name: pixel_format_to_name(meta.pix_fmt),
            width: meta.width,
            height: meta.height,
            fps: meta.fps,
            language: meta.language.clone(),
            duration_secs: meta.duration.as_secs_f64(),
        }
    }
}

impl From<&AudioMetadata> for AudioMetadataData {
    fn from(meta: &AudioMetadata) -> Self {
        Self {
            index: meta.index,
            codec_name: codec_id_to_name(meta.codec_id),
            sample_fmt_name: sample_format_to_name(meta.sample_format),
            sample_rate: meta.sample_rate,
            channels: meta.channels,
            language: meta.language.clone(),
            duration_secs: meta.duration.as_secs_f64(),
        }
    }
}

impl From<&SubtitleMetadata> for SubtitleMetadataData {
    fn from(meta: &SubtitleMetadata) -> Self {
        Self {
            index: meta.index,
            codec_name: codec_id_to_name(meta.codec_id),
            language: meta.language.clone(),
            duration_secs: meta.duration.as_secs_f64(),
        }
    }
}

impl From<&Metadata> for MetadataData {
    fn from(meta: &Metadata) -> Self {
        Self {
            path: meta.path.clone(),
            size: meta.size,
            bitrate: meta.bitrate,
            duration_secs: meta.duration.as_secs_f64(),
            format: meta.format.clone(),
            videos: meta.videos.iter().map(|v| v.into()).collect(),
            audios: meta.audios.iter().map(|a| a.into()).collect(),
            subtitles: meta.subtitles.iter().map(|s| s.into()).collect(),
        }
    }
}

impl TryFrom<&VideoMetadataData> for VideoMetadata {
    type Error = crate::Error;

    fn try_from(data: &VideoMetadataData) -> Result<Self> {
        Ok(Self {
            index: data.index,
            codec_id: name_to_codec_id(&data.codec_name)?,
            pix_fmt: name_to_pixel_format(&data.pix_fmt_name)?,
            width: data.width,
            height: data.height,
            fps: data.fps,
            language: data.language.clone(),
            duration: Duration::from_secs_f64(data.duration_secs),
        })
    }
}

impl TryFrom<&AudioMetadataData> for AudioMetadata {
    type Error = crate::Error;

    fn try_from(data: &AudioMetadataData) -> Result<Self> {
        Ok(Self {
            index: data.index,
            codec_id: name_to_codec_id(&data.codec_name)?,
            sample_format: name_to_sample_format(&data.sample_fmt_name)?,
            sample_rate: data.sample_rate,
            channels: data.channels,
            language: data.language.clone(),
            duration: Duration::from_secs_f64(data.duration_secs),
        })
    }
}

impl TryFrom<&SubtitleMetadataData> for SubtitleMetadata {
    type Error = crate::Error;

    fn try_from(data: &SubtitleMetadataData) -> Result<Self> {
        Ok(Self {
            index: data.index,
            codec_id: name_to_codec_id(&data.codec_name)?,
            language: data.language.clone(),
            duration: Duration::from_secs_f64(data.duration_secs),
        })
    }
}

impl TryFrom<&MetadataData> for Metadata {
    type Error = crate::Error;

    fn try_from(data: &MetadataData) -> Result<Self> {
        Ok(Self {
            path: data.path.clone(),
            size: data.size,
            bitrate: data.bitrate,
            duration: Duration::from_secs_f64(data.duration_secs),
            format: data.format.clone(),
            videos: data
                .videos
                .iter()
                .map(|v| v.try_into())
                .collect::<Result<Vec<_>>>()?,
            audios: data
                .audios
                .iter()
                .map(|a| a.try_into())
                .collect::<Result<Vec<_>>>()?,
            subtitles: data
                .subtitles
                .iter()
                .map(|s| s.try_into())
                .collect::<Result<Vec<_>>>()?,
        })
    }
}

impl TryFrom<MetadataData> for Metadata {
    type Error = crate::Error;

    fn try_from(data: MetadataData) -> Result<Self> {
        Metadata::try_from(&data)
    }
}

impl TryFrom<VideoMetadataData> for VideoMetadata {
    type Error = crate::Error;

    fn try_from(data: VideoMetadataData) -> Result<Self> {
        VideoMetadata::try_from(&data)
    }
}

impl TryFrom<AudioMetadataData> for AudioMetadata {
    type Error = crate::Error;

    fn try_from(data: AudioMetadataData) -> Result<Self> {
        AudioMetadata::try_from(&data)
    }
}

impl TryFrom<SubtitleMetadataData> for SubtitleMetadata {
    type Error = crate::Error;

    fn try_from(data: SubtitleMetadataData) -> Result<Self> {
        SubtitleMetadata::try_from(&data)
    }
}

fn codec_id_to_name(id: ffmpeg::codec::Id) -> String {
    match id {
        ffmpeg::codec::Id::None => "none",
        ffmpeg::codec::Id::MPEG1VIDEO => "mpeg1video",
        ffmpeg::codec::Id::MPEG2VIDEO => "mpeg2video",
        ffmpeg::codec::Id::H263 => "h263",
        ffmpeg::codec::Id::H264 => "h264",
        ffmpeg::codec::Id::HEVC => "hevc",
        ffmpeg::codec::Id::VP8 => "vp8",
        ffmpeg::codec::Id::VP9 => "vp9",
        ffmpeg::codec::Id::AV1 => "av1",
        ffmpeg::codec::Id::AAC => "aac",
        ffmpeg::codec::Id::MP3 => "mp3",
        ffmpeg::codec::Id::OPUS => "opus",
        ffmpeg::codec::Id::VORBIS => "vorbis",
        ffmpeg::codec::Id::FLAC => "flac",
        ffmpeg::codec::Id::AC3 => "ac3",
        ffmpeg::codec::Id::EAC3 => "eac3",
        ffmpeg::codec::Id::DTS => "dca",
        ffmpeg::codec::Id::PCM_S16LE => "pcm_s16le",
        ffmpeg::codec::Id::PCM_S16BE => "pcm_s16be",
        ffmpeg::codec::Id::SUBRIP => "subrip",
        ffmpeg::codec::Id::ASS => "ass",
        ffmpeg::codec::Id::SSA => "ssa",
        ffmpeg::codec::Id::SRT => "srt",
        ffmpeg::codec::Id::WEBVTT => "webvtt",
        _ => "unknown",
    }
    .to_string()
}

fn name_to_codec_id(name: &str) -> Result<ffmpeg::codec::Id> {
    let codec_id = match name {
        "none" => ffmpeg::codec::Id::None,
        "mpeg1video" => ffmpeg::codec::Id::MPEG1VIDEO,
        "mpeg2video" => ffmpeg::codec::Id::MPEG2VIDEO,
        "h263" => ffmpeg::codec::Id::H263,
        "h264" => ffmpeg::codec::Id::H264,
        "hevc" => ffmpeg::codec::Id::HEVC,
        "vp8" => ffmpeg::codec::Id::VP8,
        "vp9" => ffmpeg::codec::Id::VP9,
        "av1" => ffmpeg::codec::Id::AV1,
        "aac" => ffmpeg::codec::Id::AAC,
        "mp3" => ffmpeg::codec::Id::MP3,
        "opus" => ffmpeg::codec::Id::OPUS,
        "vorbis" => ffmpeg::codec::Id::VORBIS,
        "flac" => ffmpeg::codec::Id::FLAC,
        "ac3" => ffmpeg::codec::Id::AC3,
        "eac3" => ffmpeg::codec::Id::EAC3,
        "dca" => ffmpeg::codec::Id::DTS,
        "pcm_s16le" => ffmpeg::codec::Id::PCM_S16LE,
        "pcm_s16be" => ffmpeg::codec::Id::PCM_S16BE,
        "subrip" => ffmpeg::codec::Id::SUBRIP,
        "ass" => ffmpeg::codec::Id::ASS,
        "ssa" => ffmpeg::codec::Id::SSA,
        "srt" => ffmpeg::codec::Id::SRT,
        "webvtt" => ffmpeg::codec::Id::WEBVTT,
        "unknown" => ffmpeg::codec::Id::None,
        _ => {
            return Err(crate::Error::InvalidCodecName(name.to_string()));
        }
    };

    Ok(codec_id)
}

fn pixel_format_to_name(fmt: ffmpeg::format::Pixel) -> String {
    match fmt {
        ffmpeg::format::Pixel::None => "none",
        ffmpeg::format::Pixel::YUV420P => "yuv420p",
        ffmpeg::format::Pixel::YUYV422 => "yuyv422",
        ffmpeg::format::Pixel::RGB24 => "rgb24",
        ffmpeg::format::Pixel::BGR24 => "bgr24",
        ffmpeg::format::Pixel::YUV422P => "yuv422p",
        ffmpeg::format::Pixel::YUV444P => "yuv444p",
        ffmpeg::format::Pixel::YUV410P => "yuv410p",
        ffmpeg::format::Pixel::YUV411P => "yuv411p",
        ffmpeg::format::Pixel::GRAY8 => "gray",
        ffmpeg::format::Pixel::MonoWhite => "monow",
        ffmpeg::format::Pixel::MonoBlack => "monob",
        ffmpeg::format::Pixel::PAL8 => "pal8",
        ffmpeg::format::Pixel::YUVJ420P => "yuvj420p",
        ffmpeg::format::Pixel::YUVJ422P => "yuvj422p",
        ffmpeg::format::Pixel::YUVJ444P => "yuvj444p",
        ffmpeg::format::Pixel::NV12 => "nv12",
        ffmpeg::format::Pixel::NV21 => "nv21",
        ffmpeg::format::Pixel::ARGB => "argb",
        ffmpeg::format::Pixel::RGBA => "rgba",
        ffmpeg::format::Pixel::ABGR => "abgr",
        ffmpeg::format::Pixel::BGRA => "bgra",
        ffmpeg::format::Pixel::GRAY16LE => "gray16le",
        ffmpeg::format::Pixel::GRAY16BE => "gray16be",
        ffmpeg::format::Pixel::YUV440P => "yuv440p",
        ffmpeg::format::Pixel::YUVJ440P => "yuvj440p",
        ffmpeg::format::Pixel::YUVA420P => "yuva420p",
        ffmpeg::format::Pixel::RGB48LE => "rgb48le",
        ffmpeg::format::Pixel::RGB48BE => "rgb48be",
        ffmpeg::format::Pixel::RGBA64LE => "rgba64le",
        ffmpeg::format::Pixel::RGBA64BE => "rgba64be",
        _ => "unknown",
    }
    .to_string()
}

fn name_to_pixel_format(name: &str) -> Result<ffmpeg::format::Pixel> {
    let fmt = match name {
        "none" => ffmpeg::format::Pixel::None,
        "yuv420p" => ffmpeg::format::Pixel::YUV420P,
        "yuyv422" => ffmpeg::format::Pixel::YUYV422,
        "rgb24" => ffmpeg::format::Pixel::RGB24,
        "bgr24" => ffmpeg::format::Pixel::BGR24,
        "yuv422p" => ffmpeg::format::Pixel::YUV422P,
        "yuv444p" => ffmpeg::format::Pixel::YUV444P,
        "yuv410p" => ffmpeg::format::Pixel::YUV410P,
        "yuv411p" => ffmpeg::format::Pixel::YUV411P,
        "gray" => ffmpeg::format::Pixel::GRAY8,
        "monow" => ffmpeg::format::Pixel::MonoWhite,
        "monob" => ffmpeg::format::Pixel::MonoBlack,
        "pal8" => ffmpeg::format::Pixel::PAL8,
        "yuvj420p" => ffmpeg::format::Pixel::YUVJ420P,
        "yuvj422p" => ffmpeg::format::Pixel::YUVJ422P,
        "yuvj444p" => ffmpeg::format::Pixel::YUVJ444P,
        "nv12" => ffmpeg::format::Pixel::NV12,
        "nv21" => ffmpeg::format::Pixel::NV21,
        "argb" => ffmpeg::format::Pixel::ARGB,
        "rgba" => ffmpeg::format::Pixel::RGBA,
        "abgr" => ffmpeg::format::Pixel::ABGR,
        "bgra" => ffmpeg::format::Pixel::BGRA,
        "gray16le" => ffmpeg::format::Pixel::GRAY16LE,
        "gray16be" => ffmpeg::format::Pixel::GRAY16BE,
        "yuv440p" => ffmpeg::format::Pixel::YUV440P,
        "yuvj440p" => ffmpeg::format::Pixel::YUVJ440P,
        "yuva420p" => ffmpeg::format::Pixel::YUVA420P,
        "rgb48le" => ffmpeg::format::Pixel::RGB48LE,
        "rgb48be" => ffmpeg::format::Pixel::RGB48BE,
        "rgba64le" => ffmpeg::format::Pixel::RGBA64LE,
        "rgba64be" => ffmpeg::format::Pixel::RGBA64BE,
        "unknown" => ffmpeg::format::Pixel::None,
        _ => {
            return Err(crate::Error::InvalidPixelFormat(name.to_string()));
        }
    };

    Ok(fmt)
}

fn sample_format_to_name(fmt: ffmpeg::format::Sample) -> String {
    use ffmpeg::format::sample::Type;

    match fmt {
        ffmpeg::format::Sample::None => "none",
        ffmpeg::format::Sample::U8(Type::Packed) => "u8",
        ffmpeg::format::Sample::I16(Type::Packed) => "s16",
        ffmpeg::format::Sample::I32(Type::Packed) => "s32",
        ffmpeg::format::Sample::I64(Type::Packed) => "s64",
        ffmpeg::format::Sample::F32(Type::Packed) => "flt",
        ffmpeg::format::Sample::F64(Type::Packed) => "dbl",
        ffmpeg::format::Sample::U8(Type::Planar) => "u8p",
        ffmpeg::format::Sample::I16(Type::Planar) => "s16p",
        ffmpeg::format::Sample::I32(Type::Planar) => "s32p",
        ffmpeg::format::Sample::I64(Type::Planar) => "s64p",
        ffmpeg::format::Sample::F32(Type::Planar) => "fltp",
        ffmpeg::format::Sample::F64(Type::Planar) => "dblp",
    }
    .to_string()
}

fn name_to_sample_format(name: &str) -> Result<ffmpeg::format::Sample> {
    use ffmpeg::format::sample::Type;

    let fmt = match name {
        "none" => ffmpeg::format::Sample::None,
        "u8" => ffmpeg::format::Sample::U8(Type::Packed),
        "s16" => ffmpeg::format::Sample::I16(Type::Packed),
        "s32" => ffmpeg::format::Sample::I32(Type::Packed),
        "s64" => ffmpeg::format::Sample::I64(Type::Packed),
        "flt" => ffmpeg::format::Sample::F32(Type::Packed),
        "dbl" => ffmpeg::format::Sample::F64(Type::Packed),
        "u8p" => ffmpeg::format::Sample::U8(Type::Planar),
        "s16p" => ffmpeg::format::Sample::I16(Type::Planar),
        "s32p" => ffmpeg::format::Sample::I32(Type::Planar),
        "s64p" => ffmpeg::format::Sample::I64(Type::Planar),
        "fltp" => ffmpeg::format::Sample::F32(Type::Planar),
        "dblp" => ffmpeg::format::Sample::F64(Type::Planar),
        "unknown" => ffmpeg::format::Sample::None,
        _ => return Err(crate::Error::InvalidSampleFormat(name.to_string())),
    };

    Ok(fmt)
}

pub fn resolve_relative_path(path: &Path, base_dir: Option<&PathBuf>) -> PathBuf {
    if path.is_relative()
        && let Some(base) = base_dir
    {
        let resolved = base.join(path);
        if resolved.exists() {
            return resolved;
        }
    }
    path.to_path_buf()
}
