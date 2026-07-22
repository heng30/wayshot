//! Rolling (streaming) implementations for STFT, ISTFT, GRU, and normalization.
//!
//! **NOTE:** This module is currently unused. The main inference path uses ONNX
//! Runtime with patched streaming models (`combined_streaming.onnx`) that expose
//! GRU hidden states as model I/O and time-slice the decoder path to T=1. See
//! `scripts/patch_onnx_streaming.py` and `MODES.md` for details.
//!
//! This module remains available behind `#[cfg(feature = "rolling")]` as a
//! potential alternative for platforms without ONNX Runtime or for fully
//! custom inference pipelines.
//!
//! All types in this module are designed for zero heap allocation per frame:
//! all memory is pre-allocated at construction time. This makes them suitable
//! for real-time audio processing where allocation jitter is unacceptable.
//!
//! # Components
//!
//! - [`RollingStft`] / [`RollingIstft`] — Overlap-add STFT/ISTFT matching `DFState`
//! - [`RollingGru`] — Manual GRU cell with persistent hidden state
//! - [`RollingNorm`] — ERB mean normalization and complex unit normalization

use num_complex::Complex32;

// ─────────────────────────── Constants ───────────────────────────

/// Default FFT size (matching DeepFilterNet3).
pub const FFT_SIZE: usize = 960;
/// Default hop size (matching DeepFilterNet3).
pub const HOP_SIZE: usize = 480;
/// Number of frequency bins: FFT_SIZE / 2 + 1.
pub const FREQ_SIZE: usize = FFT_SIZE / 2 + 1;
/// Overlap length: FFT_SIZE - HOP_SIZE.
const OVERLAP: usize = FFT_SIZE - HOP_SIZE;

// ─────────────────────── RollingStft ────────────────────────────

/// Streaming STFT using overlap-add, matching `DFState::analysis`.
///
/// All memory is pre-allocated. [`analysis`](Self::analysis) performs zero heap allocations.
///
/// Uses a Vorbis window: `sin(π/2 · sin²(π·n/N))` and the same FFT normalization
/// as `libDF`.
pub struct RollingStft {
    /// Overlap memory from previous frame (OVERLAP samples).
    analysis_mem: Vec<f32>,
    /// Windowed FFT input buffer (FFT_SIZE samples).
    fft_buf: Vec<f32>,
    /// Scratch buffer for FFT.
    fft_scratch: Vec<Complex32>,
    /// Vorbis window coefficients (FFT_SIZE samples).
    window: Vec<f32>,
    /// Normalization factor applied after FFT.
    wnorm: f32,
    /// FFT plan (forward: real → complex).
    fft_forward: std::sync::Arc<dyn realfft::RealToComplex<f32>>,
}

impl RollingStft {
    /// Create a new `RollingStft` with default parameters (FFT_SIZE=960, HOP_SIZE=480).
    pub fn new() -> Self {
        let mut planner = realfft::RealFftPlanner::<f32>::new();
        let fft_forward = planner.plan_fft_forward(FFT_SIZE);
        let fft_scratch = fft_forward.make_scratch_vec();
        let fft_buf = vec![0.0f32; FFT_SIZE];

        let window = vorbis_window(FFT_SIZE);
        let wnorm = 1.0 / (FFT_SIZE.pow(2) as f32 / (2 * HOP_SIZE) as f32);

        Self {
            analysis_mem: vec![0.0f32; OVERLAP],
            fft_buf,
            fft_scratch,
            window,
            wnorm,
            fft_forward,
        }
    }

