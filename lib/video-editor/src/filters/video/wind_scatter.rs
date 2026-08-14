// Wind Scatter filter — the image blows apart into pixel particles (scatter)
// or the particles fly back together to reassemble the image (reassemble).
//
// End position ("scatter"):  full image → particles drift apart in the wind
//                            direction while rotating and fading out.
// Start position ("reassemble"): particles fly back from the upwind side into
//                            place while rotating upright and fading in.
//
// Both directions read left→right for angle 0: scatter blows particles toward
// the right, reassemble pulls them back from the left — so the motion reads
// consistently with the wind direction instead of being a strict time-reverse.
// Timing follows the fade in / fade out convention: Start animates from the
// clip beginning (like FadeInFilter), End animates from the clip ending
// (like FadeOutFilter).

use crate::{
    Result,
    filters::{
        progress_ratio_from_offset,
        subtitle::style::scale_pixel_for_height,
        traits::{EffectPosition, VideoData, VideoFilter},
    },
    tracks::video_frame_cache::VideoImage,
};
use image::{Rgba, RgbaImage};
use rayon::prelude::*;
use std::time::Duration;

/// Interpolated per-frame parameters.
#[derive(Debug, Clone, Copy)]
struct ScatterParams {
    angle_deg: f32,
    tile_size: u32,
    max_rotation_deg: f32,
    speed: f32,
}

/// Precomputed per-cluster motion parameters for one frame.
#[derive(Debug, Clone, Copy)]
struct ClusterParams {
    /// Displacement vector (already eased, pixels).
    dx: f32,
    dy: f32,
    /// Rotation in radians (already eased).
    rot: f32,
    /// Alpha multiplier 0.0-1.0 (already eased).
    alpha: f32,
    /// True once the cluster has started moving (e > 0).
    started: bool,
}

#[derive(
    Debug,
    Clone,
    derivative::Derivative,
    derive_setters::Setters,
    serde::Serialize,
    serde::Deserialize,
)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[serde(default)]
#[non_exhaustive]
pub struct WindScatterFilter {
    /// Whether this filter acts on the start or end of the clip.
    /// Start = "reassemble" (scattered particles → full image)
    /// End   = "scatter"    (full image → blown-away particles)
    #[derivative(Default(value = "EffectPosition::Start"))]
    pub position: EffectPosition,

    /// Duration of the scatter/reassemble animation.
    #[derivative(Default(value = "Duration::from_secs_f32(1.0)"))]
    pub duration: Duration,

    /// Wind direction in degrees. 0 = blowing left→right,
    /// 90 = blowing top→bottom, 180 = right→left, -90 = bottom→top.
    #[derivative(Default(value = "0.0"))]
    pub angle_deg: f32,

    /// Edge length of a particle cluster in pixels (>= 1).
    /// 1 = every pixel flies independently; 4 = 4x4 micro-clusters (default).
    /// Larger values give chunkier, more visible flakes.
    #[derivative(Default(value = "4"))]
    pub tile_size: u32,

    /// Maximum rotation a particle can accumulate while scattering (degrees).
    #[derivative(Default(value = "45.0"))]
    pub max_rotation_deg: f32,

    /// How far particles scatter from their home position, as a multiple of
    /// the shorter frame side (scaled by 0.35 internally so particles stay
    /// on-screen). 1.0 = a noticeable in-frame scatter.
    #[derivative(Default(value = "1.0"))]
    pub speed: f32,

    /// Random seed. Fixed by default so the animation is deterministic —
    /// the same filter instance renders identical frames every time.
    #[derivative(Default(value = "42"))]
    pub seed: u64,
}

impl WindScatterFilter {
    pub const NAME: &'static str = "wind scatter";

    pub fn new(position: EffectPosition, duration: Duration) -> Self {
        Self {
            position,
            duration,
            ..Self::default()
        }
    }

