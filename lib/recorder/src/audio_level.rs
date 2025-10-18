/// Calculate RMS (Root Mean Square) audio level in decibels.
///
/// RMS provides a measure of the average power of an audio signal,
/// which is useful for representing perceived loudness.
///
/// # Arguments
///
/// * `samples` - Audio samples as floating-point values in the range [-1.0, 1.0]
///
/// # Returns
///
/// `Some(f32)` containing the RMS level in decibels, or `None` if the input is empty.
/// Returns -200 dB for signals below the noise floor.
///
/// # Examples
///
/// ```
/// use recorder::calc_rms_level;
///
/// let samples = vec![0.5, -0.3, 0.8, -0.2];
/// let level = calc_rms_level(&samples);
///
/// match level {
///     Some(db) => println!("RMS level: {:.1} dB", db),
///     None => println!("No audio samples"),
/// }
/// ```
pub fn calc_rms_level(samples: &[f32]) -> Option<f32> {
    if samples.is_empty() {
        return None;
    }

    let sum_squares: f32 = samples.iter().map(|s| s * s).sum();

    let rms = (sum_squares / samples.len() as f32).sqrt();

    if rms <= 1e-10 {
        return Some(-200.0);
    }

    Some(20.0 * rms.log10())
}

/// Calculate peak audio level in decibels.
///
/// Peak level represents the maximum amplitude in the audio signal,
/// which is useful for detecting clipping and setting recording levels.
///
/// # Arguments
///
/// * `samples` - Audio samples as floating-point values in the range [-1.0, 1.0]
///
/// # Returns
///
/// `Some(f32)` containing the peak level in decibels, or `None` if the input is empty.
/// Returns negative infinity for signals with zero amplitude.
///
/// # Examples
///
/// ```
/// use recorder::calc_peak_level;
///
/// let samples = vec![0.5, -0.3, 0.8, -0.2];
/// let level = calc_peak_level(&samples);
///
/// match level {
///     Some(db) => println!("Peak level: {:.1} dB", db),
///     None => println!("No audio samples"),
/// }
/// ```
pub fn calc_peak_level(samples: &[f32]) -> Option<f32> {
    if samples.is_empty() {
        return None;
    }

    let max_sample = samples
        .iter()
        .map(|s| s.abs())
        .fold(0.0f32, |a, b| a.max(b));

    if max_sample <= 0.0 {
        return Some(f32::NEG_INFINITY);
    }

    Some(20.0 * max_sample.log10())
}

/// Calculate LUFS-style (Loudness Units Full Scale) audio level.
///
/// This provides a simplified approximation of LUFS loudness measurement,
/// which is commonly used in broadcast and streaming applications.
///
/// # Arguments
///
/// * `samples` - Audio samples as floating-point values in the range [-1.0, 1.0]
///
/// # Returns
///
/// `Some(f32)` containing the LUFS-style level in decibels, or `None` if the input is empty.
/// Returns -200 dB for signals below the noise floor.
///
/// # Examples
///
/// ```
/// use recorder::calc_lufs_style;
///
/// let samples = vec![0.5, -0.3, 0.8, -0.2];
/// let level = calc_lufs_style(&samples);
///
/// match level {
///     Some(db) => println!("LUFS-style level: {:.1} dB", db),
///     None => println!("No audio samples"),
/// }
/// ```
pub fn calc_lufs_style(samples: &[f32]) -> Option<f32> {
    if samples.is_empty() {
        return None;
    }

    let sum_squares: f32 = samples.iter().map(|s| s * s).sum();

    let mean_square = sum_squares / samples.len() as f32;

    if mean_square <= 1e-10 {
        return Some(-200.0);
    }

    // Simplified LUFS calculation
    Some(-0.691 + 10.0 * mean_square.log10())
}

/// Convert decibel level to normalized value in range [0.0, 1.0].
///
/// This function is useful for creating audio level meters and visualizations
/// where a normalized value is needed for display purposes.
///
/// # Arguments
///
/// * `db` - Decibel level to convert
/// * `min_db` - Minimum decibel value (maps to 0.0)
/// * `max_db` - Maximum decibel value (maps to 1.0)
///
/// # Returns
///
/// Normalized value in the range [0.0, 1.0].
///
/// # Examples
///
/// ```
/// use recorder::db_to_normalized;
///
/// let db_level = -12.5;
/// let normalized = db_to_normalized(db_level, -60.0, 0.0);
/// println!("Normalized level: {:.2}", normalized); // e.g., 0.79
/// ```
pub fn db_to_normalized(db: f32, min_db: f32, max_db: f32) -> f32 {
    let clamped = db.clamp(min_db, max_db);
    (clamped - min_db) / (max_db - min_db)
}

/// Convert decibels to linear gain coefficient
/// Supports both positive gain (amplification) and negative gain (attenuation)
fn db_to_linear(db: f32) -> f32 {
    if db <= -120.0 {
        return 0.0; // Consider as mute below -120dB
    }
    10.0f32.powf(db / 20.0)
}

pub fn apply_gain(audio_data: &mut [f32], db_gain: f32) {
    let gain = db_to_linear(db_gain);

    for sample in audio_data.iter_mut() {
        *sample *= gain;
    }
}
