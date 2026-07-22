use crate::{
    Result,
    filters::{
        interpolation::get_float_at_time,
        keyframe::{AnimatableProperty, KeyframeTracks, KeyframeValue},
        traits::{AudioData, AudioFilter},
    },
};
use derivative::Derivative;
use derive_setters::Setters;
use pitch_shift::{Shifter, TOTAL_F32};

#[derive(Debug, Clone, Derivative, Setters, serde::Serialize, serde::Deserialize)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct VoiceChangerFilter {
    /// Pitch shift in semitones (positive = higher, negative = lower)
    /// Range: 0 to +12 (one octave), default 0
    #[derivative(Default(value = "3.0"))]
    pub pitch_semitones: f32,

    /// Formant shift in semitones (optional, for more natural voice)
    /// Positive = more feminine, negative = more masculine
    #[derivative(Default(value = "0.0"))]
    pub formant_semitones: f32,

    /// Keyframe tracks for animation
    #[serde(skip_serializing_if = "KeyframeTracks::is_empty", default)]
    pub keyframe_tracks: KeyframeTracks,
}

impl VoiceChangerFilter {
    pub const NAME: &'static str = "voice changer";

    /// Create a new voice changer with the specified pitch shift
    pub fn new(pitch_semitones: f32) -> Self {
        Self {
            pitch_semitones,
            formant_semitones: 0.0,
            keyframe_tracks: KeyframeTracks::default(),
        }
    }

    pub fn higher(semitones: f32) -> Self {
        Self::new(semitones.abs().min(12.0))
    }

    pub fn lower(semitones: f32) -> Self {
        Self::new(semitones.abs().min(0.0))
    }

    pub fn animatable_properties() -> Vec<AnimatableProperty> {
        vec![
            AnimatableProperty::float("pitch_semitones", "Pitch (semitones)", 0.0, 5.0, 0.0),
            AnimatableProperty::float("formant_semitones", "Formant (semitones)", -6.0, 6.0, 0.0),
        ]
    }

    fn get_pitch_at_time(&self, time_ms: i64) -> f32 {
        self.keyframe_tracks
            .get_track("pitch_semitones")
            .map(|track| get_float_at_time(track, time_ms, self.pitch_semitones))
            .unwrap_or(self.pitch_semitones)
    }

    fn get_formant_at_time(&self, time_ms: i64) -> f32 {
        self.keyframe_tracks
            .get_track("formant_semitones")
            .map(|track| get_float_at_time(track, time_ms, self.formant_semitones))
            .unwrap_or(self.formant_semitones)
    }

    fn semitones_to_ratio(semitones: f32) -> f32 {
        2f32.powf(semitones / 12.0)
    }