    /// Params straight from the struct (used by tests).
    #[cfg(test)]
    fn struct_params(&self) -> ScatterParams {
        ScatterParams {
            angle_deg: self.angle_deg,
            tile_size: self.tile_size.max(1),
            max_rotation_deg: self.max_rotation_deg,
            speed: self.speed,
        }
    }

    /// splitmix64 — cheap deterministic hash for per-cluster randomness.
    fn splitmix64(mut x: u64) -> u64 {
        x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
        x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        x ^ (x >> 31)
    }

    /// Deterministic pseudo-random value in [0, 1) for a cluster + salt.
    fn cluster_rand(seed: u64, tx: u32, ty: u32, salt: u64) -> f32 {
        let mut h = seed ^ salt.wrapping_mul(0x9E37_79B9_7F4A_7C15);
        h = WindScatterFilter::splitmix64(h ^ (u64::from(tx) << 32) ^ u64::from(ty));
        ((h & 0x00FF_FFFF) as f32) / 0x0100_0000 as f32
    }

    /// Compute all per-cluster motion parameters for the given scatter progress.
    ///
    /// `p` runs 0.0 (image fully intact) → 1.0 (fully scattered/transparent).
    fn build_cluster_params(
        &self,
        params: &ScatterParams,
        width: u32,
        height: u32,
        p: f32,
    ) -> Vec<ClusterParams> {
        let tile = params.tile_size.max(1);
        let cols = width.div_ceil(tile);
        let rows = height.div_ceil(tile);
        // Particles scatter within the frame (up to ~35% of the shorter side)
        // so the scattered state stays visible — the fade never reaches black.
        let travel = (width.min(height) as f32) * 0.35 * params.speed;

        let wind_angle = params.angle_deg.to_radians();
        let max_rot = params.max_rotation_deg.to_radians();
        // Scatter (End) pushes particles along the wind direction; reassemble
        // (Start) pulls them back from the opposite side, so both read
        // left→right for angle 0.
        let direction = if self.position == EffectPosition::Start {
            -1.0
        } else {
            1.0
        };

        let mut cluster_params = Vec::with_capacity((cols * rows) as usize);
        for ty in 0..rows {
            for tx in 0..cols {
                // Delay: each cluster starts moving at a slightly different time.
                let delay = 0.25 * WindScatterFilter::cluster_rand(self.seed, tx, ty, 0);
                // Direction jitter around the wind angle.
                let jitter = (WindScatterFilter::cluster_rand(self.seed, tx, ty, 1) - 0.5) * 0.6;
                let dist_factor = 0.3 + 0.5 * WindScatterFilter::cluster_rand(self.seed, tx, ty, 2);
                let rot_amt = max_rot * WindScatterFilter::cluster_rand(self.seed, tx, ty, 3);
                let rot_sign = if WindScatterFilter::cluster_rand(self.seed, tx, ty, 4) < 0.5 {
                    -1.0
                } else {
                    1.0
                };

                // Local progress 0→1 within this cluster's own window.
                let local = ((p - delay) / (1.0 - delay).max(1e-6)).clamp(0.0, 1.0);
                // Linear motion: keep displacement and rotation proportional to
                // the progress so particles don't all fly off-screen halfway
                // through — the whole animation reads as a steady, even fade.
                let e = local;

                let angle = wind_angle + jitter;
                let dist = travel * dist_factor * e * direction;
                cluster_params.push(ClusterParams {
                    dx: angle.cos() * dist,
                    dy: angle.sin() * dist,
                    rot: rot_amt * rot_sign * e,
                    // Linear fade between fully intact (alpha 1 at p=0) and a
                    // visible scattered residue (alpha 0.35 at p=1). Both the
                    // scatter and reassemble stay watchable from the first
                    // frame — no black screen at either end.
                    alpha: 0.35 + 0.65 * (1.0 - p),
                    started: e > 0.0,
                });
            }
        }
        cluster_params
    }

