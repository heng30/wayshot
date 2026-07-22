//! LaMa inpainting via ONNX Runtime.
//!
//! For fixed-size ONNX models (e.g. 512x512), wide regions are processed
//! in overlapping horizontal strips to avoid resize-induced quality loss.

use super::{
    InpaintArea, InpaintError, Inpainter, Mask, chw_to_rgb_image, composite_back, crop_to_area,
    pad_to_multiple, rgb_image_to_chw,
};
use image::RgbImage;
use ndarray::{Array3, Array4};
use ort::{session::Session, value::Tensor};

pub struct LamaInpainter {
    session: Session,
    batch_size: usize,
    model_h: usize,
    model_w: usize,
}

impl LamaInpainter {
    pub fn new(model_path: &str, batch_size: usize) -> Result<Self, ort::Error> {
        let session = Session::builder()?.commit_from_file(model_path)?;
        Ok(Self {
            session,
            batch_size,
            model_h: 512,
            model_w: 512,
        })
    }

    /// Run the ONNX model on a single padded chunk.
    fn run_model(
        &mut self,
        img_batch: &Array4<f32>,
        mask_batch: &Array4<f32>,
    ) -> Result<Array3<f32>, InpaintError> {
        let img_tensor = Tensor::from_array(img_batch.clone())?.into_dyn();
        let mask_tensor = Tensor::from_array(mask_batch.clone())?.into_dyn();
        let outputs = self.session.run(ort::inputs![img_tensor, mask_tensor])?;
        let (shape, output_data) = outputs[0].try_extract_tensor::<f32>()?;
        let out_h = shape[2] as usize;
        let out_w = shape[3] as usize;

        let mut result = Array3::zeros((3, out_h, out_w));
        for c in 0..3 {
            for y in 0..out_h {
                for x in 0..out_w {
                    let idx = (c * out_h + y) * out_w + x;
                    // Try different output formats:
                    // If output is [-1, 1]: clamp to [0, 1] via (v+1)/2
                    // If output is [0, 1]: just clamp
                    let raw = output_data[idx];
                    let pred = (raw / 255.0).clamp(0.0, 1.0);
                    result[[c, y, x]] = pred;
                }
            }
        }
        Ok(result)
    }
}

impl Inpainter for LamaInpainter {
    fn inpaint(
        &mut self,
        frames: &[RgbImage],
        mask: &Mask,
        area: &InpaintArea,
    ) -> Result<Vec<RgbImage>, InpaintError> {
        let (ymin, ymax, xmin, xmax) = *area;
        let h = (ymax - ymin) as usize;
        let w = (xmax - xmin) as usize;
        let n = frames.len();
        if n == 0 {
            return Ok(vec![]);
        }

        let mh = self.model_h;
        let mw = self.model_w;

        // If the entire region fits in model size, process as one chunk
        if h <= mh && w <= mw {
            return inpaint_direct(self, frames, mask, area, mh, mw);
        }

        // For regions larger than model size, split into overlapping horizontal strips.
        // Each strip is mw pixels wide with overlap/2 overlap on each side.
        let overlap = mw / 4; // 128px overlap for 512-wide model
        let step = mw - overlap;

        // Calculate strip start positions (in area-local coordinates)
        let mut strip_starts: Vec<usize> = Vec::new();
        let mut pos: usize = 0;
        while pos + mw <= w {
            strip_starts.push(pos);
            pos += step;
        }
        // Last strip: ensure it's always mw wide by shifting left
        if w > mw {
            if strip_starts.is_empty() || strip_starts.last().unwrap() + mw < w {
                strip_starts.push(w - mw);
            }
        } else {
            // Region fits in one strip
            strip_starts.push(0);
        }

        let mut results = Vec::with_capacity(n);

        for frame in frames {
            let (cropped, cropped_mask) = crop_to_area(frame, mask, area);
            let cropped_chw = rgb_image_to_chw(&cropped);

            // Accumulate inpainted pixels with weight for blending overlaps
            let mut accum = Array3::zeros((3, h, w));
            let mut weight = ndarray::Array2::<f32>::zeros((h, w));

            for sx in &strip_starts {
                let sx_start = *sx;
                let _sx_end = sx_start + mw;

                // Vertical: if h <= mh, use full height; otherwise take mh-sized vertical strips
                // For typical subtitle removal (short height, wide width), h usually fits.
                let sy_start = 0;
                let sy_end = h.min(mh);
                let strip_h = sy_end - sy_start;
                let strip_w = mw; // Always full model width

                // Pad to model size
                let padded_h = mh;
                let padded_w = mw;
                let mut img_block = Array4::zeros((1, 3, padded_h, padded_w));
                let mut mask_block = Array4::zeros((1, 1, padded_h, padded_w));

                // Fill image and mask from the cropped region
                // This ONNX model uses [0, 1] input range (rgb_image_to_chw already normalizes)
                for c in 0..3 {
                    for y in 0..strip_h {
                        for x in 0..strip_w {
                            let src_y = sy_start + y;
                            let src_x = sx_start + x;
                            if src_y < cropped_chw.shape()[1] && src_x < cropped_chw.shape()[2] {
                                img_block[[0, c, y, x]] = cropped_chw[[c, src_y, src_x]];
                            } else {
                                let ey = src_y.min(cropped_chw.shape()[1] - 1);
                                let ex = src_x.min(cropped_chw.shape()[2] - 1);
                                img_block[[0, c, y, x]] = cropped_chw[[c, ey, ex]];
                            }
                        }
                    }
                }
                for y in 0..strip_h {
                    for x in 0..strip_w {
                        let src_y = sy_start + y;
                        let src_x = sx_start + x;
                        if src_y < h && src_x < w {
                            mask_block[[0, 0, y, x]] =
                                cropped_mask[src_y * w + src_x] as f32 / 255.0;
                        }
                        // Out of bounds: mask stays 0 (no inpaint needed)
                    }
                }

                let strip_result = self.run_model(&img_block, &mask_block)?;

                // Compute blending weight: cosine ramp only at overlap boundaries
                // where an adjacent strip exists. At the outer edges of the
                // full region, weight is 1.0 (no ramp needed).
                let has_left_neighbor = sx_start > 0;
                let has_right_neighbor = sx_start + mw < w;
                for y in 0..strip_h {
                    for x in 0..strip_w {
                        let dest_y = sy_start + y;
                        let dest_x = sx_start + x;
                        if dest_y >= h || dest_x >= w {
                            continue;
                        }

                        let wt = if has_left_neighbor && (x as f32 / overlap as f32) < 1.0 {
                            let d = x as f32 / overlap as f32;
                            0.5 * (1.0 - (1.0 - 2.0 * d).cos())
                        } else if has_right_neighbor
                            && ((strip_w - 1 - x) as f32 / overlap as f32) < 1.0
                        {
                            let d = (strip_w - 1 - x) as f32 / overlap as f32;
                            0.5 * (1.0 - (1.0 - 2.0 * d).cos())
                        } else {
                            1.0
                        };

                        for c in 0..3 {
                            accum[[c, dest_y, dest_x]] += strip_result[[c, y, x]] * wt;
                        }
                        weight[[dest_y, dest_x]] += wt;
                    }
                }
            }

            // Normalize by weights
            for y in 0..h {
                for x in 0..w {
                    let wt = weight[[y, x]];
                    if wt > 0.0 {
                        for c in 0..3 {
                            accum[[c, y, x]] /= wt;
                        }
                    }
                }
            }

            let mut result_img = chw_to_rgb_image(&accum);
            composite_back(&mut result_img, frame, mask, area);
            results.push(result_img);
        }

        Ok(results)
    }
}