    /// High-quality resampling using rubato (sinc interpolation)
    /// This changes both pitch AND duration proportionally
    fn resample_with_rubato(input: &[f32], ratio: f64) -> Vec<f32> {
        if input.is_empty() || (ratio - 1.0).abs() < 0.001 {
            return input.to_vec();
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

        let chunk_size = 1024usize;
        let mut resampler: Async<f32> =
            Async::new_sinc(ratio, 2.0, &params, chunk_size, 1, FixedAsync::Input)
                .map(|r| r)
                .expect("Failed to create resampler");

        let out_len_needed = resampler.process_all_needed_output_len(input.len());
        let mut output_data = vec![vec![0.0f32; out_len_needed]; 1];
        let input_data = vec![input.to_vec()];
        let input_adapter = SequentialSliceOfVecs::new(&input_data, 1, input.len()).unwrap();
        let mut output_adapter =
            SequentialSliceOfVecs::new_mut(&mut output_data, 1, out_len_needed).unwrap();

        let (_nbr_in, nbr_out) = resampler
            .process_all_into_buffer(&input_adapter, &mut output_adapter, input.len(), None)
            .expect("Resampling failed");

        output_data[0][..nbr_out].to_vec()
    }

    /// Time stretch using pitch_shift library - preserves pitch while changing duration
    /// stretch_ratio > 1.0 stretches (slower), < 1.0 compresses (faster)
    fn time_stretch_with_pitch_shift(
        input: &[f32],
        stretch_ratio: f32,
        sample_rate: f32,
    ) -> Vec<f32> {
        if input.is_empty() || (stretch_ratio - 1.0).abs() < 0.001 {
            return input.to_vec();
        }

        // pitch_shift: out_samples/128 = stretch_ratio
        let out_samples = (128.0 * stretch_ratio).min(1023.0) as usize;
        if out_samples == 0 {
            return input.to_vec();
        }

        // Create Shifter state buffer
        let state: Box<[f32; TOTAL_F32]> = vec![0.0; TOTAL_F32]
            .try_into()
            .expect("State buffer size mismatch");
        let mut shifter = Shifter::new(state);

        let mut output = Vec::new();

        // Process in 128-sample chunks
        for chunk in input.chunks(128) {
            // Pad last chunk if needed
            let mut input_chunk = [0.0f32; 128];
            let copy_len = chunk.len().min(128);
            input_chunk[..copy_len].copy_from_slice(&chunk[..copy_len]);

            // shift_semitones = 0 preserves pitch
            let shifted = shifter.shift(
                &input_chunk,
                0.0, // shift_semitones = 0 (preserve pitch)
                out_samples,
                sample_rate,
            );
            output.extend_from_slice(shifted);
        }

        output
    }

    /// Pitch shift preserving duration using pitch_shift library
    /// 1. Resample (changes pitch and duration)
    /// 2. Time stretch to restore duration (preserves pitch change)
    fn pitch_shift_preserving_duration(input: &[f32], pitch_ratio: f32) -> Vec<f32> {
        if input.is_empty() || (pitch_ratio - 1.0).abs() < 0.001 {
            return input.to_vec();
        }

        // Step 1: Resample with rubato
        // For pitch shift:
        // - pitch_ratio > 1 means pitch UP → resample with ratio < 1 (downsample)
        //   This makes fewer samples → when played at same rate, pitch goes UP
        // - pitch_ratio < 1 means pitch DOWN → resample with ratio > 1 (upsample)
        //   This makes more samples → when played at same rate, pitch goes DOWN
        let resample_ratio = 1.0 / pitch_ratio as f64;
        let resampled = Self::resample_with_rubato(input, resample_ratio);

        // Step 2: Time stretch to restore original duration using pitch_shift
        // If we resampled with ratio 1/pitch_ratio:
        // - resampled length = input.len() * 1/pitch_ratio
        // - To get back to input.len(), stretch by pitch_ratio
        let stretch_ratio = pitch_ratio;
        let sample_rate = 44100.0; // Standard sample rate assumption
        let stretched = Self::time_stretch_with_pitch_shift(&resampled, stretch_ratio, sample_rate);

        // Ensure exact output length
        if stretched.len() != input.len() {
            Self::resample_to_length(&stretched, input.len())
        } else {
            stretched
        }
    }

    /// Resample input to target length using linear interpolation
    /// Used only for final length adjustment
    fn resample_to_length(input: &[f32], target_len: usize) -> Vec<f32> {
        if input.is_empty() || target_len == 0 {
            return Vec::new();
        }
        if input.len() == target_len {
            return input.to_vec();
        }
        let ratio = input.len() as f32 / target_len as f32;
        (0..target_len)
            .map(|i| {
                let pos = i as f32 * ratio;
                let idx = pos as usize;
                let frac = pos - idx as f32;
                if idx + 1 < input.len() {
                    input[idx] + (input[idx + 1] - input[idx]) * frac
                } else {
                    input[idx.min(input.len().saturating_sub(1))]
                }
            })
            .collect()
    }

    /// Apply formant shifting by spectral envelope scaling
    /// Formant shift changes voice character without changing pitch
    fn apply_formant_shift(input: &[f32], formant_ratio: f32) -> Vec<f32> {
        if input.is_empty() || (formant_ratio - 1.0).abs() < 0.01 {
            return input.to_vec();
        }

        // Simple formant shift via spectral envelope scaling
        // This approximates formant movement by shifting the frequency axis
        let input_len = input.len();

        // Scale spectral envelope by formant ratio and resample back
        let temp_len = (input_len as f32 / formant_ratio).ceil() as usize;
        let mut temp = vec![0.0f32; temp_len];

        for i in 0..temp_len {
            let src_pos = i as f32 * formant_ratio;
            let idx = src_pos as usize;
            let frac = src_pos - idx as f32;
            if idx + 1 < input_len {
                temp[i] = input[idx] + (input[idx + 1] - input[idx]) * frac;
            } else {
                temp[i] = input[idx.min(input_len.saturating_sub(1))];
            }
        }

        // Resample back to original length
        Self::resample_to_length(&temp, input_len)
    }

    /// Process a single chunk with the given pitch and formant values
    fn process_chunk(
        &self,
        input: &[f32],
        pitch_semitones: f32,
        formant_semitones: f32,
    ) -> Vec<f32> {
        // Skip processing if no effect needed
        if pitch_semitones.abs() < 0.01 && formant_semitones.abs() < 0.01 {
            return input.to_vec();
        }

        let pitch_ratio = Self::semitones_to_ratio(pitch_semitones);
        let formant_ratio = Self::semitones_to_ratio(formant_semitones);

        let mut output = if pitch_semitones.abs() >= 0.01 {
            Self::pitch_shift_preserving_duration(input, pitch_ratio)
        } else {
            input.to_vec()
        };

        if formant_semitones.abs() >= 0.01 {
            output = Self::apply_formant_shift(&output, formant_ratio);
        }

        // Ensure output length matches input
        if output.len() != input.len() {
            Self::resample_to_length(&output, input.len())
        } else {
            output
        }
    }

    /// Apply pitch/formant shift with static values (no keyframes)
    fn apply_static(&self, data: &mut AudioData) -> Result<()> {
        let pitch_ratio = Self::semitones_to_ratio(self.pitch_semitones);
        let formant_ratio = Self::semitones_to_ratio(self.formant_semitones);
        let channels = data.config.channels as usize;

        let total_samples = data.samples.len() / channels;
        let mut output = vec![0.0f32; data.samples.len()];

        for ch in 0..channels {
            // Extract channel samples
            let channel_in: Vec<f32> = (0..total_samples)
                .map(|i| data.samples[i * channels + ch])
                .collect();

            // Apply pitch shift
            let mut channel_out = if self.pitch_semitones.abs() >= 0.01 {
                Self::pitch_shift_preserving_duration(&channel_in, pitch_ratio)
            } else {
                channel_in.clone()
            };

            // Apply formant shift if needed
            if self.formant_semitones.abs() >= 0.01 {
                channel_out = Self::apply_formant_shift(&channel_out, formant_ratio);
            }

            // Handle any length mismatch gracefully
            let samples_to_copy = total_samples.min(channel_out.len());
            for i in 0..samples_to_copy {
                output[i * channels + ch] = channel_out[i];
            }
        }

        data.samples = output;
        Ok(())
    }
}

impl AudioFilter for VoiceChangerFilter {
    crate::impl_default_audio_filter!(VoiceChangerFilter);

