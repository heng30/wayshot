#![allow(dead_code)]

use std::path::Path;

use symphonia::core::audio::{Audio, GenericAudioBufferRef};
use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::formats::probe::Hint;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;

use crate::{Error, Result};

/// Write a 32-bit float mono waveform to a 16-bit PCM WAV file.
pub fn write_wav(path: impl AsRef<Path>, samples: &[f32], sample_rate: u32) -> Result<()> {
    let file = std::fs::File::create(path)?;
    write_wav_to(std::io::BufWriter::new(file), samples, sample_rate)
}

/// Write a 32-bit float mono waveform as 16-bit PCM WAV to any
/// `Write + Seek` sink (e.g. `Cursor<Vec<u8>>`, `BufWriter<File>`,
/// or a memory-mapped buffer). Mirrors [`write_wav`] but lets callers
/// stream the encoded bytes anywhere — useful for HTTP responses,
/// channels, sockets, or in-memory pipelines that don't want to touch
/// the filesystem.
pub fn write_wav_to<W: std::io::Write + std::io::Seek>(
    writer: W,
    samples: &[f32],
    sample_rate: u32,
) -> Result<()> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::new(writer, spec)?;
    for &s in samples {
        let clamped = s.clamp(-1.0, 1.0);
        writer.write_sample((clamped * i16::MAX as f32) as i16)?;
    }
    writer.finalize()?;
    Ok(())
}

/// Encode a 32-bit float mono waveform as a 16-bit PCM WAV byte buffer.
/// Convenience wrapper over [`write_wav_to`] for the common
/// "give me the bytes" case (HTTP responses, channels, etc.).
pub fn encode_wav(samples: &[f32], sample_rate: u32) -> Result<Vec<u8>> {
    let mut buf = std::io::Cursor::new(Vec::<u8>::new());
    write_wav_to(&mut buf, samples, sample_rate)?;
    Ok(buf.into_inner())
}