    /// Perform STFT analysis on one frame of HOP_SIZE samples.
    ///
    /// - `input`: exactly HOP_SIZE time-domain samples
    /// - `output`: exactly FREQ_SIZE complex frequency bins
    ///
    /// Zero heap allocations per call.
    pub fn analysis(&mut self, input: &[f32], output: &mut [Complex32]) {
        debug_assert_eq!(input.len(), HOP_SIZE);
        debug_assert_eq!(output.len(), FREQ_SIZE);

        // Window the overlap memory (first part)
        let (buf_first, buf_second) = self.fft_buf.split_at_mut(OVERLAP);
        let (win_first, win_second) = self.window.split_at(OVERLAP);

        for ((&mem, &w), buf) in self.analysis_mem.iter().zip(win_first).zip(buf_first.iter_mut()) {
            *buf = mem * w;
        }

        // Window the new input (second part)
        for ((&inp, &w), buf) in input.iter().zip(win_second).zip(buf_second.iter_mut()) {
            *buf = inp * w;
        }

        // Shift analysis_mem: discard oldest HOP_SIZE, keep rest, append new input
        let analysis_split = OVERLAP - HOP_SIZE;
        if analysis_split > 0 {
            self.analysis_mem.rotate_left(HOP_SIZE);
        }
        self.analysis_mem[analysis_split..].copy_from_slice(input);

        // Forward FFT
        self.fft_forward
            .process_with_scratch(&mut self.fft_buf, output, &mut self.fft_scratch)
            .expect("FFT forward failed");

        // Apply normalization
        for x in output.iter_mut() {
            *x *= self.wnorm;
        }
    }

    /// Reset internal state (overlap memory).
    pub fn reset(&mut self) {
        self.analysis_mem.fill(0.0);
    }
}

// ─────────────────────── RollingIstft ───────────────────────────

/// Streaming ISTFT using overlap-add, matching `DFState::synthesis`.
///
/// All memory is pre-allocated. [`synthesis`](Self::synthesis) performs zero heap allocations.
pub struct RollingIstft {
    /// Overlap-add memory from previous frame (OVERLAP samples).
    synthesis_mem: Vec<f32>,
    /// Scratch buffer for inverse FFT output (FFT_SIZE samples).
    ifft_buf: Vec<f32>,
    /// Scratch buffer for inverse FFT.
    ifft_scratch: Vec<Complex32>,
    /// Vorbis window coefficients (FFT_SIZE samples).
    window: Vec<f32>,
    /// FFT plan (inverse: complex → real).
    fft_inverse: std::sync::Arc<dyn realfft::ComplexToReal<f32>>,
}

impl RollingIstft {
    /// Create a new `RollingIstft` with default parameters.
    pub fn new() -> Self {
        let mut planner = realfft::RealFftPlanner::<f32>::new();
        let fft_inverse = planner.plan_fft_inverse(FFT_SIZE);
        let ifft_scratch = fft_inverse.make_scratch_vec();
        let ifft_buf = fft_inverse.make_output_vec();
        let window = vorbis_window(FFT_SIZE);

        Self {
            synthesis_mem: vec![0.0f32; OVERLAP],
            ifft_buf,
            ifft_scratch,
            window,
            fft_inverse,
        }
    }

    /// Perform ISTFT synthesis: convert FREQ_SIZE complex bins back to HOP_SIZE time-domain samples.
    ///
    /// - `input`: exactly FREQ_SIZE complex frequency bins (modified in-place by FFT)
    /// - `output`: exactly HOP_SIZE time-domain samples
    ///
    /// Zero heap allocations per call.
    pub fn synthesis(&mut self, input: &mut [Complex32], output: &mut [f32]) {
        debug_assert_eq!(input.len(), FREQ_SIZE);
        debug_assert_eq!(output.len(), HOP_SIZE);

        // Inverse FFT
        let _ = self
            .fft_inverse
            .process_with_scratch(input, &mut self.ifft_buf, &mut self.ifft_scratch);

        // Apply window
        for (x, &w) in self.ifft_buf.iter_mut().zip(self.window.iter()) {
            *x *= w;
        }

        // Overlap-add: first HOP_SIZE samples + synthesis_mem = output
        let (x_first, x_second) = self.ifft_buf.split_at(HOP_SIZE);
        for ((&xi, &mem), out) in x_first.iter().zip(self.synthesis_mem.iter()).zip(output.iter_mut()) {
            *out = xi + mem;
        }

        // Update synthesis_mem with overlap from current frame
        let split = OVERLAP - HOP_SIZE;
        if split > 0 {
            self.synthesis_mem.rotate_left(HOP_SIZE);
        }
        let (s_first, s_second) = self.synthesis_mem.split_at_mut(split);
        let (xs_first, xs_second) = x_second.split_at(split);
        for (&xi, mem) in xs_first.iter().zip(s_first.iter_mut()) {
            *mem += xi; // overlap-add
        }
        for (&xi, mem) in xs_second.iter().zip(s_second.iter_mut()) {
            *mem = xi; // overwrite shifted portion
        }
    }