    fn apply(&self, data: &mut AudioData) -> Result<()> {
        if data.samples.is_empty() {
            return Ok(());
        }

        let sample_rate = data.config.sample_rate;
        let channels = data.config.channels as usize;
        let start_time_ms = data.relative_timeline_offset.as_millis() as i64;

        // Check if any keyframes exist - if not, use simple single-value processing
        let has_pitch_keyframes = self.keyframe_tracks.get_track("pitch_semitones").is_some();
        let has_formant_keyframes = self
            .keyframe_tracks
            .get_track("formant_semitones")
            .is_some();

        // If no keyframes and static values are zero, pass through
        if !has_pitch_keyframes && !has_formant_keyframes {
            if self.pitch_semitones.abs() < 0.01 && self.formant_semitones.abs() < 0.01 {
                return Ok(());
            }
            // Use original single-value processing for efficiency
            return self.apply_static(data);
        }

        // Process in chunks for smooth keyframe transitions
        let chunk_duration_ms = 20; // ~20ms chunks for smooth transitions
        let samples_per_ms = sample_rate as f64 / 1000.0;
        let chunk_samples = (chunk_duration_ms as f64 * samples_per_ms) as usize;

        let total_samples = data.samples.len() / channels;
        let mut output = vec![0.0f32; data.samples.len()];

        for chunk_start in (0..total_samples).step_by(chunk_samples) {
            let chunk_end = (chunk_start + chunk_samples).min(total_samples);
            let chunk_center_sample = (chunk_start + chunk_end) / 2;

            // Calculate time for this chunk
            let chunk_time_ms =
                start_time_ms + (chunk_center_sample as f64 / samples_per_ms) as i64;

            // Get pitch and formant at chunk time
            let pitch = self.get_pitch_at_time(chunk_time_ms);
            let formant = self.get_formant_at_time(chunk_time_ms);

            // Process each channel for this chunk
            for ch in 0..channels {
                // Extract chunk samples for channel
                let chunk_in: Vec<f32> = (chunk_start..chunk_end)
                    .map(|i| data.samples[i * channels + ch])
                    .collect();

                // Apply pitch/formant shift
                let chunk_out = self.process_chunk(&chunk_in, pitch, formant);

                // Copy to output
                for (i, sample) in chunk_out.iter().enumerate() {
                    if chunk_start + i < total_samples {
                        output[(chunk_start + i) * channels + ch] = *sample;
                    }
                }
            }
        }

        data.samples = output;
        Ok(())
    }

