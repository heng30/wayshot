use num_complex::Complex32;
use once_cell::sync::Lazy;
use rayon::prelude::*;
use rustfft::{Fft, FftPlanner, num_traits::Zero};
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

struct FftCacheEntry {
    fft_forward: Arc<dyn Fft<f32>>,
    fft_inverse: Arc<dyn Fft<f32>>,
    hann_window: Vec<f32>,
}

struct FftCache {
    entries: RwLock<HashMap<usize, Arc<FftCacheEntry>>>,
}

impl FftCache {
    fn new() -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
        }
    }

    fn get_or_create(&self, n_fft: usize) -> Arc<FftCacheEntry> {
        {
            let entries = self.entries.read().unwrap();
            if let Some(entry) = entries.get(&n_fft) {
                return Arc::clone(entry);
            }
        }

        let mut entries = self.entries.write().unwrap();

        if let Some(entry) = entries.get(&n_fft) {
            return Arc::clone(entry);
        }

        let mut planner = FftPlanner::new();
        let entry = Arc::new(FftCacheEntry {
            fft_forward: planner.plan_fft_forward(n_fft),
            fft_inverse: planner.plan_fft_inverse(n_fft),
            hann_window: compute_hann(n_fft),
        });

        entries.insert(n_fft, Arc::clone(&entry));
        entry
    }
}

static FFT_CACHE: Lazy<FftCache> = Lazy::new(FftCache::new);

fn compute_hann(n_fft: usize) -> Vec<f32> {
    if n_fft <= 1 {
        return vec![1.0];
    }
    let denom = (n_fft - 1) as f32;
    (0..n_fft)
        .map(|i| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * (i as f32) / denom).cos())
        .collect()
}

pub fn to_planar_stereo(interleaved: &[f32], channels: u16) -> Vec<[f32; 2]> {
    if channels == 1 {
        interleaved.iter().map(|&x| [x, x]).collect()
    } else {
        let mut out = Vec::with_capacity(interleaved.len() / 2);
        let mut i = 0;
        while i + 1 < interleaved.len() {
            out.push([interleaved[i], interleaved[i + 1]]);
            i += 2;
        }
        out
    }
}

/// Compute complex-as-channels spectrogram for stereo with center padding.
/// Layout is [1, 4, F, Frames] flattened => channels order: L.re, L.im, R.re, R.im.
pub(crate) fn stft_cac_stereo_centered_into(
    left: &[f32],
    right: &[f32],
    n_fft: usize,
    hop: usize,
    out: &mut Vec<f32>,
) -> (usize, usize) {
    assert_eq!(left.len(), right.len());

    let t = left.len();
    let pad = n_fft / 2;

    let padded_len = pad + t + pad;
    let mut l_sig = vec![0.0f32; padded_len];
    let mut r_sig = vec![0.0f32; padded_len];

    l_sig[pad..pad + t].copy_from_slice(left);
    r_sig[pad..pad + t].copy_from_slice(right);

    let frames = 1 + (t / hop);
    let f_bins = n_fft / 2;

    let cache = FFT_CACHE.get_or_create(n_fft);
    let fft = &cache.fft_forward;
    let window = &cache.hann_window;

    out.clear();
    out.resize(4 * f_bins * frames, 0.0);

    let mut buf_l = vec![Complex32::zero(); n_fft];
    let mut buf_r = vec![Complex32::zero(); n_fft];

    for fr in 0..frames {
        let start = fr * hop;
        let li = &l_sig[start..start + n_fft];
        let ri = &r_sig[start..start + n_fft];

        for i in 0..n_fft {
            let w = window[i];
            buf_l[i] = Complex32::new(li[i] * w, 0.0);
            buf_r[i] = Complex32::new(ri[i] * w, 0.0);
        }

        fft.process(&mut buf_l);
        fft.process(&mut buf_r);

        for fi in 0..f_bins {
            let base_fr = fi * frames + fr;
            out[base_fr] = buf_l[fi].re;
            out[f_bins * frames + base_fr] = buf_l[fi].im;
            out[2 * f_bins * frames + base_fr] = buf_r[fi].re;
            out[3 * f_bins * frames + base_fr] = buf_r[fi].im;
        }
    }

    (f_bins, frames)
}

#[derive(Default)]
pub(crate) struct IstftStereoWorkspace {
    left_out: Vec<f32>,
    right_out: Vec<f32>,
    window_sum: Vec<f32>,
    buf_l: Vec<Complex32>,
    buf_r: Vec<Complex32>,
}

impl IstftStereoWorkspace {
    fn ensure_capacity(&mut self, n_fft: usize, target_length: usize) {
        let pad = n_fft / 2;
        let padded_length = target_length + 2 * pad;

        if self.left_out.len() != padded_length {
            self.left_out.resize(padded_length, 0.0);
            self.right_out.resize(padded_length, 0.0);
            self.window_sum.resize(padded_length, 0.0);
        }
        if self.buf_l.len() != n_fft {
            self.buf_l.resize(n_fft, Complex32::zero());
            self.buf_r.resize(n_fft, Complex32::zero());
        }
    }