    /// Reset internal state (overlap-add memory).
    pub fn reset(&mut self) {
        self.synthesis_mem.fill(0.0);
    }
}

// ───────────────────────── RollingGru ───────────────────────────

/// A single GRU layer with persistent hidden state.
///
/// Implements the standard GRU equations:
/// ```text
/// z = sigmoid(Wz·x + Uz·h + bz)
/// r = sigmoid(Wr·x + Ur·h + br)
/// n = tanh(Wn·x + r*(Un·h + bn_h) + bn_x)
/// h_new = (1-z)*n + z*h
/// ```
///
/// All weight matrices and scratch buffers are pre-allocated at construction.
/// [`step`](Self::step) performs zero heap allocations.
struct GruLayer {
    input_size: usize,
    hidden_size: usize,
    // Weight matrices: [3*hidden_size, input_size] for input, [3*hidden_size, hidden_size] for recurrent
    // Stored as flat row-major: W_ih = [Wz; Wr; Wn], W_hh = [Uz; Ur; Un]
    w_ih: Vec<f32>, // [3*H, I]
    w_hh: Vec<f32>, // [3*H, H]
    b_ih: Vec<f32>, // [3*H]  (input bias: bz_x, br_x, bn_x)
    b_hh: Vec<f32>, // [3*H]  (recurrent bias: bz_h, br_h, bn_h)
    // Hidden state
    h: Vec<f32>, // [H]
    // Scratch buffers (avoid allocation per step)
    wx: Vec<f32>,  // [3*H] = W_ih @ x + b_ih
    uh: Vec<f32>,  // [3*H] = W_hh @ h + b_hh
}

impl GruLayer {
    /// Create a GRU layer with given dimensions. All weights initialized to zero.
    fn new(input_size: usize, hidden_size: usize) -> Self {
        let gates = 3 * hidden_size;
        Self {
            input_size,
            hidden_size,
            w_ih: vec![0.0; gates * input_size],
            w_hh: vec![0.0; gates * hidden_size],
            b_ih: vec![0.0; gates],
            b_hh: vec![0.0; gates],
            h: vec![0.0; hidden_size],
            wx: vec![0.0; gates],
            uh: vec![0.0; gates],
        }
    }

    /// Run one GRU step. Reads from `input`, writes result to `output`.
    /// Zero heap allocations.
    fn step(&mut self, input: &[f32], output: &mut [f32]) {
        debug_assert_eq!(input.len(), self.input_size);
        debug_assert_eq!(output.len(), self.hidden_size);
        let h = self.hidden_size;

        // wx = W_ih @ input + b_ih
        self.wx.copy_from_slice(&self.b_ih);
        mat_vec_add(&self.w_ih, input, &mut self.wx, 3 * h, self.input_size);

        // uh = W_hh @ h + b_hh
        self.uh.copy_from_slice(&self.b_hh);
        mat_vec_add(&self.w_hh, &self.h, &mut self.uh, 3 * h, h);

        // z = sigmoid(wx[0..H] + uh[0..H])
        // r = sigmoid(wx[H..2H] + uh[H..2H])
        // n = tanh(wx[2H..3H] + r * uh[2H..3H])
        // h_new = (1-z)*n + z*h
        for i in 0..h {
            let z = sigmoid(self.wx[i] + self.uh[i]);
            let r = sigmoid(self.wx[h + i] + self.uh[h + i]);
            let n = tanh_approx(self.wx[2 * h + i] + r * self.uh[2 * h + i]);
            let h_new = (1.0 - z) * n + z * self.h[i];
            self.h[i] = h_new;
            output[i] = h_new;
        }
    }

    /// Reset hidden state to zeros.
    fn reset(&mut self) {
        self.h.fill(0.0);
    }
}