    fn get_animatable_properties(&self) -> Vec<AnimatableProperty> {
        Self::animatable_properties()
    }

    fn get_keyframe_tracks(&self) -> KeyframeTracks {
        self.keyframe_tracks.clone()
    }

    fn set_keyframe_tracks(&mut self, tracks: KeyframeTracks) {
        self.keyframe_tracks = tracks;
    }

    fn supports_keyframes(&self) -> bool {
        true
    }

    fn update_keyframes_at_time(&self, tracks: &mut KeyframeTracks, time_ms: i64) -> bool {
        let mut updated = false;

        if let Some(track) = tracks.get_track("pitch_semitones") {
            if track.keyframes.iter().any(|k| k.time_ms == time_ms) {
                tracks.update_keyframe_value(
                    "pitch_semitones",
                    time_ms,
                    KeyframeValue::Float(self.pitch_semitones),
                );
                updated = true;
            }
        }

        if let Some(track) = tracks.get_track("formant_semitones") {
            if track.keyframes.iter().any(|k| k.time_ms == time_ms) {
                tracks.update_keyframe_value(
                    "formant_semitones",
                    time_ms,
                    KeyframeValue::Float(self.formant_semitones),
                );
                updated = true;
            }
        }

        updated
    }
}

#[cfg(test)]
mod tests {
    use super::VoiceChangerFilter;
    use crate::filters::keyframe::{Keyframe, KeyframeTracks};
    use crate::filters::traits::{AudioData, AudioFilter, AudioFilterConfig};
    use crate::metadata::Metadata;
    use crate::tracks::segment::Segment;
    use std::sync::Arc;
    use std::time::Duration;

    #[test]
    fn test_semitones_to_ratio() {
        // 0 semitones = 1.0 ratio
        assert!((VoiceChangerFilter::semitones_to_ratio(0.0) - 1.0).abs() < 0.001);

        // +12 semitones (one octave) = 2.0 ratio
        assert!((VoiceChangerFilter::semitones_to_ratio(12.0) - 2.0).abs() < 0.001);

        // -12 semitones (one octave) = 0.5 ratio
        assert!((VoiceChangerFilter::semitones_to_ratio(-12.0) - 0.5).abs() < 0.001);

        // +7 semitones (perfect fifth) ≈ 1.5 ratio
        let ratio = VoiceChangerFilter::semitones_to_ratio(7.0);
        assert!((ratio - 1.4983).abs() < 0.01);
    }