    /// Bilinear interpolation sampling from source image.
    fn sample_bilinear(source: &RgbaImage, x: f32, y: f32) -> Rgba<u8> {
        let width = source.width();
        let height = source.height();

        let x = x.clamp(0.0, (width - 1) as f32);
        let y = y.clamp(0.0, (height - 1) as f32);

        let x0 = x.floor() as u32;
        let y0 = y.floor() as u32;
        let x1 = (x0 + 1).min(width - 1);
        let y1 = (y0 + 1).min(height - 1);

        let dx = x - x0 as f32;
        let dy = y - y0 as f32;

        let p00 = source.get_pixel(x0, y0);
        let p01 = source.get_pixel(x1, y0);
        let p10 = source.get_pixel(x0, y1);
        let p11 = source.get_pixel(x1, y1);

        fn interpolate(v00: u8, v01: u8, v10: u8, v11: u8, dx: f32, dy: f32) -> u8 {
            let v0 = v00 as f32 * (1.0 - dx) + v01 as f32 * dx;
            let v1 = v10 as f32 * (1.0 - dx) + v11 as f32 * dx;
            ((v0 * (1.0 - dy) + v1 * dy).clamp(0.0, 255.0)) as u8
        }

        Rgba([
            interpolate(p00[0], p01[0], p10[0], p11[0], dx, dy),
            interpolate(p00[1], p01[1], p10[1], p11[1], dx, dy),
            interpolate(p00[2], p01[2], p10[2], p11[2], dx, dy),
            interpolate(p00[3], p01[3], p10[3], p11[3], dx, dy),
        ])
    }

    /// Apply the wind scatter at a given scatter progress `p` (0 = intact, 1 = gone),
    /// using the struct's raw parameters (test/standalone entry point).
    #[cfg(test)]
    fn apply_scatter(
        &self,
        buffer: &mut RgbaImage,
        canvas_width: u32,
        canvas_height: u32,
        p: f32,
    ) -> Result<()> {
        let params = self.struct_params();
        self.apply_scatter_with_params(buffer, canvas_width, canvas_height, p, &params)
    }

    /// Apply the wind scatter with explicit params.
    fn apply_scatter_with_params(
        &self,
        buffer: &mut RgbaImage,
        canvas_width: u32,
        canvas_height: u32,
        p: f32,
        params: &ScatterParams,
    ) -> Result<()> {
        // Ensure buffer is canvas-sized.
        if buffer.width() != canvas_width || buffer.height() != canvas_height {
            let mut canvas = RgbaImage::new(canvas_width, canvas_height);
            let x = (canvas_width.saturating_sub(buffer.width())) / 2;
            let y = (canvas_height.saturating_sub(buffer.height())) / 2;
            image::imageops::overlay(&mut canvas, buffer, x as i64, y as i64);
            *buffer = canvas;
        }

        // Fully intact: nothing to do.
        if p <= 0.0 {
            return Ok(());
        }

        let tile = params.tile_size.max(1);
        let cols = canvas_width.div_ceil(tile);
        let cluster_params = self.build_cluster_params(params, canvas_width, canvas_height, p);
        let source = buffer.clone();
        let mut result = RgbaImage::new(canvas_width, canvas_height);

        result
            .par_enumerate_pixels_mut()
            .for_each(|(dst_x, dst_y, pixel)| {
                let tx = (dst_x / tile) as usize;
                let ty = (dst_y / tile) as usize;
                let cluster = cluster_params[ty * cols as usize + tx];

                if !cluster.started {
                    // Cluster hasn't moved yet: copy the source pixel as-is.
                    *pixel = *source.get_pixel(dst_x, dst_y);
                    return;
                }

                let cx = (tx as u32 * tile + tile / 2) as f32;
                let cy = (ty as u32 * tile + tile / 2) as f32;
                let px = dst_x as f32;
                let py = dst_y as f32;

                // Reverse map: dst → (translate back) → (rotate back) → src.
                let rel_x = px - cluster.dx - cx;
                let rel_y = py - cluster.dy - cy;
                let (sin_r, cos_r) = cluster.rot.sin_cos();
                let src_x = cx + rel_x * cos_r - rel_y * sin_r;
                let src_y = cy + rel_x * sin_r + rel_y * cos_r;

                if src_x < 0.0
                    || src_x >= canvas_width as f32
                    || src_y < 0.0
                    || src_y >= canvas_height as f32
                {
                    *pixel = Rgba([0, 0, 0, 0]);
                    return;
                }

                let mut sampled = Self::sample_bilinear(&source, src_x, src_y);
                if cluster.alpha < 1.0 {
                    sampled.0[0] = (sampled.0[0] as f32 * cluster.alpha) as u8;
                    sampled.0[1] = (sampled.0[1] as f32 * cluster.alpha) as u8;
                    sampled.0[2] = (sampled.0[2] as f32 * cluster.alpha) as u8;
                    sampled.0[3] = (sampled.0[3] as f32 * cluster.alpha) as u8;
                }
                *pixel = sampled;
            });

        *buffer = result;
        Ok(())
    }
}