/// Multi-layer GRU with persistent hidden state across time steps.
///
/// All weights, hidden states, and scratch buffers are pre-allocated at construction.
/// [`step`](Self::step) performs zero heap allocations.
pub struct RollingGru {
    layers: Vec<GruLayer>,
    /// Scratch buffer for inter-layer data flow.
    inter_buf: Vec<f32>,
}

impl RollingGru {
    /// Create a multi-layer GRU.
    ///
    /// - `input_size`: dimension of the input vector
    /// - `hidden_size`: dimension of each GRU layer's hidden state
    /// - `num_layers`: number of stacked GRU layers
    pub fn new(input_size: usize, hidden_size: usize, num_layers: usize) -> Self {
        let mut layers = Vec::with_capacity(num_layers);
        // First layer takes input_size, subsequent layers take hidden_size
        layers.push(GruLayer::new(input_size, hidden_size));
        for _ in 1..num_layers {
            layers.push(GruLayer::new(hidden_size, hidden_size));
        }
        Self {
            layers,
            inter_buf: vec![0.0; hidden_size],
        }
    }

    /// Run one time step through all GRU layers.
    ///
    /// - `input`: vector of size `input_size`
    /// - `output`: vector of size `hidden_size` (output of last layer)
    ///
    /// Zero heap allocations.
    pub fn step(&mut self, input: &[f32], output: &mut [f32]) {
        if self.layers.len() == 1 {
            self.layers[0].step(input, output);
            return;
        }

        // First layer
        self.layers[0].step(input, &mut self.inter_buf);

        // Middle layers
        for i in 1..self.layers.len() - 1 {
            // Need to copy inter_buf because step reads input and writes output
            // and they can't alias the same GruLayer fields
            let (prev_layers, rest) = self.layers.split_at_mut(i);
            let _ = prev_layers; // unused, just for the split
            // Copy current inter_buf to output as temp, then step into inter_buf
            output.copy_from_slice(&self.inter_buf);
            rest[0].step(output, &mut self.inter_buf);
        }

        // Last layer
        let last = self.layers.len() - 1;
        if last > 0 {
            // inter_buf is input to last layer
            // We need to copy because step borrows &mut self
            output.copy_from_slice(&self.inter_buf);
            self.layers[last].step(output, &mut self.inter_buf);
            output.copy_from_slice(&self.inter_buf);
        }
    }

    /// Load weights from flat arrays (PyTorch GRU format).
    ///
    /// PyTorch stores GRU weights as:
    /// - `weight_ih_lN`: shape [3*hidden_size, input_size_for_layer_N]
    /// - `weight_hh_lN`: shape [3*hidden_size, hidden_size]
    /// - `bias_ih_lN`:   shape [3*hidden_size]
    /// - `bias_hh_lN`:   shape [3*hidden_size]
    ///
    /// Each layer N expects the weights in this order.
    pub fn load_weights(
        &mut self,
        layer: usize,
        w_ih: &[f32],
        w_hh: &[f32],
        b_ih: &[f32],
        b_hh: &[f32],
    ) {
        let l = &mut self.layers[layer];
        debug_assert_eq!(w_ih.len(), 3 * l.hidden_size * l.input_size);
        debug_assert_eq!(w_hh.len(), 3 * l.hidden_size * l.hidden_size);
        debug_assert_eq!(b_ih.len(), 3 * l.hidden_size);
        debug_assert_eq!(b_hh.len(), 3 * l.hidden_size);
        l.w_ih.copy_from_slice(w_ih);
        l.w_hh.copy_from_slice(w_hh);
        l.b_ih.copy_from_slice(b_ih);
        l.b_hh.copy_from_slice(b_hh);
    }

    /// Get the hidden state for all layers as a flat slice.
    /// Layout: `[layer0_h..., layer1_h..., ...]`
    pub fn hidden_state(&self) -> Vec<f32> {
        let mut h = Vec::with_capacity(self.layers.len() * self.layers[0].hidden_size);
        for l in &self.layers {
            h.extend_from_slice(&l.h);
        }
        h
    }

    /// Set the hidden state from a flat slice.
    pub fn set_hidden_state(&mut self, h: &[f32]) {
        let hs = self.layers[0].hidden_size;
        for (i, layer) in self.layers.iter_mut().enumerate() {
            let off = i * hs;
            layer.h.copy_from_slice(&h[off..off + hs]);
        }
    }

