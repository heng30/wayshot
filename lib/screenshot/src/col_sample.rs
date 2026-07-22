use image::RgbaImage;

use crate::util::{linspace, predict_offset_iter};

pub(crate) type ColSamples = Vec<Vec<f32>>;

/// Column sampling: average grayscale of grouped columns per row.
pub fn col_sampling(img: &RgbaImage) -> ColSamples {
    let w = img.width() as usize;
    let h = img.height() as usize;

    if w == 0 || h == 0 {
        return vec![];
    }

    let groups: Vec<Vec<usize>> = if w < 200 {
        vec![
            linspace(4.min(w - 1), w / 4, 3),
            linspace(w / 2, 5 * w / 8, 3),
            linspace(6 * w / 8, 7 * w / 8, 3),
        ]
    } else {
        vec![
            linspace(20.min(w - 1), w / 8, 5),
            linspace(w / 4, 3 * w / 8, 5),
            linspace(w / 2, 5 * w / 8, 5),
            linspace(5 * w / 8, 3 * w / 4, 5),
            linspace(7 * w / 8, w.saturating_sub(20), 5),
        ]
    };

    let mut result: Vec<Vec<f32>> = vec![vec![0.0; groups.len()]; h];

    for (group_idx, cols) in groups.iter().enumerate() {
        for y in 0..h {
            let mut sum = 0.0f32;
            let mut count = 0;
            for &x in cols {
                if x < w {
                    let pixel = img.get_pixel(x as u32, y as u32);
                    let gray =
                        0.299 * pixel[0] as f32 + 0.587 * pixel[1] as f32 + 0.114 * pixel[2] as f32;
                    sum += gray;
                    count += 1;
                }
            }
            result[y][group_idx] = if count > 0 { sum / count as f32 } else { 0.0 };
        }
    }

    result
}

/// Find the best offset between two column sample sets.
pub fn diff_overlap(
    cols1: &ColSamples,
    cols2: &ColSamples,
    predict: i32,
    approx_diff: f32,
    min_overlap: u32,
) -> (i32, f32) {
    let h1 = cols1.len() as i32;
    let h2 = cols2.len() as i32;

    if h1 == 0 || h2 == 0 {
        return (0, f32::MAX);
    }

    let max_offset = (h1 - min_overlap as i32).max(0);
    if max_offset < 1 {
        return (0, f32::MAX);
    }
    let mut best = (0i32, f32::MAX);
    let mut approach_count = 0;

    for offset in predict_offset_iter(max_offset, predict) {
        let diff = compute_col_diff(cols1, cols2, offset);

        if diff < best.1 {
            best = (offset, diff);
        }

        if best.1 < approx_diff {
            approach_count += 1;
            if approach_count > 10 {
                return best;
            }
            if diff < approx_diff / 4.0 {
                return best;
            }
        }
    }

    best
}

/// Compute average absolute difference between two column sample sets at a given offset.
fn compute_col_diff(cols1: &ColSamples, cols2: &ColSamples, offset: i32) -> f32 {
    let h1 = cols1.len();
    let h2 = cols2.len();

    if h1 == 0 || h2 == 0 {
        return f32::MAX;
    }

    let num_groups = cols1.get(0).map(|v| v.len()).unwrap_or(0);
    if num_groups == 0 {
        return f32::MAX;
    }

    let mut sum = 0.0f32;
    let mut count = 0usize;

    if offset == 0 {
        let len = h1.min(h2);
        for y in 0..len {
            for g in 0..num_groups {
                let diff = (cols1[y][g] - cols2[y][g]).abs();
                sum += diff;
                count += 1;
            }
        }
    } else if offset > 0 {
        let offset_u = offset as usize;
        let len = (h1 - offset_u).min(h2 - offset_u);
        for i in 0..len {
            let y1 = offset_u + i;
            let y2 = i;
            if y1 < h1 && y2 < h2 {
                for g in 0..num_groups {
                    let diff = (cols1[y1][g] - cols2[y2][g]).abs();
                    sum += diff;
                    count += 1;
                }
            }
        }
    } else {
        let offset_u = (-offset) as usize;
        let len = (h1 - offset_u).min(h2 - offset_u);
        for i in 0..len {
            let y1 = i;
            let y2 = offset_u + i;
            if y1 < h1 && y2 < h2 {
                for g in 0..num_groups {
                    let diff = (cols1[y1][g] - cols2[y2][g]).abs();
                    sum += diff;
                    count += 1;
                }
            }
        }
    }

    if count == 0 {
        return f32::MAX;
    }

    sum / count as f32
}
