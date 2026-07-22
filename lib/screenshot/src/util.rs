use image::RgbaImage;

/// Convert an RGBA image to a flat grayscale f32 vector.
pub fn to_grayscale_vec(img: &RgbaImage) -> Vec<f32> {
    img.pixels()
        .map(|p| 0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32)
        .collect()
}

/// Generate evenly spaced indices from start to end (inclusive).
pub fn linspace(start: usize, end: usize, n: usize) -> Vec<usize> {
    if n == 0 {
        return vec![];
    }
    if n == 1 {
        return vec![start];
    }
    let step = (end.saturating_sub(start)) as f32 / (n - 1) as f32;
    (0..n)
        .map(|i| (start as f32 + i as f32 * step).round() as usize)
        .collect()
}

/// Generate offset search order starting from a predicted value, expanding outward.
pub fn predict_offset_iter(max: i32, predict: i32) -> Vec<i32> {
    let p = predict.clamp(0, max);
    let mut result = vec![p];

    for delta in 1..=max {
        if p + delta <= max {
            result.push(p + delta);
        }
        if p - delta >= 0 {
            result.push(p - delta);
        }
    }

    result
}