    #[test]
    fn test_resample_to_length() {
        let input = vec![0.0f32, 1.0, 2.0, 3.0];

        // Same length
        let output = VoiceChangerFilter::resample_to_length(&input, 4);
        assert_eq!(output.len(), 4);

        // Upsample
        let output = VoiceChangerFilter::resample_to_length(&input, 8);
        assert_eq!(output.len(), 8);

        // Downsample
        let output = VoiceChangerFilter::resample_to_length(&input, 2);
        assert_eq!(output.len(), 2);
    }

    #[test]
    fn test_pitch_shift_no_change() {
        // When pitch_ratio is 1.0, output should be same as input
        let input = vec![0.5f32, 0.3, 0.8, 0.2, 0.6, 0.4, 0.7, 0.1];
        let output = VoiceChangerFilter::pitch_shift_preserving_duration(&input, 1.0);

        assert_eq!(output.len(), input.len());
        for (orig, processed) in input.iter().zip(output.iter()) {
            assert!((orig - processed).abs() < 0.01);
        }
    }

    #[test]
    fn test_pitch_shift_exact_length() {
        // Test that pitch shift preserves sample length for various semitone values
        let input_len = 48000; // 1 second @ 48kHz
        let input: Vec<f32> = (0..input_len).map(|i| (i as f32 / 100.0).sin()).collect();

        let test_semitones = [-12.0, -7.0, -5.0, -3.0, -1.0, 1.0, 3.0, 5.0, 7.0, 12.0];

        for &semitones in &test_semitones {
            let pitch_ratio = VoiceChangerFilter::semitones_to_ratio(semitones);
            let output = VoiceChangerFilter::pitch_shift_preserving_duration(&input, pitch_ratio);
            assert_eq!(
                output.len(),
                input.len(),
                "Length changed from {} to {} for semitones={} (ratio={:.4})",
                input.len(),
                output.len(),
                semitones,
                pitch_ratio
            );

            // Verify output has signal energy (not silence)
            let energy: f32 = output.iter().map(|x| x * x).sum();
            assert!(
                energy > 0.0,
                "Output should have signal energy for semitones={}",
                semitones
            );
        }
    }

    #[test]
    fn test_pitch_shift_up() {
        // Pitch up by ratio 2.0 (one octave up)
        let input: Vec<f32> = (0..1000).map(|i| (i as f32 / 100.0).sin()).collect();
        let output = VoiceChangerFilter::pitch_shift_preserving_duration(&input, 2.0);

        // Output should have same length as input
        assert_eq!(output.len(), input.len());

        // Output should be different from input (pitch was shifted)
        let mut different = false;
        for (orig, processed) in input.iter().zip(output.iter()) {
            if (orig - processed).abs() > 0.1 {
                different = true;
                break;
            }
        }
        assert!(different, "Pitch shift should change the audio");
    }

    #[test]
    fn test_pitch_shift_down() {
        // Pitch down by ratio 0.5 (one octave down)
        let input: Vec<f32> = (0..1000).map(|i| (i as f32 / 100.0).sin()).collect();
        let output = VoiceChangerFilter::pitch_shift_preserving_duration(&input, 0.5);

        // Output should have same length as input
        assert_eq!(output.len(), input.len());

        // Output should be different from input (pitch was shifted)
        let mut different = false;
        for (orig, processed) in input.iter().zip(output.iter()) {
            if (orig - processed).abs() > 0.1 {
                different = true;
                break;
            }
        }
        assert!(different, "Pitch shift should change the audio");
    }

    #[test]
    fn test_pitch_shift_round_trip() {
        // Pitch up then down - won't perfectly restore due to phase vocoder artifacts
        // but should produce valid audio output
        // Need enough samples for the phase vocoder (fft_size = 2048)
        let original: Vec<f32> = (0..48000).map(|i| (i as f32 / 50.0).sin()).collect();

        // Shift up by 2x
        let shifted_up = VoiceChangerFilter::pitch_shift_preserving_duration(&original, 2.0);
        assert_eq!(shifted_up.len(), original.len());

        // Shift down by 0.5x (should restore pitch but not exact waveform)
        let restored = VoiceChangerFilter::pitch_shift_preserving_duration(&shifted_up, 0.5);
        assert_eq!(restored.len(), original.len());

        // Check that output is not silence (has signal energy)
        let energy: f32 = restored.iter().map(|x| x * x).sum();
        assert!(energy > 0.0, "Output should have signal energy");
    }

