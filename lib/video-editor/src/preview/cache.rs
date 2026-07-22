use crate::{
    Error, Result, ensure_file_exists, metadata::AudioMetadata,
    tracks::audio_track::extract_samples_from_frame,
};
use audio_utils::audio::{resample_audio_to_target_samples, resample_audio_with_channel};
use ffmpeg_next as ffmpeg;
use lru::LruCache;
use std::{
    collections::{HashSet, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
    num::NonZeroUsize,
    path::Path,
    sync::{Arc, Condvar, Mutex, OnceLock},
    time::Duration,
};

const DEFAULT_MAX_FILES: usize = 100;
pub const DEFAULT_CACHE_SAMPLE_RATE: u32 = 60; // 60Hz for UI display
static GLOBAL_AUDIO_DISPLAY_CACHE: OnceLock<GlobalAudioDisplayCache> = OnceLock::new();

pub fn get_global_audio_display_cache() -> &'static GlobalAudioDisplayCache {
    GLOBAL_AUDIO_DISPLAY_CACHE
        .get_or_init(|| GlobalAudioDisplayCache::new(DEFAULT_MAX_FILES, DEFAULT_CACHE_SAMPLE_RATE))
}

pub fn clear_global_audio_display_cache() {
    get_global_audio_display_cache().clear();
}

// Cache key for audio data - based on file path and stream index
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct AudioCacheKey {
    path_hash: u64,
    stream_index: usize,
}

impl AudioCacheKey {
    pub fn from_path(path: &Path, stream_index: usize) -> Self {
        let mut hasher = DefaultHasher::new();
        path.hash(&mut hasher);

        Self {
            path_hash: hasher.finish(),
            stream_index,
        }
    }
}

// Cached audio data for UI display
#[derive(Debug, Clone)]
pub struct AudioDisplayCacheData {
    // Resampled audio samples at cache_sample_rate (interleaved channels)
    pub samples: Vec<f32>,

    // Cache sample rate (default 60Hz)
    pub cache_sample_rate: u32,

    // Original channel count
    pub channels: u16,

    // Original sample rate
    pub original_sample_rate: u32,

    // Duration of the audio
    pub duration: Duration,
}

impl AudioDisplayCacheData {
    // Extract a segment from the cached audio data.
    // Returns (channels, resampled_samples) for the requested segment
    pub fn extract_segment(
        &self,
        source_offset: Duration,
        segment_duration: Duration,
        samples_per_channel: u32,
    ) -> (u16, Vec<f32>) {
        if self.samples.is_empty() || segment_duration.is_zero() {
            return (self.channels, Vec::new());
        }

        let channels = self.channels as usize;
        let cache_rate = self.cache_sample_rate as f64;
        let start_time = source_offset.as_secs_f64();
        let end_time = (source_offset + segment_duration).as_secs_f64();

        // Ensure sample indices align with channel boundaries.
        // For stereo audio, data format is [L0, R0, L1, R1, ...].
        // start_sample and end_sample must be multiples of channels count
        // to avoid channel misalignment (e.g., starting from R0 instead of L0).
        let raw_start_sample = start_time * cache_rate * channels as f64;
        let start_sample = (raw_start_sample / channels as f64).floor() as usize * channels;

        let raw_end_sample = end_time * cache_rate * channels as f64;
        let end_sample =
            ((raw_end_sample / channels as f64).ceil() as usize * channels).min(self.samples.len());

        if start_sample >= self.samples.len() {
            return (self.channels, Vec::new());
        }

        let segment_samples: Vec<f32> = if start_sample < end_sample {
            self.samples[start_sample..end_sample].to_vec()
        } else {
            Vec::new()
        };

        let resampled =
            resample_audio_to_target_samples(&segment_samples, self.channels, samples_per_channel);

        (self.channels, resampled)
    }
}

// State for tracking loading keys with wait mechanism
struct LoadingState {
    loading_keys: HashSet<AudioCacheKey>,
}

