use crate::{AudioProcessError, Result};
use derivative::Derivative;
use derive_setters::Setters;
use std::{fs::File, path::Path, time::Duration};
use symphonia::{
    core::{
        audio::{AudioBuffer, AudioBufferRef, Signal},
        codecs::DecoderOptions,
        errors::Error as SymphoniaError,
        formats::FormatOptions,
        io::MediaSourceStream,
        meta::MetadataOptions,
        probe::Hint,
        sample::Sample,
    },
    default,
};

#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct AudioConfig {
    #[derivative(Default(value = "16_000"))]
    pub sample_rate: u32,

    #[derivative(Default(value = "1"))]
    pub channel: u16,

    pub duration: Duration,
    pub samples: Vec<f32>,
}

#[derive(Debug, Clone)]
pub struct AudioSegment {
    pub index: u32,
    pub start_timestamp: Duration,
    pub end_timestamp: Duration,
    pub samples: Vec<f32>,
}

fn convert_planar<S, F>(buf: &AudioBuffer<S>, mut convert_fn: F) -> Vec<f32>
where
    S: Sample + Copy,
    F: FnMut(S) -> f32,
{
    let spec = *buf.spec();
    let channels = spec.channels.count();
    let frames = buf.frames();
    let mut samples = Vec::with_capacity(frames * channels);

    for frame in 0..frames {
        for channel in 0..channels {
            samples.push(convert_fn(buf.chan(channel)[frame]));
        }
    }
    samples
}

fn convert_audio_buffer_to_f32(audio_buffer: AudioBufferRef) -> Vec<f32> {
    match audio_buffer {
        AudioBufferRef::S8(buf) => convert_planar(&buf, |s| s as f32 / i8::MAX as f32),
        AudioBufferRef::U8(buf) => convert_planar(&buf, |s| {
            let half = (u8::MAX / 2 + 1) as f32;
            (s as f32 - half) / half
        }),
        AudioBufferRef::S16(buf) => convert_planar(&buf, |s| s as f32 / i16::MAX as f32),
        AudioBufferRef::U16(buf) => convert_planar(&buf, |s| {
            let half = (u16::MAX / 2 + 1) as f32;
            (s as f32 - half) / half
        }),
        AudioBufferRef::S24(buf) => {
            convert_planar(&buf, |s| s.inner() as f32 / (i32::MAX >> 8) as f32)
        }
        AudioBufferRef::U24(buf) => convert_planar(&buf, |s| {
            let half = ((1u32 << 24) / 2 + 1) as f32;
            (s.inner() as f32 - half) / half
        }),
        AudioBufferRef::S32(buf) => convert_planar(&buf, |s| s as f32 / i32::MAX as f32),
        AudioBufferRef::U32(buf) => convert_planar(&buf, |s| {
            let half = (u32::MAX / 2 + 1) as f32;
            (s as f32 - half) / half
        }),
        AudioBufferRef::F32(buf) => convert_planar(&buf, |s| s),
        AudioBufferRef::F64(buf) => convert_planar(&buf, |s| s as f32),
    }
}