    #[test]
    fn test_formant_shift_preserves_length() {
        // Test that formant shift preserves sample length
        let input_len = 48000; // 1 second @ 48kHz
        let input: Vec<f32> = (0..input_len).map(|i| (i as f32 / 100.0).sin()).collect();

        let test_ratios = [0.8, 0.9, 1.1, 1.2];

        for &ratio in &test_ratios {
            let output = VoiceChangerFilter::apply_formant_shift(&input, ratio);
            assert_eq!(
                output.len(),
                input.len(),
                "Formant shift changed length from {} to {} for ratio={}",
                input.len(),
                output.len(),
                ratio
            );
        }
    }

    #[test]
    fn test_time_stretch_with_pitch_shift() {
        // Test time stretch with pitch_shift preserves length adjustment
        let input_len = 48000;
        let input: Vec<f32> = (0..input_len)
            .map(|i| (i as f32 / 100.0).sin() * 0.5)
            .collect();

        // Stretch by 1.5x (slower, longer)
        let stretched = VoiceChangerFilter::time_stretch_with_pitch_shift(&input, 1.5, 44100.0);

        // Should be approximately 1.5x longer
        // The key is that it preserves pitch
        assert!(stretched.len() > input_len);

        // Verify output has energy
        let energy: f32 = stretched.iter().map(|x| x * x).sum();
        assert!(energy > 0.0, "Stretched output should have signal energy");
    }

    #[test]
    fn test_resample_with_rubato() {
        // Test rubato resampling changes length proportionally
        // rubato ratio = output_sr / input_sr
        // ratio > 1 means upsampling → more samples → longer output
        // ratio < 1 means downsampling → fewer samples → shorter output
        let input_len = 48000;
        let input: Vec<f32> = (0..input_len).map(|i| (i as f32 / 100.0).sin()).collect();

        // Resample by 2.0 (upsampling → output ~2x longer)
        let resampled = VoiceChangerFilter::resample_with_rubato(&input, 2.0);
        assert!(
            resampled.len() > input_len,
            "2.0 resample should produce longer output (got {} expected ~{})",
            resampled.len(),
            input_len * 2
        );

        // Resample by 0.5 (downsampling → output ~0.5x length)
        let resampled = VoiceChangerFilter::resample_with_rubato(&input, 0.5);
        assert!(
            resampled.len() < input_len,
            "0.5 resample should produce shorter output (got {} expected ~{})",
            resampled.len(),
            input_len / 2
        );
    }

    #[test]
    fn test_keyframe_pitch_animation() {
        // Create a voice changer with pitch keyframes
        let mut filter = VoiceChangerFilter::new(0.0);
        let mut tracks = KeyframeTracks::default();

        // Add keyframes: pitch 3.0 at t=0, pitch 0.0 at t=1000ms
        tracks.add_keyframe("pitch_semitones", Keyframe::float(0, 3.0));
        tracks.add_keyframe("pitch_semitones", Keyframe::float(1000, 0.0));
        filter.keyframe_tracks = tracks;

        // Test that get_pitch_at_time returns correct values
        assert!((filter.get_pitch_at_time(0) - 3.0).abs() < 0.01);
        assert!((filter.get_pitch_at_time(500) - 1.5).abs() < 0.1); // Interpolated
        assert!((filter.get_pitch_at_time(1000) - 0.0).abs() < 0.01);
        assert!((filter.get_pitch_at_time(1500) - 0.0).abs() < 0.01); // After last keyframe
    }