/// Decode an audio file (WAV/FLAC/MP3/etc. — anything Symphonia supports) to
/// a mono `f32` PCM waveform, returning `(samples, sample_rate)`.
pub fn load_audio(path: impl AsRef<Path>) -> Result<(Vec<f32>, u32)> {
    let file = std::fs::File::open(path.as_ref())?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.as_ref().extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let mut format = symphonia::default::get_probe()
        .probe(&hint, mss, FormatOptions::default(), MetadataOptions::default())
        .map_err(|e| Error::AudioDecode(e.to_string()))?;
    let track = format
        .default_track(TrackType::Audio)
        .ok_or_else(|| Error::AudioDecode("no default track".into()))?;
    let codec_params = track
        .codec_params
        .as_ref()
        .and_then(|p| p.audio())
        .ok_or_else(|| Error::AudioDecode("no audio codec params".into()))?;
    let track_id = track.id;
    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(codec_params, &AudioDecoderOptions::default())
        .map_err(|e| Error::AudioDecode(e.to_string()))?;
    let sr = codec_params
        .sample_rate
        .ok_or_else(|| Error::AudioDecode("unknown sample rate".into()))?;

    let mut pcm: Vec<f32> = Vec::new();
    loop {
        let packet = match format.next_packet() {
            Ok(Some(p)) => p,
            Ok(None) => break,
            Err(symphonia::core::errors::Error::IoError(e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(symphonia::core::errors::Error::ResetRequired) => break,
            Err(e) => return Err(Error::AudioDecode(e.to_string())),
        };
        if packet.track_id != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(symphonia::core::errors::Error::DecodeError(_)) => continue,
            Err(e) => return Err(Error::AudioDecode(e.to_string())),
        };
        append_mono_f32(&decoded, &mut pcm);
    }
    Ok((pcm, sr))
}

fn append_mono_f32(buf: &GenericAudioBufferRef<'_>, out: &mut Vec<f32>) {
    match buf {
        GenericAudioBufferRef::F32(b) => mix_to_mono(b.spec().channels().count(), b.frames(), out, |ch, i| b.plane(ch).unwrap()[i]),
        GenericAudioBufferRef::F64(b) => mix_to_mono(b.spec().channels().count(), b.frames(), out, |ch, i| b.plane(ch).unwrap()[i] as f32),
        GenericAudioBufferRef::S16(b) => mix_to_mono(b.spec().channels().count(), b.frames(), out, |ch, i| {
            b.plane(ch).unwrap()[i] as f32 / i16::MAX as f32
        }),
        GenericAudioBufferRef::S32(b) => mix_to_mono(b.spec().channels().count(), b.frames(), out, |ch, i| {
            b.plane(ch).unwrap()[i] as f32 / i32::MAX as f32
        }),
        GenericAudioBufferRef::U8(b) => mix_to_mono(b.spec().channels().count(), b.frames(), out, |ch, i| {
            (b.plane(ch).unwrap()[i] as f32 - 128.0) / 128.0
        }),
        GenericAudioBufferRef::S8(b) => mix_to_mono(b.spec().channels().count(), b.frames(), out, |ch, i| {
            b.plane(ch).unwrap()[i] as f32 / i8::MAX as f32
        }),
        GenericAudioBufferRef::U16(b) => mix_to_mono(b.spec().channels().count(), b.frames(), out, |ch, i| {
            (b.plane(ch).unwrap()[i] as f32 - 32768.0) / 32768.0
        }),
        GenericAudioBufferRef::U24(b) => mix_to_mono(b.spec().channels().count(), b.frames(), out, |ch, i| {
            (b.plane(ch).unwrap()[i].inner() as f32 - 8_388_608.0) / 8_388_608.0
        }),
        GenericAudioBufferRef::S24(b) => mix_to_mono(b.spec().channels().count(), b.frames(), out, |ch, i| {
            b.plane(ch).unwrap()[i].inner() as f32 / 8_388_608.0
        }),
        GenericAudioBufferRef::U32(b) => mix_to_mono(b.spec().channels().count(), b.frames(), out, |ch, i| {
            (b.plane(ch).unwrap()[i] as f64 - 2_147_483_648.0) as f32 / 2_147_483_648.0
        }),
    }
}

fn mix_to_mono<F: Fn(usize, usize) -> f32>(n_ch: usize, n_frames: usize, out: &mut Vec<f32>, sample: F) {
    if n_ch == 1 {
        for i in 0..n_frames {
            out.push(sample(0, i));
        }
    } else {
        let inv = 1.0 / n_ch as f32;
        for i in 0..n_frames {
            let mut sum = 0.0f32;
            for c in 0..n_ch {
                sum += sample(c, i);
            }
            out.push(sum * inv);
        }
    }
}

/// Resample a mono f32 waveform from `from_sr` to `to_sr` using a
/// fixed-ratio sinc resampler. No-op if the rates are equal.
pub fn resample(input: &[f32], from_sr: u32, to_sr: u32) -> Result<Vec<f32>> {
    if from_sr == to_sr {
        return Ok(input.to_vec());
    }
    use rubato::{
        Async, FixedAsync, Resampler, SincInterpolationParameters, SincInterpolationType,
        WindowFunction, audioadapter_buffers::direct::SequentialSliceOfVecs,
    };

    let params = SincInterpolationParameters {
        sinc_len: 256,
        f_cutoff: Some(0.95),
        interpolation: SincInterpolationType::Linear,
        oversampling_factor: 256,
        window: WindowFunction::BlackmanHarris2,
    };
    let ratio = to_sr as f64 / from_sr as f64;
    let chunk_size = 1024usize;
    let mut resampler: Async<f32> = Async::new_sinc(
        ratio,
        2.0,
        &params,
        chunk_size,
        1,
        FixedAsync::Input,
    )
    .map_err(|e| Error::Resampler(e.to_string()))?;

    let out_len_needed = resampler.process_all_needed_output_len(input.len());
    let mut output_data = vec![vec![0.0f32; out_len_needed]; 1];
    let input_data = vec![input.to_vec()];
    let input_adapter = SequentialSliceOfVecs::new(&input_data, 1, input.len()).unwrap();
    let mut output_adapter =
        SequentialSliceOfVecs::new_mut(&mut output_data, 1, out_len_needed).unwrap();

    let (_nbr_in, nbr_out) = resampler
        .process_all_into_buffer(&input_adapter, &mut output_adapter, input.len(), None)
        .map_err(|e| Error::Resampler(e.to_string()))?;

    Ok(output_data[0][..nbr_out].to_vec())
}

/// Load `path`, downmix to mono, and resample to `target_sr`.
pub fn load_audio_as(path: impl AsRef<Path>, target_sr: u32) -> Result<Vec<f32>> {
    let (pcm, sr) = load_audio(path)?;
    resample(&pcm, sr, target_sr)
}

/// Decode an encoded audio byte buffer (WAV/FLAC/MP3/etc.) to mono `f32` PCM,
/// returning `(samples, sample_rate)`. Same format support as
/// [`load_audio`], just sourced from memory instead of a file.
pub fn load_audio_bytes(bytes: &[u8]) -> Result<(Vec<f32>, u32)> {
    let cursor = std::io::Cursor::new(bytes.to_vec());
    let mss = MediaSourceStream::new(Box::new(cursor), Default::default());
    let mut format = symphonia::default::get_probe()
        .probe(&Hint::new(), mss, FormatOptions::default(), MetadataOptions::default())
        .map_err(|e| Error::AudioDecode(e.to_string()))?;
    let track = format
        .default_track(TrackType::Audio)
        .ok_or_else(|| Error::AudioDecode("no default track".into()))?;
    let codec_params = track
        .codec_params
        .as_ref()
        .and_then(|p| p.audio())
        .ok_or_else(|| Error::AudioDecode("no audio codec params".into()))?;
    let track_id = track.id;
    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(codec_params, &AudioDecoderOptions::default())
        .map_err(|e| Error::AudioDecode(e.to_string()))?;
    let sr = codec_params
        .sample_rate
        .ok_or_else(|| Error::AudioDecode("unknown sample rate".into()))?;

    let mut pcm: Vec<f32> = Vec::new();
    loop {
        let packet = match format.next_packet() {
            Ok(Some(p)) => p,
            Ok(None) => break,
            Err(symphonia::core::errors::Error::IoError(e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(symphonia::core::errors::Error::ResetRequired) => break,
            Err(e) => return Err(Error::AudioDecode(e.to_string())),
        };
        if packet.track_id != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(symphonia::core::errors::Error::DecodeError(_)) => continue,
            Err(e) => return Err(Error::AudioDecode(e.to_string())),
        };
        append_mono_f32(&decoded, &mut pcm);
    }
    Ok((pcm, sr))
}

/// Decode an encoded audio byte buffer, downmix to mono, and resample to `target_sr`.
pub fn load_audio_bytes_as(bytes: &[u8], target_sr: u32) -> Result<Vec<f32>> {
    let (pcm, sr) = load_audio_bytes(bytes)?;
    resample(&pcm, sr, target_sr)
}