/// Direct inpaint for regions that fit within model dimensions.
fn inpaint_direct(
    inpainter: &mut LamaInpainter,
    frames: &[RgbImage],
    mask: &Mask,
    area: &InpaintArea,
    model_h: usize,
    model_w: usize,
) -> Result<Vec<RgbImage>, InpaintError> {
    let (ymin, ymax, xmin, xmax) = *area;
    let h = (ymax - ymin) as usize;
    let w = (xmax - xmin) as usize;
    let padded_h = pad_to_multiple(h, 8).max(model_h);
    let padded_w = pad_to_multiple(w, 8).max(model_w);
    let n = frames.len();

    let mut results = Vec::with_capacity(n);

    for chunk in frames.chunks(inpainter.batch_size) {
        let batch_len = chunk.len();
        let mut img_batch = Array4::zeros((batch_len, 3, padded_h, padded_w));
        let mut mask_batch = Array4::zeros((batch_len, 1, padded_h, padded_w));

        for (i, frame) in chunk.iter().enumerate() {
            let (cropped, cropped_mask) = crop_to_area(frame, mask, area);
            let chw = rgb_image_to_chw(&cropped);
            for c in 0..3 {
                for y in 0..h.min(chw.shape()[1]) {
                    for x in 0..w.min(chw.shape()[2]) {
                        img_batch[[i, c, y, x]] = chw[[c, y, x]];
                    }
                }
            }
            for y in 0..h {
                for x in 0..w {
                    mask_batch[[i, 0, y, x]] = cropped_mask[y * w + x] as f32 / 255.0;
                }
            }
        }

        let img_tensor = Tensor::from_array(img_batch)?.into_dyn();
        let mask_tensor = Tensor::from_array(mask_batch)?.into_dyn();

        let outputs = inpainter
            .session
            .run(ort::inputs![img_tensor, mask_tensor])?;
        let (shape, output_data) = outputs[0].try_extract_tensor::<f32>()?;
        let out_h = shape[2] as usize;
        let out_w = shape[3] as usize;

        for i in 0..batch_len {
            let mut result = Array3::zeros((3, h, w));
            for c in 0..3 {
                for y in 0..h.min(out_h) {
                    for x in 0..w.min(out_w) {
                        let idx = ((i * 3 + c) * out_h + y) * out_w + x;
                        result[[c, y, x]] = (output_data[idx] / 255.0).clamp(0.0, 1.0);
                    }
                }
            }
            let mut result_img = chw_to_rgb_image(&result);
            composite_back(&mut result_img, &chunk[i], mask, area);
            results.push(result_img);
        }
    }

    Ok(results)
}

