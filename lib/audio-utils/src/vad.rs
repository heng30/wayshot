/// Simple Voice Activity Detection (VAD) based on energy
/// Detects speech segments and splits audio into sentences

#[derive(Debug, Clone)]
pub struct AudioSegment {
    pub start_sample: usize,
    pub end_sample: usize,
    pub audio_data: Vec<f32>,
}

#[derive(Debug, Clone)]
pub struct VadConfig {
    /// Sample rate of audio
    pub sample_rate: u32,
    /// Minimum speech duration in milliseconds
    pub min_speech_duration_ms: u32,
    /// Minimum silence duration in milliseconds to split segments
    pub min_silence_duration_ms: u32,
    /// Energy threshold for speech detection (0.0 - 1.0)
    /// Lower = more sensitive, Higher = less sensitive
    pub speech_threshold: f32,
    /// Window size in milliseconds for energy calculation
    pub window_size_ms: u32,
}

impl Default for VadConfig {
    fn default() -> Self {
        Self {
            sample_rate: 16000,
            min_speech_duration_ms: 250,
            min_silence_duration_ms: 300,
            speech_threshold: 0.01,
            window_size_ms: 30,
        }
    }
}

/// Detect speech segments in audio using energy-based VAD
pub fn detect_speech_segments(audio_data: &[f32], config: &VadConfig) -> Vec<AudioSegment> {
    if audio_data.is_empty() {
        return Vec::new();
    }

    let window_size = (config.sample_rate as usize * config.window_size_ms as usize) / 1000;
    let hop_size = window_size / 2; // 50% overlap
    let min_speech_samples = (config.sample_rate as usize * config.min_speech_duration_ms as usize) / 1000;
    let min_silence_samples = (config.sample_rate as usize * config.min_silence_duration_ms as usize) / 1000;

    // Calculate energy for each window
    let mut energies = Vec::new();

    for i in (0..audio_data.len().saturating_sub(window_size)).step_by(hop_size) {
        let window_energy: f32 = audio_data[i..i + window_size]
            .iter()
            .map(|&x| x * x)
            .sum::<f32>() / window_size as f32;

        energies.push((i, window_energy));
    }

    if energies.is_empty() {
        return Vec::new();
    }

    // Find max energy for normalization
    let max_energy = energies.iter().map(|&(_, e)| e).fold(0.0f32, |a, b| a.max(b));

    if max_energy < 1e-6 {
        // Audio is too quiet
        return Vec::new();
    }

    // Detect speech based on threshold
    let mut in_speech = false;
    let mut speech_start = 0;
    let mut silence_start = 0;
    let mut segments = Vec::new();

    for &(window_pos, energy) in &energies {
        let normalized_energy = energy / (max_energy + 1e-6);
        let is_speech = normalized_energy > config.speech_threshold;

        if is_speech && !in_speech {
            // Start of speech segment
            in_speech = true;
            speech_start = window_pos;
        } else if !is_speech && in_speech {
            // Potential end of speech segment (silence detected)
            if silence_start == 0 {
                silence_start = window_pos;
            }

            let silence_duration = window_pos - silence_start;
            if silence_duration >= min_silence_samples {
                // End of speech segment
                let speech_duration = window_pos - speech_start;

                if speech_duration >= min_speech_samples {
                    // Valid speech segment
                    let end_sample = window_pos + window_size;
                    let segment_audio = audio_data[speech_start..end_sample.min(audio_data.len())].to_vec();

                    segments.push(AudioSegment {
                        start_sample: speech_start,
                        end_sample: end_sample.min(audio_data.len()),
                        audio_data: segment_audio,
                    });
                }

                in_speech = false;
                silence_start = 0;
            }
        } else if is_speech && in_speech {
            // Still in speech, reset silence timer
            silence_start = 0;
        }
    }

    // Handle last segment if it ends with speech
    if in_speech {
        let speech_duration = audio_data.len() - speech_start;

        if speech_duration >= min_speech_samples {
            let segment_audio = audio_data[speech_start..].to_vec();

            segments.push(AudioSegment {
                start_sample: speech_start,
                end_sample: audio_data.len(),
                audio_data: segment_audio,
            });
        }
    }

    // Merge very close segments (less than min_silence_duration_ms apart)
    if segments.len() > 1 {
        let mut merged_segments = Vec::new();
        let mut current_segment = segments[0].clone();

        for segment in segments.iter().skip(1) {
            let gap = segment.start_sample - current_segment.end_sample;
            let gap_ms = (gap * 1000) / config.sample_rate as usize;

            if gap_ms < config.min_silence_duration_ms as usize {
                // Merge segments
                current_segment.end_sample = segment.end_sample;
                current_segment.audio_data.extend(&segment.audio_data);
            } else {
                merged_segments.push(current_segment.clone());
                current_segment = segment.clone();
            }
        }
        merged_segments.push(current_segment);
        merged_segments
    } else {
        segments
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_speech_segments() {
        let sample_rate = 16000;
        let config = VadConfig {
            sample_rate,
            min_speech_duration_ms: 100,
            min_silence_duration_ms: 100,
            speech_threshold: 0.01,
            window_size_ms: 30,
        };

        // Create test audio: speech - silence - speech
        let mut audio = Vec::new();

        // Speech segment 1 (0.5 seconds)
        for _ in 0..(sample_rate / 2) {
            audio.push(0.1); // Some energy
        }

        // Silence (0.3 seconds)
        for _ in 0..(sample_rate * 3 / 10) {
            audio.push(0.001); // Very low energy
        }

        // Speech segment 2 (0.5 seconds)
        for _ in 0..(sample_rate / 2) {
            audio.push(0.1);
        }

        let segments = detect_speech_segments(&audio, &config);

        // Should detect 2 segments
        assert_eq!(segments.len(), 2);
    }
}