    /// Reset hidden state of all layers to zeros.
    pub fn reset(&mut self) {
        for l in &mut self.layers {
            l.reset();
        }
    }

    /// Number of layers.
    pub fn num_layers(&self) -> usize {
        self.layers.len()
    }

    /// Hidden dimension.
    pub fn hidden_size(&self) -> usize {
        self.layers[0].hidden_size
    }
}

// ───────────────────────── RollingNorm ──────────────────────────

/// ERB mean normalization and complex unit normalization with exponential moving average state.
///
/// Matches `DFState::feat_erb` and `DFState::feat_cplx` behavior.
/// All state is pre-allocated.
pub struct RollingNorm {
    /// Running mean for ERB normalization [nb_erb].
    mean_state: Vec<f32>,
    /// Running state for complex unit normalization [nb_df].
    unit_state: Vec<f32>,
    /// ERB filter bank: number of frequency bins per ERB band.
    erb_fb: Vec<usize>,
}

impl RollingNorm {
    /// Create a new `RollingNorm` with the given ERB filter bank.
    ///
    /// - `erb_fb`: frequency bins per ERB band (length = nb_erb, sum = freq_size)
    /// - `nb_df`: number of DF frequency bins for unit normalization
    pub fn new(erb_fb: &[usize], nb_df: usize) -> Self {
        let nb_erb = erb_fb.len();

        // Initialize mean_state with linearly spaced values matching DFState
        let mean_min = -60.0f32;
        let mean_max = -90.0f32;
        let mean_step = if nb_erb > 1 {
            (mean_max - mean_min) / (nb_erb - 1) as f32
        } else {
            0.0
        };
        let mean_state: Vec<f32> = (0..nb_erb)
            .map(|i| mean_min + i as f32 * mean_step)
            .collect();

        // Initialize unit_state with linearly spaced values
        let unit_min = 0.001f32;
        let unit_max = 0.0001f32;
        let unit_step = if nb_df > 1 {
            (unit_max - unit_min) / (nb_df - 1) as f32
        } else {
            0.0
        };
        let unit_state: Vec<f32> = (0..nb_df)
            .map(|i| unit_min + i as f32 * unit_step)
            .collect();

        Self {
            mean_state,
            unit_state,
            erb_fb: erb_fb.to_vec(),
        }
    }

    /// Compute ERB features from a complex spectrum, applying band correlation and mean normalization.
    ///
    /// Matches `DFState::feat_erb`:
    /// 1. Compute band power (band_corr) using `erb_fb`
    /// 2. Convert to dB: `10 * log10(power + 1e-10)`
    /// 3. Exponential mean normalization with alpha
    /// 4. Scale by 1/40
    ///
    /// - `input`: FREQ_SIZE complex frequency bins
    /// - `alpha`: EMA smoothing factor (e.g., 0.99)
    /// - `output`: nb_erb normalized ERB features
    ///
    /// Zero heap allocations.
    pub fn erb_norm(&mut self, input: &[Complex32], alpha: f32, output: &mut [f32]) {
        debug_assert_eq!(output.len(), self.erb_fb.len());

        // Band correlation (power spectrum averaged per ERB band)
        let mut bcsum = 0usize;
        for (band_idx, &band_size) in self.erb_fb.iter().enumerate() {
            let k = 1.0 / band_size as f32;
            let mut acc = 0.0f32;
            for j in 0..band_size {
                let idx = bcsum + j;
                let c = input[idx];
                acc += (c.re * c.re + c.im * c.im) * k;
            }
            bcsum += band_size;
            output[band_idx] = acc;
        }

        // Log scale + mean normalization
        for (x, s) in output.iter_mut().zip(self.mean_state.iter_mut()) {
            *x = (*x + 1e-10).log10() * 10.0;
            *s = *x * (1.0 - alpha) + *s * alpha;
            *x -= *s;
            *x /= 40.0;
        }
    }

