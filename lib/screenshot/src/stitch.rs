use image::imageops;
use image::{GenericImage, RgbaImage};

use crate::col_sample::{ColSamples, col_sampling, diff_overlap};
use crate::cutpoint::find_flat_cutpoint;
use crate::error::StitchError;
use crate::template::{find_offset_template, find_offset_template_content};

/// Algorithm used for overlap detection between consecutive frames.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Algorithm {
    /// Template matching using NCC — good accuracy.
    #[default]
    Template,
    /// Column sampling — fast but less accurate.
    ColSample,
}

/// Configuration for the stitching algorithm.
#[derive(Debug, Clone)]
pub struct StitchConfig {
    /// Minimum expected overlap in rows between consecutive frames.
    pub min_overlap: u32,
    /// Confidence threshold for accepting a match (lower is more confident).
    pub accept_diff: f32,
    /// Minimum number of new rows that must be appended.
    pub min_append: u32,
    /// Early-exit threshold for col-sample diff search.
    pub approx_diff: f32,
    /// Matching algorithm to use.
    pub algorithm: Algorithm,
    /// Enable smart cut-point: shift the seam to a flat row within the overlap.
    pub smart_cutpoint: bool,
    /// Maximum rows to search for a flat cut point.
    pub smart_cutpoint_radius: u32,
    /// Total variation threshold below which a row is considered flat enough.
    pub smart_cutpoint_flatness: f32,
}

impl Default for StitchConfig {
    fn default() -> Self {
        Self {
            min_overlap: 100,
            accept_diff: 3.5,
            min_append: 10,
            approx_diff: 1.0,
            algorithm: Algorithm::Template,
            smart_cutpoint: true,
            smart_cutpoint_radius: 30,
            smart_cutpoint_flatness: 3.0,
        }
    }
}

/// Result of stitching a sequence of images.
#[derive(Debug)]
pub struct StitchResult {
    /// The stitched long screenshot.
    pub image: RgbaImage,
    /// Per-frame stitching outcome.
    pub outcomes: Vec<StitchOutcome>,
}

/// Outcome of stitching a single frame.
#[derive(Debug, Clone)]
pub enum StitchOutcome {
    /// First frame was stored as the base image.
    FirstFrame,
    /// Frame was appended to the composite image.
    Appended { added: u32 },
    /// Frame had too little new content (scroll too small).
    NoProgress,
    /// No overlap match was found between frame and previous.
    NoMatch,
}

/// Stitch a sequence of overlapping RGBA images into a single long screenshot.
///
/// Images should be ordered top-to-bottom. Consecutive images must have partial
/// overlap (as in scroll screenshots). All images must have the same width.
pub fn stitch(images: Vec<RgbaImage>, config: StitchConfig) -> Result<StitchResult, StitchError> {
    if images.is_empty() {
        return Err(StitchError::EmptyInput);
    }

    let width = images[0].width();
    for (i, img) in images.iter().enumerate() {
        if img.width() != width {
            return Err(StitchError::DifferentWidths {
                first: width,
                mismatch: img.width(),
                index: i,
            });
        }
    }

    let mut stitcher = Stitcher::new(config);
    let mut outcomes = Vec::with_capacity(images.len());

    for image in images {
        let outcome = stitcher.push_frame(image);
        outcomes.push(outcome);
    }

    Ok(StitchResult {
        image: stitcher
            .into_image()
            .expect("stitcher should have at least one frame"),
        outcomes,
    })
}

const SIGNATURE_COLS: u32 = 18;
const SIGNATURE_ROWS: u32 = 24;
const DUPLICATE_AVG_DIFF: f32 = 1.1;
const DUPLICATE_MAX_DIFF: u8 = 4;

/// Incremental stitcher that processes frames one at a time.
///
/// The core algorithm: each new frame is compared against the *previous frame*
/// to find how much it overlaps. The offset represents how many rows of new
/// content the previous frame has beyond the overlap point. New content from
/// the current frame (rows below the overlap) is appended to the composite.
pub struct Stitcher {
    config: StitchConfig,
    full_image: Option<RgbaImage>,
    last_frame: Option<RgbaImage>,
    last_cols: Option<ColSamples>,
    last_offset: i32,
    last_signature: Option<Vec<u8>>,
}

