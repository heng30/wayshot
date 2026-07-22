use image::imageops;
use image::RgbaImage;

use crate::util::{predict_offset_iter, to_grayscale_vec};

const ROI_TOP_IGNORE_RATIO: f32 = 0.12;
const ROI_BOTTOM_IGNORE_RATIO: f32 = 0.08;
const ROI_SIDE_IGNORE_RATIO: f32 = 0.04;
const ROI_MIN_IGNORE_PX: u32 = 24;
const TEMPLATE_MIN_HEIGHT: u32 = 48;
const TEMPLATE_FALLBACK_MIN_SCORE: f32 = 0.72;
const TEMPLATE_FALLBACK_MIN_MARGIN: f32 = 0.015;
const TEMPLATE_VERIFY_MAX_DIFF: f32 = 18.0;

/// Find offset using NCC template matching: crop a template from top of new frame, search in prev.
///
/// Takes a template from the top 5%-25% region of the frame, then searches
/// for that template in the previous frame using NCC. The search position
/// where the template matches best determines the number of new rows to append.
///
/// Returns (offset, confidence) where offset is the number of new rows to append.
pub fn find_offset_template(
    prev: &RgbaImage,
    frame: &RgbaImage,
    predict: i32,
    _min_overlap: u32,
) -> (i32, f32) {
    let h = prev.height() as i32;
    let w = prev.width() as i32;

    if h < 100 || w < 50 || prev.width() != frame.width() || prev.height() != frame.height() {
        return (0, f32::MAX);
    }

    let skip_top = (h as f32 * 0.05) as u32;
    let template_height = (h as f32 * 0.20) as u32;
    let template = imageops::crop_imm(frame, 0, skip_top, w as u32, template_height).to_image();
    let template_gray = to_grayscale_vec(&template);
    let prev_gray = to_grayscale_vec(prev);

    let search_start = skip_top as i32;
    let search_end = h - template_height as i32;

    if search_end <= search_start {
        return (0, f32::MAX);
    }

    let mut best_offset = 0i32;
    let mut best_score = f32::MIN;

    let predict = predict.clamp(0, search_end - search_start);
    let offsets = predict_offset_iter(search_end - search_start, predict);

    for offset in offsets {
        let search_y = search_start + offset;

        let score = ncc_score(&prev_gray, &template_gray, search_y as u32, w as u32);

        if score > best_score {
            best_score = score;
            best_offset = offset;
        }

        if best_score > 0.95 {
            break;
        }
    }

    let diff = 1.0 - best_score.max(0.0);
    (best_offset, diff * 10.0)
}

/// Find offset using content-region template matching with verification.
pub fn find_offset_template_content(
    prev: &RgbaImage,
    frame: &RgbaImage,
    predict: i32,
    min_overlap: u32,
) -> Option<(i32, f32)> {
    if prev.width() != frame.width() || prev.height() != frame.height() {
        return None;
    }

    let width = prev.width();
    let height = prev.height();
    let (roi_x, roi_y, roi_w, roi_h) = content_roi(width, height);
    if roi_h < TEMPLATE_MIN_HEIGHT * 2 || roi_w < 40 {
        return None;
    }

    let template_h = (roi_h / 3).max(TEMPLATE_MIN_HEIGHT).min(roi_h - 1);
    let search_start = roi_y as i32;
    let search_end = (roi_y + roi_h - template_h) as i32;
    if search_end <= search_start {
        return None;
    }

    let prev_gray = to_grayscale_vec(prev);
    let frame_gray = to_grayscale_vec(frame);
    let frame_template_y = roi_y;

    let max_offset = (height as i32 - min_overlap as i32).max(0);
    let predict = predict.clamp(0, max_offset.min(search_end - search_start));

    let mut best_offset = 0i32;
    let mut best_score = f32::MIN;
    let mut second_score = f32::MIN;

    for offset in predict_offset_iter(search_end - search_start, predict) {
        let search_y = search_start + offset;
        if search_y < 0 || search_y + template_h as i32 > height as i32 {
            continue;
        }

        let score = ncc_score_region(
            &prev_gray,
            &frame_gray,
            width,
            roi_x,
            roi_w,
            search_y as u32,
            frame_template_y,
            template_h,
        );

        if score > best_score {
            second_score = best_score;
            best_score = score;
            best_offset = offset;
        } else if score > second_score {
            second_score = score;
        }
    }

    if best_score < TEMPLATE_FALLBACK_MIN_SCORE {
        return None;
    }

    if second_score.is_finite() && best_score - second_score < TEMPLATE_FALLBACK_MIN_MARGIN {
        return None;
    }

    let verification = overlap_mean_abs_diff(
        &prev_gray,
        &frame_gray,
        width,
        roi_x,
        roi_w,
        best_offset as u32,
        height.saturating_sub(best_offset as u32),
    );

    if !verification.is_finite() || verification > TEMPLATE_VERIFY_MAX_DIFF {
        return None;
    }

    let confidence = (1.0 - best_score.max(0.0)) * 8.0 + verification / 10.0;
    Some((best_offset, confidence))
}