    /// Compute complex unit-normalized features for the DF path.
    ///
    /// Matches `DFState::feat_cplx`:
    /// 1. Exponential moving average of magnitude
    /// 2. Normalize by sqrt(EMA magnitude)
    ///
    /// - `input`: nb_df complex frequency bins (first nb_df bins of spectrum)
    /// - `alpha`: EMA smoothing factor
    /// - `output`: nb_df unit-normalized complex values
    ///
    /// Zero heap allocations.
    pub fn unit_norm(&mut self, input: &[Complex32], alpha: f32, output: &mut [Complex32]) {
        debug_assert_eq!(input.len(), self.unit_state.len());
        debug_assert_eq!(output.len(), input.len());

        for ((x, s), o) in input.iter().zip(self.unit_state.iter_mut()).zip(output.iter_mut()) {
            let mag = x.norm(); // sqrt(re² + im²)
            *s = mag * (1.0 - alpha) + *s * alpha;
            let scale = 1.0 / s.sqrt();
            *o = Complex32::new(x.re * scale, x.im * scale);
        }
    }

    /// Reset normalization states to initial values.
    pub fn reset(&mut self) {
        let nb_erb = self.mean_state.len();
        let mean_min = -60.0f32;
        let mean_max = -90.0f32;
        let mean_step = if nb_erb > 1 {
            (mean_max - mean_min) / (nb_erb - 1) as f32
        } else {
            0.0
        };
        for (i, s) in self.mean_state.iter_mut().enumerate() {
            *s = mean_min + i as f32 * mean_step;
        }

        let nb_df = self.unit_state.len();
        let unit_min = 0.001f32;
        let unit_max = 0.0001f32;
        let unit_step = if nb_df > 1 {
            (unit_max - unit_min) / (nb_df - 1) as f32
        } else {
            0.0
        };
        for (i, s) in self.unit_state.iter_mut().enumerate() {
            *s = unit_min + i as f32 * unit_step;
        }
    }

    /// Get the ERB filter bank.
    pub fn erb_fb(&self) -> &[usize] {
        &self.erb_fb
    }
}

// ─────────────────────── Helper functions ───────────────────────

/// Compute the Vorbis window: sin(π/2 · sin²(π·n/N))
fn vorbis_window(size: usize) -> Vec<f32> {
    let pi = std::f64::consts::PI;
    let half = size / 2;
    (0..size)
        .map(|i| {
            let sin_val = (0.5 * pi * (i as f64 + 0.5) / half as f64).sin();
            (0.5 * pi * sin_val * sin_val).sin() as f32
        })
        .collect()
}

/// Matrix-vector multiply-add: out += M @ x, where M is [rows x cols] row-major.
#[inline]
fn mat_vec_add(m: &[f32], x: &[f32], out: &mut [f32], rows: usize, cols: usize) {
    debug_assert_eq!(m.len(), rows * cols);
    debug_assert_eq!(x.len(), cols);
    debug_assert_eq!(out.len(), rows);
    for i in 0..rows {
        let row_off = i * cols;
        let mut acc = 0.0f32;
        for j in 0..cols {
            acc += m[row_off + j] * x[j];
        }
        out[i] += acc;
    }
}

/// Sigmoid activation: 1 / (1 + exp(-x))
#[inline]
fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}

/// Tanh activation (uses std).
#[inline]
fn tanh_approx(x: f32) -> f32 {
    x.tanh()
}