// Global audio display cache manager
pub struct GlobalAudioDisplayCache {
    cache: Mutex<LruCache<AudioCacheKey, Arc<AudioDisplayCacheData>>>,
    loading: Mutex<LoadingState>,
    loading_condvar: Condvar,
    cache_sample_rate: u32,
}

impl GlobalAudioDisplayCache {
    pub fn new(max_files: usize, cache_sample_rate: u32) -> Self {
        Self {
            cache: Mutex::new(LruCache::new(
                NonZeroUsize::new(max_files)
                    .unwrap_or_else(|| NonZeroUsize::new(DEFAULT_MAX_FILES).unwrap()),
            )),
            loading: Mutex::new(LoadingState {
                loading_keys: HashSet::new(),
            }),
            loading_condvar: Condvar::new(),
            cache_sample_rate,
        }
    }

    pub fn get(&self, key: &AudioCacheKey) -> Option<Arc<AudioDisplayCacheData>> {
        self.cache.lock().unwrap().get(key).cloned()
    }

    pub fn get_by_path(
        &self,
        path: &Path,
        stream_index: usize,
    ) -> Option<Arc<AudioDisplayCacheData>> {
        let key = AudioCacheKey::from_path(path, stream_index);
        self.get(&key)
    }

    pub fn put(&self, key: AudioCacheKey, data: Arc<AudioDisplayCacheData>) {
        self.cache.lock().unwrap().put(key, data);
    }

    pub fn contains(&self, key: &AudioCacheKey) -> bool {
        self.cache.lock().unwrap().contains(key)
    }

    pub fn is_loading(&self, key: &AudioCacheKey) -> bool {
        self.loading.lock().unwrap().loading_keys.contains(key)
    }

    pub fn is_loading_path(&self, path: &Path, stream_index: usize) -> bool {
        let key = AudioCacheKey::from_path(path, stream_index);
        self.is_loading(&key)
    }

    pub fn remove(&self, key: &AudioCacheKey) -> Option<Arc<AudioDisplayCacheData>> {
        self.cache.lock().unwrap().pop(key)
    }

    pub fn remove_by_path(
        &self,
        path: &Path,
        stream_index: usize,
    ) -> Option<Arc<AudioDisplayCacheData>> {
        let key = AudioCacheKey::from_path(path, stream_index);
        self.remove(&key)
    }

    pub fn clear(&self) {
        self.cache.lock().unwrap().clear();
        self.loading.lock().unwrap().loading_keys.clear();
        self.loading_condvar.notify_all();
    }

    // Load and cache audio data from file (blocking)
    // This method handles concurrent loading - if another thread is loading the same key,
    // it will wait for that thread to complete and then return the cached result.
    pub fn load_and_cache(
        &self,
        path: &Path,
        stream_index: usize,
        audio_meta: &AudioMetadata,
    ) -> Result<Arc<AudioDisplayCacheData>> {
        let key = AudioCacheKey::from_path(path, stream_index);

        if let Some(data) = self.get(&key) {
            return Ok(data);
        }

        // Check if another thread is loading this key
        {
            let mut loading = self.loading.lock().unwrap();

            // Double-check cache while holding loading lock
            if let Some(data) = self.cache.lock().unwrap().get(&key) {
                return Ok(data.clone());
            }

            // If another thread is loading, wait for it
            while loading.loading_keys.contains(&key) {
                loading = self.loading_condvar.wait(loading).unwrap();

                // After being notified, check cache again
                if let Some(data) = self.cache.lock().unwrap().get(&key) {
                    return Ok(data.clone());
                }
            }

            loading.loading_keys.insert(key.clone());
        }

        // Load the audio data (without holding any locks)
        let result =
            extract_full_audio_for_display(path, stream_index, audio_meta, self.cache_sample_rate);

        if let Ok(data) = result {
            let cached = Arc::new(data);
            self.cache.lock().unwrap().put(key.clone(), cached.clone());

            self.loading.lock().unwrap().loading_keys.remove(&key);
            self.loading_condvar.notify_all();

            return Ok(cached);
        }

        self.loading.lock().unwrap().loading_keys.remove(&key);
        self.loading_condvar.notify_all();

        Err(result.unwrap_err())
    }
}