pub fn load_audio_file(path: impl AsRef<Path>) -> Result<AudioConfig> {
    let file = File::open(&path)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(extension) = path.as_ref().extension()
        && let Some(ext_str) = extension.to_str()
    {
        hint.with_extension(&ext_str.to_lowercase());
    }

    let meta_opts: MetadataOptions = Default::default();
    let fmt_opts: FormatOptions = Default::default();
    let probed = default::get_probe()
        .format(&hint, mss, &fmt_opts, &meta_opts)
        .map_err(|e| AudioProcessError::Audio(format!("Failed to probe format: {e}")))?;

    let mut format = probed.format;

    // Find the first audio track by checking for sample_rate (audio-specific property)
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.sample_rate.is_some())
        .ok_or_else(|| AudioProcessError::Audio("No audio track found".to_string()))?;

    let codec_params = &track.codec_params;
    let mut decoder = default::get_codecs()
        .make(codec_params, &DecoderOptions::default())
        .map_err(|e| AudioProcessError::Audio(format!("Failed to create decoder: {e}")))?;

    let track_id = track.id;
    let mut all_samples = Vec::new();

    // Decode first packet to get audio format info
    let (sample_rate, channel_count) = loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(SymphoniaError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                return Err(AudioProcessError::Audio(
                    "No audio packets found".to_string(),
                ));
            }
            Err(SymphoniaError::ResetRequired) => continue,
            Err(e) => {
                return Err(AudioProcessError::Audio(format!(
                    "Failed to get packet: {e}"
                )));
            }
        };

        if packet.track_id() != track_id {
            continue;
        }

        match decoder.decode(&packet) {
            Ok(audio_buffer) => {
                let spec = *audio_buffer.spec();
                let samples = convert_audio_buffer_to_f32(audio_buffer);
                all_samples.extend_from_slice(&samples);
                break (spec.rate, spec.channels.count());
            }
            Err(SymphoniaError::IoError(_)) | Err(SymphoniaError::DecodeError(_)) => continue,
            Err(e) => {
                return Err(AudioProcessError::Audio(format!(
                    "Failed to decode audio: {e}"
                )));
            }
        }
    };

    log::info!("Detected audio format: {sample_rate} Hz, {channel_count} channels");

    // Continue decoding the rest of the packets
    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(SymphoniaError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                break;
            }
            Err(SymphoniaError::ResetRequired) => continue,
            Err(e) => {
                return Err(AudioProcessError::Audio(format!(
                    "Failed to get packet: {e}"
                )));
            }
        };

        if packet.track_id() != track_id {
            continue;
        }

        match decoder.decode(&packet) {
            Ok(audio_buffer) => {
                let samples = convert_audio_buffer_to_f32(audio_buffer);
                all_samples.extend_from_slice(&samples);
            }
            Err(SymphoniaError::IoError(_)) | Err(SymphoniaError::DecodeError(_)) => continue,
            Err(e) => {
                return Err(AudioProcessError::Audio(format!(
                    "Failed to decode audio: {e}"
                )));
            }
        }
    }

    let sample_count = all_samples.len() / channel_count as usize;
    let duration = std::time::Duration::from_secs_f64(sample_count as f64 / sample_rate as f64);

    log::info!(
        "Loaded audio file: {} - {} Hz, {} channels, {} samples, duration: {:.2}s",
        path.as_ref().display(),
        sample_rate,
        channel_count,
        sample_count,
        duration.as_secs_f64()
    );

    Ok(AudioConfig {
        sample_rate,
        channel: channel_count as u16,
        duration,
        samples: all_samples,
    })
}

pub fn load_audio_file_and_convert(
    path: impl AsRef<Path>,
    target_channel: u16,
    target_sample_rate: u32,
) -> Result<AudioConfig> {
    let mut audio_config = load_audio_file(path)?;

    if audio_config.sample_rate != target_sample_rate || audio_config.channel != target_channel {
        let samples = crate::audio::resample_audio_with_channel(
            &audio_config.samples,
            audio_config.sample_rate,
            audio_config.channel,
            target_sample_rate,
            target_channel,
        )?;

        log::info!(
            "Audio format: {} Hz, {} channels -> target: {target_sample_rate} Hz, {target_channel} channels",
            audio_config.sample_rate,
            audio_config.channel
        );

        audio_config.sample_rate = target_sample_rate;
        audio_config.channel = target_channel;
        audio_config.samples = samples;
    }

    Ok(audio_config)
}

pub fn gen_audio_segments(config: &AudioConfig, segments: &mut [AudioSegment]) {
    let sample_rate = config.sample_rate as f64;
    let channels = config.channel as usize;

    for segment in segments.iter_mut() {
        let start_sample =
            (segment.start_timestamp.as_secs_f64() * sample_rate * channels as f64) as usize;
        let end_sample =
            (segment.end_timestamp.as_secs_f64() * sample_rate * channels as f64) as usize;

        let start = start_sample.min(config.samples.len());
        let end = end_sample.min(config.samples.len());

        if start < end {
            segment.samples = config.samples[start..end].to_vec();
        } else {
            log::warn!(
                "Invalid segment[{}] range: start={:?} end={:?}, skipping",
                segment.index,
                segment.start_timestamp,
                segment.end_timestamp,
            );
            segment.samples.clear();
        }
    }
}