impl Stitcher {
    /// Create a new stitcher with the given configuration.
    pub fn new(config: StitchConfig) -> Self {
        Self {
            config,
            full_image: None,
            last_frame: None,
            last_cols: None,
            last_offset: 0,
            last_signature: None,
        }
    }

    /// Push a new frame into the stitcher and return the outcome.
    pub fn push_frame(&mut self, frame: RgbaImage) -> StitchOutcome {
        let signature = frame_signature(&frame, SIGNATURE_COLS, SIGNATURE_ROWS);

        // First frame: just store it
        if self.full_image.is_none() {
            self.full_image = Some(frame.clone());
            self.last_frame = Some(frame);
            self.last_signature = Some(signature);
            self.update_index_for_algorithm();
            return StitchOutcome::FirstFrame;
        }

        // Skip near-duplicate frames (not enough scroll distance)
        if let Some(ref prev_sig) = self.last_signature {
            if is_duplicate_signature(prev_sig, &signature) {
                log::debug!("Duplicate frame skipped");
                return StitchOutcome::NoProgress;
            }
        }
        self.last_signature = Some(signature);

        // Find overlap offset between last frame and current frame
        let (offset, confidence) = self.find_offset(&frame);

        // No match: confidence too high
        if confidence > self.config.accept_diff {
            log::debug!(
                "NoMatch: offset={}, confidence={}, accept_diff={}",
                offset,
                confidence,
                self.config.accept_diff
            );
            self.update_last_frame(frame);
            return StitchOutcome::NoMatch;
        }

        // offset = number of new rows to append from current frame
        let new_height = if offset > 0 { offset as u32 } else { 0 };

        // Too little new content
        if new_height < self.config.min_append {
            log::debug!(
                "NoProgress: offset={}, new_height={}, min_append={}",
                offset,
                new_height,
                self.config.min_append
            );
            self.update_last_frame(frame);
            self.last_offset = offset;
            return StitchOutcome::NoProgress;
        }

        // Append new content
        let full = self.full_image.as_ref().expect("full image set");
        let overlap = frame.height().saturating_sub(new_height);

        // Smart cut-point: shift the seam to a flat row within the overlap.
        // We trim `cut_shift` rows from the end of the composite (overlap content)
        // and replace them with the same rows from the current frame, so the seam
        // falls on a flat row. Total stitched height stays the same.
        let cut_shift = if self.config.smart_cutpoint {
            find_flat_cutpoint(
                &frame,
                overlap,
                self.config.smart_cutpoint_radius,
                self.config.smart_cutpoint_flatness,
            )
        } else {
            0
        };

        let trim = cut_shift.min(full.height());
        let adjusted_overlap = overlap.saturating_sub(cut_shift);
        let adjusted_new_height = new_height + cut_shift;

        let mut combined = RgbaImage::new(full.width(), full.height() - trim + adjusted_new_height);
        if trim > 0 {
            let trimmed =
                imageops::crop_imm(full, 0, 0, full.width(), full.height() - trim).to_image();
            combined
                .copy_from(&trimmed, 0, 0)
                .expect("copy trimmed full image");
        } else {
            combined.copy_from(full, 0, 0).expect("copy full image");
        }

        let slice = imageops::crop_imm(
            &frame,
            0,
            adjusted_overlap,
            frame.width(),
            adjusted_new_height,
        )
        .to_image();
        combined
            .copy_from(&slice, 0, full.height() - trim)
            .expect("copy slice");

        self.full_image = Some(combined);
        self.update_last_frame(frame);
        self.last_offset = offset;
        StitchOutcome::Appended { added: new_height }
    }

    fn update_index_for_algorithm(&mut self) {
        if let Some(frame) = &self.last_frame {
            if matches!(self.config.algorithm, Algorithm::ColSample) {
                self.last_cols = Some(col_sampling(frame));
            }
        }
    }