/// Normalized cross-correlation score for template matching.
pub(crate) fn ncc_score(image_gray: &[f32], template_gray: &[f32], y_offset: u32, width: u32) -> f32 {
    let tmpl_len = template_gray.len();
    if tmpl_len == 0 {
        return f32::MIN;
    }

    let tmpl_mean: f32 = template_gray.iter().sum::<f32>() / tmpl_len as f32;
    let tmpl_var: f32 = template_gray
        .iter()
        .map(|&v| (v - tmpl_mean).powi(2))
        .sum::<f32>()
        / tmpl_len as f32;
    let tmpl_std = tmpl_var.sqrt();

    if tmpl_std < 1.0 {
        return f32::MIN;
    }

    let start_idx = (y_offset as usize) * (width as usize);
    let end_idx = start_idx + tmpl_len;

    if end_idx > image_gray.len() {
        return f32::MIN;
    }

    let mut img_sum = 0.0f32;
    let mut sum_img_sq = 0.0f32;

    for i in 0..tmpl_len {
        let img_val = image_gray[start_idx + i];
        img_sum += img_val;
        sum_img_sq += img_val * img_val;
    }

    let img_mean = img_sum / tmpl_len as f32;
    let img_var = sum_img_sq / tmpl_len as f32 - img_mean * img_mean;
    let img_std = img_var.max(0.0).sqrt();

    if img_std < 1.0 {
        return f32::MIN;
    }

    let mut ncc = 0.0f32;
    for (i, &tmpl_val) in template_gray.iter().enumerate() {
        let img_val = image_gray[start_idx + i];
        ncc += (tmpl_val - tmpl_mean) * (img_val - img_mean);
    }

    ncc / (tmpl_len as f32 * tmpl_std * img_std)
}

/// NCC score within a specific ROI region.
fn ncc_score_region(
    image_gray: &[f32],
    template_gray: &[f32],
    width: u32,
    roi_x: u32,
    roi_w: u32,
    image_y: u32,
    template_y: u32,
    template_h: u32,
) -> f32 {
    if roi_w == 0 || template_h == 0 || width == 0 {
        return f32::MIN;
    }

    let mut tmpl_sum = 0.0f32;
    let mut img_sum = 0.0f32;
    let mut count = 0usize;

    for row in 0..template_h {
        let tmpl_base = ((template_y + row) * width + roi_x) as usize;
        let img_base = ((image_y + row) * width + roi_x) as usize;
        for col in 0..roi_w as usize {
            tmpl_sum += template_gray[tmpl_base + col];
            img_sum += image_gray[img_base + col];
            count += 1;
        }
    }

    if count == 0 {
        return f32::MIN;
    }

    let tmpl_mean = tmpl_sum / count as f32;
    let img_mean = img_sum / count as f32;
    let mut numerator = 0.0f32;
    let mut tmpl_var = 0.0f32;
    let mut img_var = 0.0f32;

    for row in 0..template_h {
        let tmpl_base = ((template_y + row) * width + roi_x) as usize;
        let img_base = ((image_y + row) * width + roi_x) as usize;
        for col in 0..roi_w as usize {
            let tmpl = template_gray[tmpl_base + col] - tmpl_mean;
            let img = image_gray[img_base + col] - img_mean;
            numerator += tmpl * img;
            tmpl_var += tmpl * tmpl;
            img_var += img * img;
        }
    }

    if tmpl_var <= 1.0 || img_var <= 1.0 {
        return f32::MIN;
    }

    numerator / (tmpl_var.sqrt() * img_var.sqrt())
}

/// Mean absolute difference in the overlap region for verification.
fn overlap_mean_abs_diff(
    prev_gray: &[f32],
    frame_gray: &[f32],
    width: u32,
    roi_x: u32,
    roi_w: u32,
    offset: u32,
    overlap_h: u32,
) -> f32 {
    if roi_w == 0 || overlap_h == 0 {
        return f32::MAX;
    }

    let sample_h = overlap_h.min(160);
    let start_prev_y = offset + overlap_h.saturating_sub(sample_h);
    let start_frame_y = overlap_h.saturating_sub(sample_h);
    let mut sum = 0.0f32;
    let mut count = 0usize;

    for row in 0..sample_h {
        let prev_base = ((start_prev_y + row) * width + roi_x) as usize;
        let frame_base = ((start_frame_y + row) * width + roi_x) as usize;
        for col in 0..roi_w as usize {
            sum += (prev_gray[prev_base + col] - frame_gray[frame_base + col]).abs();
            count += 1;
        }
    }

    if count == 0 {
        return f32::MAX;
    }

    sum / count as f32
}

/// Compute the content region of interest (ignoring edges).
pub(crate) fn content_roi(width: u32, height: u32) -> (u32, u32, u32, u32) {
    let side = ((width as f32 * ROI_SIDE_IGNORE_RATIO) as u32).max(ROI_MIN_IGNORE_PX);
    let top = ((height as f32 * ROI_TOP_IGNORE_RATIO) as u32).max(ROI_MIN_IGNORE_PX);
    let bottom = ((height as f32 * ROI_BOTTOM_IGNORE_RATIO) as u32).max(ROI_MIN_IGNORE_PX);
    let x = side.min(width.saturating_sub(1));
    let y = top.min(height.saturating_sub(1));
    let roi_w = width.saturating_sub(x.saturating_mul(2)).max(1);
    let roi_h = height.saturating_sub(y).saturating_sub(bottom).max(1);
    (x, y, roi_w, roi_h)
}