impl VideoFilter for WindScatterFilter {
    crate::impl_default_video_filter!(WindScatterFilter);

    fn take_effect_in_layer_frame(&self) -> bool {
        false
    }

    fn apply(&self, data: &mut VideoData) -> Result<()> {
        let canvas_width = data.config.output_width;
        let canvas_height = data.config.output_height;
        let frame_time_offset = data.relative_timeline_offset;
        let segment_duration = data.from_segment.duration;

        // Timing follows the fade in / fade out convention:
        //   Start (reassemble): animates from the clip beginning, like FadeInFilter.
        //   End (scatter): animates from the clip ending, like FadeOutFilter.
        let time_until_end = segment_duration.saturating_sub(frame_time_offset);

        // Pixel-unit params are scaled to the output height so the look stays
        // consistent across resolutions.
        let params = ScatterParams {
            angle_deg: self.angle_deg,
            tile_size: scale_pixel_for_height(self.tile_size, canvas_height).max(1),
            max_rotation_deg: self.max_rotation_deg,
            speed: self.speed,
        };

        for frame in data.frames.iter_mut() {
            if let VideoImage::Image { buffer, .. } = frame {
                // p = scatter progress: 0 = image intact, 1 = fully scattered.
                let p = if self.position == EffectPosition::Start {
                    // Fade-in style: ratio 0→1 from the clip start.
                    let ratio = progress_ratio_from_offset(frame_time_offset, self.duration);
                    1.0 - ratio
                } else {
                    // Fade-out style: skip until we're inside the ending window.
                    if time_until_end > self.duration {
                        continue;
                    }
                    // time_until_end runs duration→0, so remaining visibility
                    // runs 1→0; scatter progress is the inverse.
                    1.0 - progress_ratio_from_offset(time_until_end, self.duration)
                };

                self.apply_scatter_with_params(
                    buffer,
                    canvas_width,
                    canvas_height,
                    p,
                    &params,
                )?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    fn solid_image(width: u32, height: u32) -> RgbaImage {
        RgbaImage::from_pixel(width, height, Rgba([128, 64, 200, 255]))
    }

    fn count_opaque_pixels(img: &RgbaImage) -> u32 {
        img.pixels().filter(|p| p[3] > 0).count() as u32
    }

    #[test]
    fn test_name() {
        assert_eq!(WindScatterFilter::NAME, "wind scatter");
    }

    #[test]
    fn test_defaults() {
        let f = WindScatterFilter::default();
        assert_eq!(f.position, EffectPosition::Start);
        assert_eq!(f.duration, Duration::from_secs_f32(1.0));
        assert_eq!(f.angle_deg, 0.0);
        assert_eq!(f.tile_size, 4);
        assert_eq!(f.max_rotation_deg, 45.0);
        assert_eq!(f.speed, 1.0);
        assert_eq!(f.seed, 42);
    }

    #[test]
    fn test_p0_unchanged() {
        let f = WindScatterFilter::default();
        let mut buffer = solid_image(64, 64);
        let original = buffer.clone();
        f.apply_scatter(&mut buffer, 64, 64, 0.0).unwrap();
        assert_eq!(buffer, original, "p=0 must leave the image untouched");
    }

    #[test]
    fn test_p1_visible_residue() {
        // At full scatter the frame keeps a visible particle residue instead
        // of going black — the effect is watchable from the first frame.
        let f = WindScatterFilter::default();
        let mut buffer = solid_image(64, 64);
        f.apply_scatter(&mut buffer, 64, 64, 1.0).unwrap();
        let opaque = count_opaque_pixels(&buffer);
        assert!(
            opaque > 0,
            "p=1 must keep a visible scattered residue (opaque={opaque})"
        );
        assert!(
            opaque < 64 * 64,
            "p=1 must not keep the full image intact (opaque={opaque})"
        );
    }

    #[test]
    fn test_mid_progress_changes_image() {
        let f = WindScatterFilter::default();
        let mut buffer = solid_image(64, 64);
        let original = buffer.clone();
        f.apply_scatter(&mut buffer, 64, 64, 0.5).unwrap();
        assert_ne!(buffer, original, "mid progress must alter the image");
        // Some pixels should be gone (transparent), some should still be intact.
        let opaque = count_opaque_pixels(&buffer);
        assert!(opaque > 0 && opaque < 64 * 64);
    }

    #[test]
    fn test_deterministic_same_seed() {
        let f = WindScatterFilter::default();
        let mut a = solid_image(64, 64);
        let mut b = solid_image(64, 64);
        f.apply_scatter(&mut a, 64, 64, 0.6).unwrap();
        f.apply_scatter(&mut b, 64, 64, 0.6).unwrap();
        assert_eq!(a, b, "same seed must produce identical frames");
    }

    #[test]
    fn test_scatter_and_reassemble_opposite_direction() {
        // Start (reassemble) pulls particles back from the upwind side while
        // End (scatter) pushes them downwind — so for angle 0 the scatter
        // centroid drifts right and the reassemble centroid drifts left.
        fn centroid(img: &RgbaImage) -> (f32, f32) {
            let mut sx = 0.0f32;
            let mut sy = 0.0f32;
            let mut n = 0.0f32;
            for y in 0..img.height() {
                for x in 0..img.width() {
                    if img.get_pixel(x, y)[3] > 0 {
                        sx += x as f32;
                        sy += y as f32;
                        n += 1.0;
                    }
                }
            }
            if n == 0.0 {
                (0.0, 0.0)
            } else {
                (sx / n, sy / n)
            }
        }

        let f_end = WindScatterFilter::new(EffectPosition::End, Duration::from_secs_f32(1.0));
        let f_start = WindScatterFilter::new(EffectPosition::Start, Duration::from_secs_f32(1.0));

        let mut a = solid_image(128, 128);
        let mut b = solid_image(128, 128);
        f_end.apply_scatter(&mut a, 128, 128, 0.7).unwrap();
        f_start.apply_scatter(&mut b, 128, 128, 0.7).unwrap();

        let (ax, _) = centroid(&a);
        let (bx, _) = centroid(&b);
        assert!(
            ax > 64.0,
            "End scatter should drift right with 0° wind (centroid x={ax})"
        );
        assert!(
            bx < 64.0,
            "Start reassemble should pull back from the left (centroid x={bx})"
        );
    }

    #[test]
    fn test_angle_changes_direction() {
        // Scatter (End): particles drift along the wind direction. With 0° wind
        // the centroid shifts right; with 90° wind it shifts down.
        fn centroid(img: &RgbaImage) -> (f32, f32) {
            let mut sx = 0.0f32;
            let mut sy = 0.0f32;
            let mut n = 0.0f32;
            for y in 0..img.height() {
                for x in 0..img.width() {
                    if img.get_pixel(x, y)[3] > 0 {
                        sx += x as f32;
                        sy += y as f32;
                        n += 1.0;
                    }
                }
            }
            if n == 0.0 {
                (0.0, 0.0)
            } else {
                (sx / n, sy / n)
            }
        }

        let f_right = WindScatterFilter::new(EffectPosition::End, Duration::from_secs_f32(1.0))
            .with_angle_deg(0.0);
        let f_down = WindScatterFilter::new(EffectPosition::End, Duration::from_secs_f32(1.0))
            .with_angle_deg(90.0);

        let mut a = solid_image(128, 128);
        let mut b = solid_image(128, 128);
        f_right.apply_scatter(&mut a, 128, 128, 0.7).unwrap();
        f_down.apply_scatter(&mut b, 128, 128, 0.7).unwrap();

        let (ax, _) = centroid(&a);
        let (_, by) = centroid(&b);
        assert!(
            ax > 64.0,
            "0° wind should push particles right (centroid x={ax})"
        );
        assert!(
            by > 64.0,
            "90° wind should push particles down (centroid y={by})"
        );
    }

    #[test]
    fn test_alpha_fades_roughly_linearly() {
        // The fade must be roughly linear in scatter progress between the
        // intact state (alpha≈1) and the visible residue (alpha≈0.35).
        fn avg_alpha(img: &RgbaImage) -> f32 {
            let mut sum = 0.0f32;
            let mut n = 0.0f32;
            for y in 0..img.height() {
                for x in 0..img.width() {
                    sum += img.get_pixel(x, y)[3] as f32;
                    n += 1.0;
                }
            }
            sum / n / 255.0
        }

        let f = WindScatterFilter::new(EffectPosition::End, Duration::from_secs_f32(1.0));
        let mut a = solid_image(64, 64);
        let mut b = solid_image(64, 64);
        f.apply_scatter(&mut a, 64, 64, 0.0).unwrap();
        f.apply_scatter(&mut b, 64, 64, 1.0).unwrap();

        let intact = avg_alpha(&a);
        let residue = avg_alpha(&b);
        assert!(
            intact > 0.9,
            "p=0 must stay nearly intact (intact={intact})"
        );
        assert!(
            residue > 0.2 && residue < 0.6,
            "p=1 must be a visible residue, not black (residue={residue})"
        );

        let mut mid = solid_image(64, 64);
        f.apply_scatter(&mut mid, 64, 64, 0.5).unwrap();
        let mid_alpha = avg_alpha(&mid);
        assert!(
            mid_alpha > residue && mid_alpha < intact,
            "p=0.5 must sit between the two ends (mid={mid_alpha})"
        );
    }

    #[test]
    fn test_serde_roundtrip() {
        let f = WindScatterFilter::new(EffectPosition::End, Duration::from_secs_f32(1.5))
            .with_angle_deg(45.0)
            .with_tile_size(8)
            .with_max_rotation_deg(90.0)
            .with_speed(1.2)
            .with_seed(7);

        let json = serde_json::to_string(&f).unwrap();
        let back: WindScatterFilter = serde_json::from_str(&json).unwrap();

        assert_eq!(back.position, f.position);
        assert_eq!(back.duration, f.duration);
        assert_eq!(back.angle_deg, f.angle_deg);
        assert_eq!(back.tile_size, f.tile_size);
        assert_eq!(back.max_rotation_deg, f.max_rotation_deg);
        assert_eq!(back.speed, f.speed);
        assert_eq!(back.seed, f.seed);
    }

    #[test]
    fn test_tile_size_affects_output() {
        let f_small = WindScatterFilter {
            tile_size: 1,
            ..WindScatterFilter::default()
        };
        let f_big = WindScatterFilter {
            tile_size: 16,
            ..WindScatterFilter::default()
        };
        let mut a = solid_image(64, 64);
        let mut b = solid_image(64, 64);
        f_small.apply_scatter(&mut a, 64, 64, 0.5).unwrap();
        f_big.apply_scatter(&mut b, 64, 64, 0.5).unwrap();
        assert_ne!(a, b, "different tile sizes must render differently");
    }
}