    fn reset(&mut self) {
        self.left_out.fill(0.0);
        self.right_out.fill(0.0);
        self.window_sum.fill(0.0);
    }
}

#[derive(Default)]
pub(crate) struct IstftBatchWorkspace {
    per_source: Vec<IstftStereoWorkspace>,
}

impl IstftBatchWorkspace {
    pub(crate) fn ensure_sources(&mut self, num_sources: usize) {
        if self.per_source.len() < num_sources {
            self.per_source
                .resize_with(num_sources, IstftStereoWorkspace::default);
        }
    }
}

fn istft_cac_stereo_reconstruct(
    spec_cac: &[f32],
    f_bins: usize,
    frames: usize,
    n_fft: usize,
    hop: usize,
    target_length: usize,
    ws: &mut IstftStereoWorkspace,
) -> (usize, usize) {
    let cache = FFT_CACHE.get_or_create(n_fft);
    let ifft = &cache.fft_inverse;
    let window = &cache.hann_window;

    let pad = n_fft / 2;
    let padded_length = target_length + 2 * pad;
    ws.ensure_capacity(n_fft, target_length);
    ws.reset();

    let scale = 1.0 / (n_fft as f32);

    for fr in 0..frames {
        ws.buf_l.fill(Complex32::zero());
        ws.buf_r.fill(Complex32::zero());

        for fi in 0..f_bins {
            let base_fr = fi * frames + fr;
            ws.buf_l[fi] = Complex32::new(spec_cac[base_fr], spec_cac[f_bins * frames + base_fr]);
            ws.buf_r[fi] = Complex32::new(
                spec_cac[2 * f_bins * frames + base_fr],
                spec_cac[3 * f_bins * frames + base_fr],
            );
        }

        for fi in 1..f_bins {
            let neg_fi = n_fft - fi;
            ws.buf_l[neg_fi] = ws.buf_l[fi].conj();
            ws.buf_r[neg_fi] = ws.buf_r[fi].conj();
        }

        ws.buf_l[0].im = 0.0;
        ws.buf_r[0].im = 0.0;
        if n_fft % 2 == 0 && f_bins < n_fft {
            ws.buf_l[n_fft / 2].im = 0.0;
            ws.buf_r[n_fft / 2].im = 0.0;
        }

        ifft.process(&mut ws.buf_l);
        ifft.process(&mut ws.buf_r);

        let start = fr * hop;
        for i in 0..n_fft {
            let pos = start + i;
            if pos < padded_length {
                let w = window[i];
                ws.left_out[pos] += ws.buf_l[i].re * w * scale;
                ws.right_out[pos] += ws.buf_r[i].re * w * scale;
                ws.window_sum[pos] += w * w;
            }
        }
    }

    for i in 0..padded_length {
        let sum = ws.window_sum[i];
        if sum > 1e-10 {
            ws.left_out[i] /= sum;
            ws.right_out[i] /= sum;
        }
    }

    let start = pad.min(ws.left_out.len());
    let end = (pad + target_length).min(ws.left_out.len());
    (start, end)
}

pub(crate) fn istft_cac_stereo_into(
    spec_cac: &[f32],
    f_bins: usize,
    frames: usize,
    n_fft: usize,
    hop: usize,
    target_length: usize,
    ws: &mut IstftStereoWorkspace,
    left_dst: &mut [f32],
    right_dst: &mut [f32],
) {
    assert_eq!(left_dst.len(), target_length);
    assert_eq!(right_dst.len(), target_length);

    let (start, end) =
        istft_cac_stereo_reconstruct(spec_cac, f_bins, frames, n_fft, hop, target_length, ws);
    let copy_len = end.saturating_sub(start);

    if copy_len == target_length {
        left_dst.copy_from_slice(&ws.left_out[start..end]);
        right_dst.copy_from_slice(&ws.right_out[start..end]);
    } else {
        left_dst.fill(0.0);
        right_dst.fill(0.0);
        left_dst[..copy_len].copy_from_slice(&ws.left_out[start..end]);
        right_dst[..copy_len].copy_from_slice(&ws.right_out[start..end]);
    }
}

pub(crate) fn istft_cac_stereo_add_into(
    spec_cac: &[f32],
    f_bins: usize,
    frames: usize,
    n_fft: usize,
    hop: usize,
    target_length: usize,
    ws: &mut IstftStereoWorkspace,
    left_dst: &mut [f32],
    right_dst: &mut [f32],
) {
    assert_eq!(left_dst.len(), target_length);
    assert_eq!(right_dst.len(), target_length);

    let (start, end) =
        istft_cac_stereo_reconstruct(spec_cac, f_bins, frames, n_fft, hop, target_length, ws);

    for (dst, value) in left_dst.iter_mut().zip(ws.left_out[start..end].iter()) {
        *dst += *value;
    }
    for (dst, value) in right_dst.iter_mut().zip(ws.right_out[start..end].iter()) {
        *dst += *value;
    }
}