    fn update_last_frame(&mut self, frame: RgbaImage) {
        if matches!(self.config.algorithm, Algorithm::ColSample) {
            self.last_cols = Some(col_sampling(&frame));
        }
        self.last_frame = Some(frame);
    }

    /// Find the vertical offset between the last frame and the current frame.
    fn find_offset(&self, frame: &RgbaImage) -> (i32, f32) {
        match self.config.algorithm {
            Algorithm::Template => {
                if let Some(ref prev) = self.last_frame {
                    // Try content-region template first (ignores static edges), then fallback
                    if let Some(r) = find_offset_template_content(
                        prev,
                        frame,
                        self.last_offset,
                        self.config.min_overlap,
                    ) {
                        log::debug!("Template(content) offset={}, confidence={}", r.0, r.1);
                        r
                    } else {
                        let r = find_offset_template(
                            prev,
                            frame,
                            self.last_offset,
                            self.config.min_overlap,
                        );
                        log::debug!("Template(fallback) offset={}, confidence={}", r.0, r.1);
                        r
                    }
                } else {
                    (0, f32::MAX)
                }
            }
            Algorithm::ColSample => {
                if let Some(ref cols1) = self.last_cols {
                    let cols2 = col_sampling(frame);
                    let r = diff_overlap(
                        cols1,
                        &cols2,
                        self.last_offset,
                        self.config.approx_diff,
                        self.config.min_overlap,
                    );
                    log::debug!("ColSample offset={}, confidence={}", r.0, r.1);
                    r
                } else {
                    (0, f32::MAX)
                }
            }
        }
    }

    /// Get the stitched image so far.
    pub fn full_image(&self) -> Option<&RgbaImage> {
        self.full_image.as_ref()
    }

    /// Take ownership of the stitched image.
    pub fn into_image(self) -> Option<RgbaImage> {
        self.full_image
    }
}

fn frame_signature(frame: &RgbaImage, cols: u32, rows: u32) -> Vec<u8> {
    let width = frame.width().max(1);
    let height = frame.height().max(1);
    let cols = cols.max(1);
    let rows = rows.max(1);
    let mut signature = Vec::with_capacity((cols * rows) as usize);

    for row in 0..rows {
        let y = ((row * height) / rows).min(height - 1);
        for col in 0..cols {
            let x = ((col * width) / cols).min(width - 1);
            let pixel = frame.get_pixel(x, y);
            let gray =
                (0.299 * pixel[0] as f32 + 0.587 * pixel[1] as f32 + 0.114 * pixel[2] as f32) as u8;
            signature.push(gray);
        }
    }

    signature
}