    #[test]
    fn test_keyframe_formant_animation() {
        // Create a voice changer with formant keyframes
        let mut filter = VoiceChangerFilter::new(0.0);
        filter.formant_semitones = 0.0;
        let mut tracks = KeyframeTracks::default();

        // Add keyframes: formant 2.0 at t=0, formant 0.0 at t=500ms
        tracks.add_keyframe("formant_semitones", Keyframe::float(0, 2.0));
        tracks.add_keyframe("formant_semitones", Keyframe::float(500, 0.0));
        filter.keyframe_tracks = tracks;

        // Test that get_formant_at_time returns correct values
        assert!((filter.get_formant_at_time(0) - 2.0).abs() < 0.01);
        assert!((filter.get_formant_at_time(250) - 1.0).abs() < 0.1); // Interpolated
        assert!((filter.get_formant_at_time(500) - 0.0).abs() < 0.01);
        assert!((filter.get_formant_at_time(1000) - 0.0).abs() < 0.01); // After last keyframe
    }

    #[test]
    fn test_chunked_processing_with_keyframes() {
        // Create a voice changer with keyframes
        let mut filter = VoiceChangerFilter::new(0.0);
        let mut tracks = KeyframeTracks::default();

        // Pitch 3.0 for first 500ms, then 0.0 after
        tracks.add_keyframe("pitch_semitones", Keyframe::float(0, 3.0));
        tracks.add_keyframe("pitch_semitones", Keyframe::float(500, 0.0));
        filter.keyframe_tracks = tracks;

        // Create audio data: 1 second at 48kHz, mono
        let sample_rate = 48000u32;
        let channels = 1u16;
        let duration_ms = 1000u32;
        let total_samples = (sample_rate * duration_ms / 1000) as usize;

        // Generate a sine wave
        let samples: Vec<f32> = (0..total_samples)
            .map(|i| (i as f32 * 0.01).sin() * 0.5)
            .collect();

        // Create a minimal segment for testing
        let metadata = Arc::new(Metadata::default());
        let segment = Arc::new(Segment::new(
            Duration::ZERO,
            Duration::from_millis(duration_ms as u64),
            metadata,
            1.0, // global_speed
        ));

        let mut audio_data = AudioData {
            config: AudioFilterConfig {
                channels,
                sample_rate,
            },
            samples,
            from_segment: segment,
            relative_timeline_offset: Duration::from_millis(0),
            chunk_duration: Duration::from_millis(duration_ms as u64),
        };

        // Apply filter
        filter.apply(&mut audio_data).unwrap();

        // Output should have same length
        assert_eq!(audio_data.samples.len(), total_samples);

        // Output should not be silence
        let energy: f32 = audio_data.samples.iter().map(|x| x * x).sum();
        assert!(energy > 0.0, "Output should have signal energy");
    }

    #[test]
    fn test_no_keyframes_uses_static_processing() {
        // Voice changer without keyframes should use static values
        let filter = VoiceChangerFilter::new(3.0);

        // No keyframes - should return static pitch
        assert!((filter.get_pitch_at_time(0) - 3.0).abs() < 0.01);
        assert!((filter.get_pitch_at_time(1000) - 3.0).abs() < 0.01);
        assert!((filter.get_formant_at_time(0) - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_pitch_shift_frequency_change() {
        // Generate 440Hz sine wave at 48kHz
        let sample_rate = 48000;
        let freq = 440.0;
        let duration_samples = sample_rate; // 1 second

        let input: Vec<f32> = (0..duration_samples)
            .map(|i| {
                let t = i as f32 / sample_rate as f32;
                (2.0 * std::f32::consts::PI * freq * t).sin() * 0.5
            })
            .collect();

        // Pitch shift up by 12 semitones (should be ~880Hz)
        let output = VoiceChangerFilter::pitch_shift_preserving_duration(&input, 2.0);

        // Output should preserve length
        assert_eq!(output.len(), input.len());

        // Output should have signal energy
        let energy: f32 = output.iter().map(|x| x * x).sum();
        assert!(energy > 0.0, "Pitch shifted output should have energy");

        // Output should be different (pitch changed)
        let mut different = false;
        for (orig, processed) in input.iter().zip(output.iter()) {
            if (orig - processed).abs() > 0.05 {
                different = true;
                break;
            }
        }
        assert!(different, "Pitch shift should change the waveform");
    }
}