pub(crate) fn istft_cac_stereo_sources_add_into(
    sources_data: &[&[f32]],
    f_bins: usize,
    frames: usize,
    n_fft: usize,
    hop: usize,
    target_length: usize,
    ws: &mut IstftBatchWorkspace,
    dst: &mut [f32],
) {
    assert_eq!(dst.len(), sources_data.len() * 2 * target_length);

    ws.ensure_sources(sources_data.len());
    ws.per_source[..sources_data.len()]
        .par_iter_mut()
        .zip(sources_data.par_iter())
        .zip(dst.par_chunks_mut(2 * target_length))
        .for_each(|((workspace, spec_cac), chunk)| {
            let (left_dst, right_dst) = chunk.split_at_mut(target_length);
            istft_cac_stereo_add_into(
                spec_cac,
                f_bins,
                frames,
                n_fft,
                hop,
                target_length,
                workspace,
                left_dst,
                right_dst,
            );
        });
}

pub fn stft_cac_stereo_centered(
    left: &[f32],
    right: &[f32],
    n_fft: usize,
    hop: usize,
) -> (Vec<f32>, usize, usize) {
    let mut out = Vec::new();
    let (f_bins, frames) = stft_cac_stereo_centered_into(left, right, n_fft, hop, &mut out);
    (out, f_bins, frames)
}

pub fn istft_cac_stereo(
    spec_cac: &[f32],
    f_bins: usize,
    frames: usize,
    n_fft: usize,
    hop: usize,
    target_length: usize,
) -> (Vec<f32>, Vec<f32>) {
    let mut ws = IstftStereoWorkspace::default();
    let mut left_final = vec![0.0f32; target_length];
    let mut right_final = vec![0.0f32; target_length];
    istft_cac_stereo_into(
        spec_cac,
        f_bins,
        frames,
        n_fft,
        hop,
        target_length,
        &mut ws,
        &mut left_final,
        &mut right_final,
    );

    (left_final, right_final)
}

pub fn istft_cac_stereo_parallel(
    sources_data: &[&[f32]],
    f_bins: usize,
    frames: usize,
    n_fft: usize,
    hop: usize,
    target_length: usize,
) -> Vec<(Vec<f32>, Vec<f32>)> {
    sources_data
        .par_iter()
        .map(|spec_cac| istft_cac_stereo(spec_cac, f_bins, frames, n_fft, hop, target_length))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_abs_diff_eq;

    #[test]
    fn stft_into_matches_wrapper_output() {
        let n_fft = 1024usize;
        let hop = 256usize;
        let t = 4096usize;
        let left: Vec<f32> = (0..t).map(|i| (i as f32 * 0.01).sin()).collect();
        let right: Vec<f32> = (0..t).map(|i| (i as f32 * 0.02).cos()).collect();

        let (spec_a, f_bins_a, frames_a) = stft_cac_stereo_centered(&left, &right, n_fft, hop);
        let mut spec_b = Vec::new();
        let (f_bins_b, frames_b) =
            stft_cac_stereo_centered_into(&left, &right, n_fft, hop, &mut spec_b);

        assert_eq!(f_bins_a, f_bins_b);
        assert_eq!(frames_a, frames_b);
        assert_eq!(spec_a.len(), spec_b.len());
        for (a, b) in spec_a.iter().zip(spec_b.iter()) {
            assert_abs_diff_eq!(a, b, epsilon = 1e-7);
        }
    }

    #[test]
    fn istft_add_into_matches_wrapper_sum() {
        let n_fft = 1024usize;
        let hop = 256usize;
        let t = 4096usize;
        let left: Vec<f32> = (0..t).map(|i| (i as f32 * 0.013).sin() * 0.2).collect();
        let right: Vec<f32> = (0..t).map(|i| (i as f32 * 0.017).cos() * 0.15).collect();
        let (spec, f_bins, frames) = stft_cac_stereo_centered(&left, &right, n_fft, hop);
        let (base_left, base_right) = istft_cac_stereo(&spec, f_bins, frames, n_fft, hop, t);

        let mut ws = IstftStereoWorkspace::default();
        let mut acc_left = vec![0.25f32; t];
        let mut acc_right = vec![-0.5f32; t];
        istft_cac_stereo_add_into(
            &spec,
            f_bins,
            frames,
            n_fft,
            hop,
            t,
            &mut ws,
            &mut acc_left,
            &mut acc_right,
        );

        for i in 0..t {
            assert_abs_diff_eq!(acc_left[i], 0.25 + base_left[i], epsilon = 1e-5);
            assert_abs_diff_eq!(acc_right[i], -0.5 + base_right[i], epsilon = 1e-5);
        }
    }
}