fn is_duplicate_signature(previous: &[u8], current: &[u8]) -> bool {
    if previous.len() != current.len() || previous.is_empty() {
        return false;
    }

    let mut sum = 0f32;
    let mut max_diff = 0u8;

    for (&a, &b) in previous.iter().zip(current.iter()) {
        let diff = a.abs_diff(b);
        max_diff = max_diff.max(diff);
        sum += diff as f32;
    }

    let avg = sum / previous.len() as f32;
    avg <= DUPLICATE_AVG_DIFF && max_diff <= DUPLICATE_MAX_DIFF
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage, imageops};

    fn make_scroll_canvas(width: u32, height: u32) -> RgbaImage {
        let mut img = RgbaImage::from_pixel(width, height, Rgba([245, 245, 245, 255]));

        for y in (0..height).step_by(36) {
            let accent = ((y / 3) % 180) as u8;
            for x in 24..width.saturating_sub(24) {
                let stripe = if (x / 7 + y / 11) % 2 == 0 { 220 } else { 180 };
                img.put_pixel(x, y, Rgba([accent, stripe, 80, 255]));
                if y + 1 < height {
                    img.put_pixel(x, y + 1, Rgba([30, 30, 30, 255]));
                }
            }
        }

        for block in 0..10 {
            let y0 = 30 + block * 80;
            let block_h = 34 + (block % 3) * 8;
            let color = [
                ((40u16 + block as u16 * 17) % 200) as u8,
                ((90u16 + block as u16 * 11) % 200) as u8,
                ((140u16 + block as u16 * 19) % 200) as u8,
                255,
            ];
            for y in y0..(y0 + block_h).min(height) {
                for x in 30..width.saturating_sub(30) {
                    if x % (9 + block as u32 % 5) == 0 || y % (7 + block as u32 % 4) == 0 {
                        img.put_pixel(x, y, Rgba(color));
                    }
                }
            }
        }

        img
    }

    fn crop_frame(canvas: &RgbaImage, y: u32, height: u32) -> RgbaImage {
        imageops::crop_imm(canvas, 0, y, canvas.width(), height).to_image()
    }

    #[test]
    fn stitch_two_frames_col_sample() {
        let canvas = make_scroll_canvas(320, 1000);
        let first = crop_frame(&canvas, 0, 400);
        let second = crop_frame(&canvas, 200, 400);

        let config = StitchConfig {
            algorithm: Algorithm::ColSample,
            min_overlap: 100,
            ..StitchConfig::default()
        };

        let mut stitcher = Stitcher::new(config);
        assert!(matches!(
            stitcher.push_frame(first),
            StitchOutcome::FirstFrame
        ));

        match stitcher.push_frame(second) {
            StitchOutcome::Appended { added } => {
                assert!(added >= 150 && added <= 250, "added={added}");
            }
            other => panic!("expected appended, got {other:?}"),
        }
    }

    #[test]
    fn stitch_empty_input() {
        let result = stitch(vec![], StitchConfig::default());
        assert!(matches!(result, Err(StitchError::EmptyInput)));
    }

    #[test]
    fn stitch_single_frame() {
        let frame = make_scroll_canvas(320, 300);
        let result = stitch(vec![frame], StitchConfig::default()).unwrap();
        assert_eq!(result.image.width(), 320);
        assert_eq!(result.image.height(), 300);
        assert!(matches!(result.outcomes[0], StitchOutcome::FirstFrame));
    }

    #[test]
    fn stitcher_handles_bad_frame() {
        let canvas = make_scroll_canvas(320, 1000);
        let first = crop_frame(&canvas, 0, 400);
        let bad = RgbaImage::from_pixel(320, 400, Rgba([255, 255, 255, 255]));

        let config = StitchConfig {
            algorithm: Algorithm::ColSample,
            min_overlap: 100,
            ..StitchConfig::default()
        };
        let mut stitcher = Stitcher::new(config);
        assert!(matches!(
            stitcher.push_frame(first),
            StitchOutcome::FirstFrame
        ));
        assert!(matches!(stitcher.push_frame(bad), StitchOutcome::NoMatch));
    }

    #[test]
    fn stitch_multiple_frames() {
        let canvas = make_scroll_canvas(300, 2000);
        let overlap = 200;
        let frame_height = 400;
        let step = frame_height - overlap;

        let mut frames = Vec::new();
        let mut y = 0;
        while y + frame_height <= canvas.height() {
            frames.push(crop_frame(&canvas, y, frame_height));
            y += step;
        }

        let result = stitch(
            frames,
            StitchConfig {
                min_overlap: overlap / 2,
                algorithm: Algorithm::ColSample,
                ..StitchConfig::default()
            },
        )
        .unwrap();

        assert!(
            result.image.height() > 400,
            "stitched image should be taller than one frame"
        );
        assert_eq!(result.image.width(), 300);
    }

    #[test]
    fn stitcher_incremental() {
        let canvas = make_scroll_canvas(200, 800);
        let frame1 = crop_frame(&canvas, 0, 400);
        let frame2 = crop_frame(&canvas, 200, 400);
        let frame3 = crop_frame(&canvas, 400, 400);

        let config = StitchConfig {
            algorithm: Algorithm::ColSample,
            min_overlap: 100,
            ..StitchConfig::default()
        };
        let mut stitcher = Stitcher::new(config);
        stitcher.push_frame(frame1);
        stitcher.push_frame(frame2);
        stitcher.push_frame(frame3);

        let result = stitcher.into_image().unwrap();
        assert!(result.height() > 400);
        assert_eq!(result.width(), 200);
    }
}

