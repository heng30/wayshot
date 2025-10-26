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

pub fn db_to_normalized(db: f32, min_db: f32, max_db: f32) -> f32 {
    let clamped = db.clamp(min_db, max_db);
    (clamped - min_db) / (max_db - min_db)
}

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