// Extract full audio from file and resample to low sample rate for display
fn extract_full_audio_for_display(
    path: &Path,
    stream_index: usize,
    audio_meta: &AudioMetadata,
    cache_sample_rate: u32,
) -> Result<AudioDisplayCacheData> {
    ensure_file_exists!(path);

    ffmpeg::init().map_err(|e| Error::FFmpeg(format!("Failed to initialize FFmpeg: {}", e)))?;

    let mut input_ctx = ffmpeg::format::input(path)
        .map_err(|e| Error::FFmpeg(format!("Failed to open input file: {}", e)))?;

    // Find the specified audio stream
    let stream = input_ctx
        .streams()
        .find(|s| s.index() == stream_index)
        .ok_or_else(|| Error::FFmpeg(format!("Audio stream {} not found", stream_index)))?;

    let codec_par = stream.parameters();
    let mut decoder = ffmpeg::codec::context::Context::from_parameters(codec_par.clone())
        .map_err(|e| Error::FFmpeg(format!("Failed to create decoder context: {}", e)))?
        .decoder()
        .audio()
        .map_err(|e| Error::FFmpeg(format!("Failed to get audio decoder: {}", e)))?;

    let source_format = decoder.format();
    let channels = audio_meta.channels;
    let original_sample_rate = audio_meta.sample_rate;
    let duration = audio_meta.duration;

    let mut decoded_data = Vec::new();
    for (stream, packet) in input_ctx.packets() {
        if stream.index() != stream_index {
            continue;
        }

        if let Err(e) = decoder.send_packet(&packet) {
            log::trace!("Error sending packet to decoder: {:?}", e);
            continue;
        }

        let mut decoded_frame = ffmpeg::frame::Audio::empty();
        loop {
            match decoder.receive_frame(&mut decoded_frame) {
                Ok(_) => {
                    if decoded_frame.samples() > 0 {
                        extract_samples_from_frame(
                            &decoded_frame,
                            source_format,
                            channels,
                            &mut decoded_data,
                        )?;
                    }
                }
                Err(ffmpeg::Error::Other { .. }) | Err(ffmpeg::Error::Eof) => break,
                Err(e) => {
                    log::trace!("Error receiving frame: {:?}", e);
                    break;
                }
            }
        }
    }

    // Flush decoder
    _ = decoder.send_eof();
    let mut decoded_frame = ffmpeg::frame::Audio::empty();
    while decoder.receive_frame(&mut decoded_frame).is_ok() {
        if decoded_frame.samples() > 0 {
            extract_samples_from_frame(&decoded_frame, source_format, channels, &mut decoded_data)?;
        }
    }

    // Resample to low sample rate for display
    let resampled_samples = if !decoded_data.is_empty() && cache_sample_rate > 0 {
        // Calculate target sample count at cache_sample_rate
        let duration_secs = duration.as_secs_f64();
        let target_samples = (duration_secs * cache_sample_rate as f64 * channels as f64) as usize;

        if target_samples > 0 {
            resample_audio_with_channel(
                &decoded_data,
                original_sample_rate,
                channels,
                cache_sample_rate,
                channels,
            )
            .map_err(|e| Error::FFmpeg(format!("Resample error: {:?}", e)))?
        } else {
            decoded_data
        }
    } else {
        decoded_data
    };

    log::debug!(
        "Audio display cache: {} samples ({}Hz, {}ch, {:?}) for {}",
        resampled_samples.len(),
        cache_sample_rate,
        channels,
        duration,
        path.display()
    );

    Ok(AudioDisplayCacheData {
        samples: resampled_samples,
        cache_sample_rate,
        channels,
        original_sample_rate,
        duration,
    })
}
