use super::{MP4PlayerError, Result};
use mp4::{AudioObjectType, AvcProfile};
use std::{fs::File, io::BufReader, path::Path, time::Duration};

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub enum SupportedCodec {
    H264,
    ACC,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub enum SupportedPixelFormat {
    YUV420P,
    Unsupported,
}

#[derive(Debug, Clone)]
pub struct VideoMetadata {
    pub track_id: u32,
    pub codec: SupportedCodec,
    pub width: u32,
    pub height: u32,
    pub frame_rate: f64,
    pub bitrate: u32,
    pub timescale: u32,
    pub duration: Duration,
    pub sample_count: u32,
    pub pixel_format: SupportedPixelFormat,
    pub video_profile: String,
    pub box_type: String,
}

#[derive(Debug, Clone)]
pub struct AudioMetadata {
    pub track_id: u32,
    pub codec: SupportedCodec,
    pub sample_rate: u32,
    pub channels: u16,
    pub bitrate: u32,
    pub timescale: u32,
    pub duration: Duration,
    pub sample_count: u32,
    pub audio_profile: String,
    pub box_type: String,
}

#[derive(Debug, Clone)]
pub struct MediaMetadata {
    pub video: Option<VideoMetadata>,
    pub audio: Vec<AudioMetadata>,
    pub duration: Duration,
    pub timescale: u32,
    pub major_brand: String,
    pub compatible_brands: Vec<String>,
}

pub fn parse<P: AsRef<Path>>(file_path: P) -> Result<MediaMetadata> {
    let mut video_metadata = None;
    let mut audio_metadata = vec![];
    let mut total_duration = Duration::ZERO;

    let file = File::open(file_path.as_ref())?;
    let file_size = file.metadata()?.len();
    let mp4_reader = mp4::Mp4Reader::read_header(BufReader::new(file), file_size)?;

    log::debug!(
        "Parsed MP4 header from `{}`. {file_size} bytes.  major brand: {}. Found {} tracks",
        file_path.as_ref().display(),
        mp4_reader.ftyp.major_brand,
        mp4_reader.tracks().len(),
    );

    for (track_id, track) in mp4_reader.tracks() {
        let track_type = match track.track_type() {
            Ok(t) => t,
            Err(e) => {
                log::warn!("Could not determine track type for track {track_id}: {e}, skipping");
                continue;
            }
        };

        log::debug!("Track {}: {:?}", track_id, track_type);

        match track_type {
            mp4::TrackType::Video => {
                if video_metadata.is_some() {
                    return Err(MP4PlayerError::ParseError(mp4::Error::InvalidData(
                        "Found two video tracks",
                    )));
                }

                video_metadata = Some(VideoMetadata {
                    track_id: *track_id,
                    width: track.width() as u32,
                    height: track.height() as u32,
                    frame_rate: track.frame_rate(),
                    bitrate: track.bitrate(),
                    timescale: track.timescale(),
                    duration: track.duration(),
                    sample_count: track.sample_count(),
                    box_type: track.box_type().unwrap_or_default().to_string(),
                    video_profile: track
                        .video_profile()
                        .unwrap_or(AvcProfile::AvcBaseline)
                        .to_string(),
                    pixel_format: SupportedPixelFormat::YUV420P,
                    codec: match track.media_type() {
                        Ok(mp4::MediaType::H264) => SupportedCodec::H264,
                        _ => SupportedCodec::Unsupported,
                    },
                });
            }
            mp4::TrackType::Audio => {
                audio_metadata.push(AudioMetadata {
                    track_id: *track_id,
                    duration: track.duration(),
                    sample_count: track.sample_count(),
                    bitrate: track.bitrate(),
                    timescale: track.timescale(),
                    box_type: track.box_type().unwrap_or_default().to_string(),
                    audio_profile: track
                        .audio_profile()
                        .unwrap_or(AudioObjectType::AacMain)
                        .to_string(),
                    sample_rate: match track.sample_freq_index() {
                        Ok(freq_index) => freq_index.freq(),
                        Err(_) => track.timescale(),
                    },
                    codec: match track.media_type() {
                        Ok(mp4::MediaType::AAC) => SupportedCodec::ACC,
                        _ => SupportedCodec::Unsupported,
                    },
                    channels: match track.channel_config() {
                        Ok(chan_config) => match chan_config {
                            mp4::ChannelConfig::Mono => 1,
                            mp4::ChannelConfig::Stereo => 2,
                            mp4::ChannelConfig::Three => 3,
                            mp4::ChannelConfig::Four => 4,
                            mp4::ChannelConfig::Five => 5,
                            mp4::ChannelConfig::FiveOne => 6,
                            mp4::ChannelConfig::SevenOne => 8,
                        },
                        Err(_) => 2, // fallback to stereo
                    },
                });
            }
            _ => {
                log::info!("Found track #{} of type {:?}", track_id, track_type);
            }
        }

        if track.duration() > total_duration {
            total_duration = track.duration();
        }
    }

    Ok(MediaMetadata {
        video: video_metadata,
        audio: audio_metadata,
        duration: total_duration,
        timescale: mp4_reader.timescale(),
        major_brand: mp4_reader.major_brand().to_string(),
        compatible_brands: mp4_reader
            .compatible_brands()
            .iter()
            .map(|brand| brand.to_string())
            .collect::<Vec<_>>(),
    })
}