// ─────────────────────────── Tests ──────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vorbis_window_symmetry() {
        let w = vorbis_window(FFT_SIZE);
        assert_eq!(w.len(), FFT_SIZE);
        // Vorbis window should be symmetric
        for i in 0..FFT_SIZE / 2 {
            let diff = (w[i] - w[FFT_SIZE - 1 - i]).abs();
            assert!(diff < 1e-6, "Window not symmetric at {}: {} vs {}", i, w[i], w[FFT_SIZE - 1 - i]);
        }
    }

    #[test]
    fn test_stft_istft_roundtrip() {
        let mut stft = RollingStft::new();
        let mut istft = RollingIstft::new();

        // Generate a simple sine wave
        let freq = 440.0f32;
        let sr = 48000.0f32;
        let total_frames = 20;

        let mut all_input = Vec::new();
        let mut all_output = Vec::new();

        for frame_idx in 0..total_frames {
            let input: Vec<f32> = (0..HOP_SIZE)
                .map(|i| {
                    let t = (frame_idx * HOP_SIZE + i) as f32 / sr;
                    (2.0 * std::f32::consts::PI * freq * t).sin() * 0.5
                })
                .collect();
            all_input.extend_from_slice(&input);

            let mut spec = vec![Complex32::new(0.0, 0.0); FREQ_SIZE];
            stft.analysis(&input, &mut spec);

            let mut output = vec![0.0f32; HOP_SIZE];
            istft.synthesis(&mut spec, &mut output);
            all_output.extend_from_slice(&output);
        }

        // After the initial transient (first 2 frames), output should match input
        // with a delay of OVERLAP samples
        let delay = OVERLAP;
        let start = delay + HOP_SIZE; // skip transient
        let end = total_frames * HOP_SIZE - HOP_SIZE;
        if end > start {
            let mut max_err = 0.0f32;
            for i in start..end {
                if i < all_output.len() && i < all_input.len() {
                    let err = (all_output[i] - all_input[i - delay]).abs();
                    max_err = max_err.max(err);
                }
            }
            // Allow some error from windowing, but should be small
            assert!(
                max_err < 0.05,
                "STFT/ISTFT roundtrip error too large: {}",
                max_err
            );
        }
    }

    #[test]
    fn test_gru_single_layer() {
        let input_size = 4;
        let hidden_size = 3;
        let mut gru = RollingGru::new(input_size, hidden_size, 1);

        // With zero weights, output should be zero (tanh(0) = 0, sigmoid(0) = 0.5)
        // h_new = (1-0.5)*tanh(0) + 0.5*0 = 0
        let input = vec![1.0f32; input_size];
        let mut output = vec![0.0f32; hidden_size];
        gru.step(&input, &mut output);

        // With zero weights: z=sigmoid(0)=0.5, r=sigmoid(0)=0.5, n=tanh(0)=0
        // h_new = (1-0.5)*0 + 0.5*0 = 0
        for &o in &output {
            assert!((o).abs() < 1e-6, "Expected ~0, got {}", o);
        }
    }

    #[test]
    fn test_gru_multi_layer() {
        let mut gru = RollingGru::new(8, 4, 3);
        assert_eq!(gru.num_layers(), 3);
        assert_eq!(gru.hidden_size(), 4);

        let input = vec![0.1f32; 8];
        let mut output = vec![0.0f32; 4];
        gru.step(&input, &mut output);

        // Just verify it runs without panic
        gru.reset();
        gru.step(&input, &mut output);
    }

    #[test]
    fn test_rolling_norm_erb() {
        let erb_fb = vec![2, 3, 5, 7, 15]; // 5 ERB bands summing to 32 bins
        let nb_df = 10;
        let mut norm = RollingNorm::new(&erb_fb, nb_df);

        // Create a spectrum with known values
        let total_bins: usize = erb_fb.iter().sum();
        let input: Vec<Complex32> = (0..total_bins)
            .map(|i| Complex32::new(0.1 * (i as f32 + 1.0), 0.0))
            .collect();

        let mut output = vec![0.0f32; erb_fb.len()];
        norm.erb_norm(&input, 0.99, &mut output);

        // Output should be finite and reasonable
        for &o in &output {
            assert!(o.is_finite(), "ERB norm output is not finite");
        }
    }

    #[test]
    fn test_rolling_norm_unit() {
        let erb_fb = vec![2, 3, 5, 7, 15];
        let nb_df = 4;
        let mut norm = RollingNorm::new(&erb_fb, nb_df);

        let input = vec![
            Complex32::new(1.0, 0.0),
            Complex32::new(0.0, 1.0),
            Complex32::new(0.5, 0.5),
            Complex32::new(0.1, -0.2),
        ];
        let mut output = vec![Complex32::new(0.0, 0.0); nb_df];
        norm.unit_norm(&input, 0.99, &mut output);

        for o in &output {
            assert!(o.re.is_finite() && o.im.is_finite(), "Unit norm output not finite");
        }
    }
}
