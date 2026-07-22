//! # DeepFilter-RS
//!
//! Real-time noise suppression and dereverberation using DeepFilterNet neural network
//! with ONNX Runtime inference.
//!
//! This crate provides audio enhancement for real-time applications such as voice calls,
//! streaming, and live audio processing. It implements the two-stage DeepFilterNet algorithm:
//! ERB-based spectral masking (broadband noise suppression) followed by deep filtering
//! (complex FIR filtering on low frequencies for phase-preserving dereverberation).
//!
//! ## Quick Start
//!
//! Use [`DeepFilterStream`] for the simplest API - it handles buffering internally:
//!
//! ```ignore
//! use deepfilter_rs::DeepFilterStream;
//!
//! let mut stream = DeepFilterStream::new(Path::new("models/dfn3_h0"))?;
//! stream.warmup()?;
//!
//! // Process any length of audio (must be 48kHz mono f32)
//! let enhanced = stream.process(&input_samples)?;
//! ```
//!
//! ## Model Variants
//!
//! | Variant | Folder | Latency | Mode | Use Case |
//! |---------|--------|---------|------|----------|
//! | DeepFilterNet2 | `dfn2` | 30ms | Stateless | General purpose |
//! | DeepFilterNet2-LL | `dfn2_ll` | 10ms | Stateless | Low-latency |
//! | DeepFilterNet2-H0 | `dfn2_h0` | 30ms | Stateful | Best quality (GRU) |
//! | DeepFilterNet3 | `dfn3` | 30ms | Stateless | High quality |
//! | DeepFilterNet3-LL | `dfn3_ll` | 10ms | Stateless | Real-time |
//! | DeepFilterNet3-H0 | `dfn3_h0` | 30ms | Stateful | Best quality (GRU) |
//!
//! Variant is auto-detected from the model folder name and `config.ini`.
//!
//! ## Audio Requirements
//!
//! - **Sample rate**: 48 kHz (resample before processing if needed)
//! - **Format**: Mono f32 samples in range [-1.0, 1.0]
//!
//! ## API Levels
//!
//! - [`DeepFilterStream`] - High-level streaming API. Handles buffering internally.
//!   Pass any length of samples, get enhanced audio back.
//!
//! - [`DeepFilterProcessor`] - Low-level frame API for integration with audio callbacks.
//!   You manage 480-sample frames yourself (10ms at 48kHz).
//!
//! ## Thread Count
//!
//! The `with_threads` constructors control ONNX Runtime's intra-op parallelism:
//!
//! - **Real-time audio**: Use 1-2 threads to minimize latency jitter
//! - **Batch/offline**: Use more threads (4-8) for throughput
//! - **Default**: ONNX Runtime picks based on CPU cores
//!
//! ## Thread Safety
//!
//! Each processor instance is independent and `Send`. Create separate instances
//! for parallel processing - they do not share state.

pub mod model;

/// Rolling (streaming) implementations for STFT, ISTFT, GRU, and normalization.
/// Currently unused — streaming is handled by patched ONNX models.
#[cfg(feature = "rolling")]
pub mod rolling;

pub use model::Model;

use deep_filter::DFState;
use num_complex::Complex32;
use ort::session::{Session, builder::GraphOptimizationLevel};
use ort::value::Tensor;
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::Path;
use std::sync::Once;
use thiserror::Error;

// Global ORT initialization guard - ensures init is called only once
static ORT_INIT: Once = Once::new();

fn init_ort() -> Result<()> {
    ORT_INIT.call_once(|| {
        // ort 2.0.0-rc.12: init() returns EnvironmentBuilder directly
        let _ = ort::init().with_name("deepfilter").commit();
    });

    Ok(())
}

/// ONNX session configuration.
///
/// - **Combined**: Single combined.onnx (windowed batch inference, existing approach).
/// - **Streaming**: Split encoder (enc_conv + enc_gru) + separate decoder sessions.
///   Processes one frame at a time through GRUs for proper state continuity.
///   Achieves near-identical quality to Tract's PulsedModel streaming.
enum Sessions {
    Combined(Session),
    Streaming {
        enc_conv: Session,
        enc_gru: Session,
        erb_dec: Session,
        df_dec: Session,
    },
}

// Common parameters for all DeepFilterNet models
pub const SAMPLE_RATE: usize = 48000;
pub const FFT_SIZE: usize = 960;
pub const HOP_SIZE: usize = 480;
pub const FREQ_SIZE: usize = 481;
pub const NB_ERB: usize = 32;
pub const NB_DF: usize = 96;
pub const DF_ORDER: usize = 5;
pub const DEFAULT_NORM_ALPHA: f32 = 0.99;
// Match DeepFilterNet CLI defaults (enhance_wav.rs)
pub const MIN_DB_THRESH: f32 = -15.0;
pub const MAX_DB_ERB_THRESH: f32 = 35.0;
pub const MAX_DB_DF_THRESH: f32 = 35.0;
/// For stateless models, use a larger window so the GRU warms up properly each frame.
/// ~400ms of context at 10ms/frame. Trade-off: more computation per frame but smoother output.
const STATELESS_WINDOW: usize = 40;

/// Which ONNX session layout to use for inference.
///
/// - `Auto` (default): pick the best available from the model directory.
/// - `SplitStreaming`: 4 separate sessions (enc_conv, enc_gru, erb_dec, df_dec).
/// - `CombinedStreaming`: single `combined_streaming.onnx` with GRU states.
/// - `Stateless`: single `combined.onnx`, 40-frame window warm-up per frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SessionMode {
    /// Auto-detect best mode from files in model directory (default).
    Auto,
    /// Split streaming: 4 separate ONNX sessions with explicit GRU state I/O.
    SplitStreaming,
    /// Combined streaming: single `combined_streaming.onnx` with GRU states.
    CombinedStreaming,
    /// Stateless: single `combined.onnx`, no persistent GRU state.
    Stateless,
}

/// Internal inference mode (derived from ModelVariant).
#[derive(Debug, Clone, Copy, PartialEq)]
enum InferenceMode {
    StatefulH0,
    StatelessWindowLast,
}

impl InferenceMode {
    /// Minimum frames needed before inference can run.
    fn min_required_frames(self, lookahead: usize) -> usize {
        match self {
            InferenceMode::StatefulH0 => 1,
            InferenceMode::StatelessWindowLast => lookahead + 1,
        }
    }
}

#[derive(Error, Debug)]
pub enum DfError {
    #[error("ONNX runtime error: {0}")]
    Onnx(#[from] ort::Error),
    #[error("Shape error: {0}")]
    Shape(#[from] ndarray::ShapeError),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Config error: {0}")]
    Config(String),
}

// ort 2.0.0-rc.12: Session builder methods return Error<SessionBuilder>
// which converts to Error<()> via From impl
impl From<ort::Error<ort::session::builder::SessionBuilder>> for DfError {
    fn from(err: ort::Error<ort::session::builder::SessionBuilder>) -> Self {
        DfError::Onnx(err.into())
    }
}

pub type Result<T> = std::result::Result<T, DfError>;

/// Model variant (auto-detected from folder name and config.ini).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ModelVariant {
    /// Standard DeepFilterNet2 (30ms latency, stateless)
    DeepFilterNet2,
    /// Low-latency DeepFilterNet2 (10ms latency, stateless)
    DeepFilterNet2LL,
    /// Stateful DeepFilterNet2 with GRU (30ms latency, best quality)
    DeepFilterNet2H0,
    /// Standard DeepFilterNet3 (30ms latency, stateless)
    DeepFilterNet3,
    /// Low-latency DeepFilterNet3 (10ms latency, stateless)
    DeepFilterNet3LL,
    /// Stateful DeepFilterNet3 with GRU (30ms latency, best quality)
    DeepFilterNet3H0,
}

impl ModelVariant {
    /// Detect variant from model directory (folder name + config.ini).
    pub fn from_model_dir(model_dir: &Path) -> Result<Self> {
        let folder_name = model_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();

        // Check for H0 suffix first (stateful models)
        let is_h0 = folder_name.ends_with("_h0");

        // Read config for model type and lookahead
        let config_path = model_dir.join("config.ini");
        let content = fs::read_to_string(&config_path).unwrap_or_default();
        let params = parse_ini(&content);

        let model = params
            .get("model")
            .map(|s| s.as_str())
            .unwrap_or("deepfilternet3");
        let lookahead = params
            .get("df_lookahead")
            .and_then(|s| s.parse::<i32>().ok())
            .unwrap_or(2);

        match (model, lookahead, is_h0) {
            ("deepfilternet2", _, true) => Ok(ModelVariant::DeepFilterNet2H0),
            ("deepfilternet2", 0, false) => Ok(ModelVariant::DeepFilterNet2LL),
            ("deepfilternet2", _, false) => Ok(ModelVariant::DeepFilterNet2),
            ("deepfilternet3", _, true) => Ok(ModelVariant::DeepFilterNet3H0),
            ("deepfilternet3", 0, false) => Ok(ModelVariant::DeepFilterNet3LL),
            ("deepfilternet3", _, false) => Ok(ModelVariant::DeepFilterNet3),
            (_, _, true) => Ok(ModelVariant::DeepFilterNet3H0),
            (_, 0, false) => Ok(ModelVariant::DeepFilterNet3LL),
            _ => Ok(ModelVariant::DeepFilterNet3),
        }
    }

    /// Whether this is a low-latency variant (10ms vs 30ms).
    pub fn is_low_latency(&self) -> bool {
        matches!(
            self,
            ModelVariant::DeepFilterNet2LL | ModelVariant::DeepFilterNet3LL
        )
    }

    /// Whether this is a stateful (H0/GRU) variant.
    pub fn is_stateful(&self) -> bool {
        matches!(
            self,
            ModelVariant::DeepFilterNet2H0 | ModelVariant::DeepFilterNet3H0
        )
    }

    /// Human-readable name.
    pub fn name(&self) -> &'static str {
        match self {
            ModelVariant::DeepFilterNet2 => "DeepFilterNet2",
            ModelVariant::DeepFilterNet2LL => "DeepFilterNet2-LL",
            ModelVariant::DeepFilterNet2H0 => "DeepFilterNet2-H0",
            ModelVariant::DeepFilterNet3 => "DeepFilterNet3",
            ModelVariant::DeepFilterNet3LL => "DeepFilterNet3-LL",
            ModelVariant::DeepFilterNet3H0 => "DeepFilterNet3-H0",
        }
    }

    /// Internal inference mode for this variant.
    fn inference_mode(&self) -> InferenceMode {
        if self.is_stateful() {
            InferenceMode::StatefulH0
        } else {
            InferenceMode::StatelessWindowLast
        }
    }
}

/// Simple INI parser
fn parse_ini(content: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('[') || line.starts_with('#') || line.is_empty() {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            map.insert(key.trim().to_string(), value.trim().to_string());
        }
    }
    map
}

/// Real-time DeepFilterNet processor
pub struct DeepFilterProcessor {
    sessions: Sessions,
    df_state: DFState,
    rolling_spec_buf_x: VecDeque<Vec<Complex32>>, // noisy spec
    rolling_spec_buf_y: VecDeque<Vec<Complex32>>, // stage-1 enhanced spec
    erb_feat_buf: VecDeque<Vec<f32>>,
    spec_feat_buf: VecDeque<Vec<Complex32>>,
    // Flat ring buffers for encoder temporal context (avoid VecDeque<Vec<>> clones)
    enc_erb_ring: Vec<f32>,  // [enc_window * NB_ERB], ring buffer
    enc_spec_ring: Vec<f32>, // [enc_window * NB_DF * 2], ring buffer (re/im interleaved)
    enc_ring_pos: usize,     // write position in ring (0..enc_window-1)
    enc_ring_count: usize,   // frames written so far (saturates at enc_window)
    frames_processed: usize,
    lookahead: usize, // model's conv_lookahead (determines delay and DF centering)
    enc_h: Vec<f32>,
    enc_hidden_dim: usize,
    enc_window: usize,
    inference_mode: InferenceMode,
    norm_alpha: f32,
    variant: ModelVariant,
    // Pre-allocated per-frame work buffers (avoid heap allocation per frame)
    work_spec: Vec<Complex32>,
    work_erb_feat: Vec<f32>,
    work_spec_feat_input: Vec<Complex32>,
    work_spec_feat_cplx: Vec<Complex32>,
    work_out_spec: Vec<Complex32>,
    work_zeros: Vec<f32>,
    // Pre-allocated inference I/O buffers
    inf_erb_data: Vec<f32>,  // [enc_window * NB_ERB]
    inf_spec_data: Vec<f32>, // [2 * enc_window * NB_DF]
    inf_mask: Vec<f32>,      // [NB_ERB]
    inf_df_coefs: Vec<f32>,  // [NB_DF * DF_ORDER * 2]
    pre_gru_emb_dim: usize,  // pre-GRU embedding dim (conv_ch * NB_ERB / 4)
    // Decoder GRU states (only used in StatefulH0 mode with fully-stateful models)
    erb_dec_h: Vec<f32>, // ERB decoder GRU hidden state
    erb_dec_hidden_dim: usize,
    erb_dec_num_layers: usize,
    df_dec_h: Vec<f32>, // DF decoder GRU hidden state
    df_dec_hidden_dim: usize,
    df_dec_num_layers: usize,
}

impl DeepFilterProcessor {
    /// Create processor from model directory.
    ///
    /// Auto-detects model variant and session mode from folder name, config.ini,
    /// and available ONNX files.
    pub fn new(model_dir: &Path) -> Result<Self> {
        let variant =
            ModelVariant::from_model_dir(model_dir).unwrap_or(ModelVariant::DeepFilterNet3);
        Self::build(model_dir, variant, Some(2), SessionMode::Auto)
    }

    /// Create processor with explicit thread count.
    ///
    /// Controls ONNX Runtime's intra-op parallelism (threads used within each inference call).
    /// - For real-time audio: use 1-2 to minimize latency variance
    /// - For batch/offline: use 4+ for throughput
    pub fn with_threads(model_dir: &Path, intra_threads: usize) -> Result<Self> {
        let variant =
            ModelVariant::from_model_dir(model_dir).unwrap_or(ModelVariant::DeepFilterNet3);
        Self::build(model_dir, variant, Some(intra_threads), SessionMode::Auto)
    }

    /// Create processor with explicit variant and thread count.
    ///
    /// Use this when you want to override auto-detection (e.g., folder name doesn't match model).
    pub fn with_variant_and_threads(
        model_dir: &Path,
        variant: ModelVariant,
        intra_threads: Option<usize>,
    ) -> Result<Self> {
        Self::build(model_dir, variant, intra_threads, SessionMode::Auto)
    }

    /// Create processor with explicit session mode, variant, and thread count.
    ///
    /// Use [`SessionMode::SplitStreaming`], [`SessionMode::CombinedStreaming`], or
    /// [`SessionMode::Stateless`] to force a specific inference mode regardless of
    /// which files exist.
    pub fn with_mode(
        model_dir: &Path,
        session_mode: SessionMode,
        intra_threads: Option<usize>,
    ) -> Result<Self> {
        let variant =
            ModelVariant::from_model_dir(model_dir).unwrap_or(ModelVariant::DeepFilterNet3);
        Self::build(model_dir, variant, intra_threads, session_mode)
    }

    /// Internal builder with all parameters.
    fn build(
        model_dir: &Path,
        variant: ModelVariant,
        intra_threads: Option<usize>,
        session_mode: SessionMode,
    ) -> Result<Self> {
        init_ort()?;
        let inference_mode = variant.inference_mode();

        let (
            df_lookahead,
            conv_lookahead,
            min_nb_erb_freqs,
            norm_alpha,
            enc_kernel_t,
            enc_hidden_dim,
            pre_gru_emb_dim,
            erb_dec_num_layers,
            erb_dec_hidden_dim,
            df_dec_num_layers,
            df_dec_hidden_dim,
        ) = {
            let config_path = model_dir.join("config.ini");
            let content = fs::read_to_string(&config_path).unwrap_or_default();
            let params = parse_ini(&content);
            let df_lookahead = params
                .get("df_lookahead")
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(2);
            let conv_lookahead = params
                .get("conv_lookahead")
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(0);
            let min_nb_erb_freqs = params
                .get("min_nb_erb_freqs")
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(1);
            let enc_kernel_t = params
                .get("conv_kernel_inp")
                .and_then(|s| s.split(',').next())
                .and_then(|s| s.trim().parse::<usize>().ok())
                .unwrap_or(3);
            let enc_hidden_dim = params
                .get("emb_hidden_dim")
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(256);
            // Pre-GRU embedding dim = conv_ch * nb_erb / 4 (output of combine layer)
            let conv_ch = params
                .get("conv_ch")
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(64);
            let pre_gru_emb_dim = conv_ch * NB_ERB / 4;
            // ERB decoder GRU: emb_num_layers - 1 layers, hidden_dim = emb_hidden_dim
            let emb_num_layers = params
                .get("emb_num_layers")
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(2);
            let erb_dec_num_layers = emb_num_layers.saturating_sub(1).max(1);
            let erb_dec_hidden_dim = enc_hidden_dim;
            // DF decoder GRU: df_num_layers layers, hidden_dim = df_hidden_dim
            let df_dec_num_layers = params
                .get("df_num_layers")
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(2);
            let df_dec_hidden_dim = params
                .get("df_hidden_dim")
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(256);
            let norm_alpha =
                if let Some(tau) = params.get("norm_tau").and_then(|s| s.parse::<f32>().ok()) {
                    let dt = HOP_SIZE as f32 / SAMPLE_RATE as f32;
                    let a = f32::exp(-dt / tau);
                    // Match python get_norm_alpha rounding behavior
                    let mut precision: u32 = 3;
                    let mut rounded = 1.0f32;
                    while rounded >= 1.0 {
                        let scale = 10f32.powi(precision as i32);
                        rounded = (a * scale).round() / scale;
                        precision += 1;
                    }
                    rounded
                } else {
                    DEFAULT_NORM_ALPHA
                };
            (
                df_lookahead,
                conv_lookahead,
                min_nb_erb_freqs,
                norm_alpha,
                enc_kernel_t,
                enc_hidden_dim,
                pre_gru_emb_dim,
                erb_dec_num_layers,
                erb_dec_hidden_dim,
                df_dec_num_layers,
                df_dec_hidden_dim,
            )
        };
        let lookahead = df_lookahead.max(conv_lookahead);

        let build_session = |path: std::path::PathBuf| -> Result<Session> {
            let mut builder =
                Session::builder()?.with_optimization_level(GraphOptimizationLevel::Level3)?;

            if let Some(threads) = intra_threads {
                builder = builder.with_intra_threads(threads)?;
            }

            Ok(builder.commit_from_file(path)?)
        };

        // Model file paths
        let enc_conv_path = model_dir.join("enc_conv_streaming.onnx");
        let enc_gru_path = model_dir.join("enc_gru_streaming.onnx");
        let erb_dec_streaming_path = model_dir.join("erb_dec_streaming.onnx");
        let df_dec_streaming_path = model_dir.join("df_dec_streaming.onnx");
        let erb_dec_path = if erb_dec_streaming_path.exists() {
            erb_dec_streaming_path
        } else {
            model_dir.join("erb_dec.onnx")
        };
        let df_dec_path = if df_dec_streaming_path.exists() {
            df_dec_streaming_path
        } else {
            model_dir.join("df_dec.onnx")
        };
        let combined_streaming_path = model_dir.join("combined_streaming.onnx");
        let combined_path = model_dir.join("combined.onnx");

        let has_split = enc_conv_path.exists()
            && enc_gru_path.exists()
            && erb_dec_path.exists()
            && df_dec_path.exists();
        let has_combined_streaming = combined_streaming_path.exists();
        let has_combined = combined_path.exists();

        let load_split = || -> Result<(Sessions, InferenceMode)> {
            if !has_split {
                return Err(DfError::Config(
                    "Split streaming requested but enc_conv_streaming.onnx / enc_gru_streaming.onnx not found".to_string(),
                ));
            }
            log::info!("Loading streaming model (split encoder + separate decoders)");
            Ok((
                Sessions::Streaming {
                    enc_conv: build_session(enc_conv_path.clone())?,
                    enc_gru: build_session(enc_gru_path.clone())?,
                    erb_dec: build_session(erb_dec_path.clone())?,
                    df_dec: build_session(df_dec_path.clone())?,
                },
                InferenceMode::StatefulH0,
            ))
        };

        let load_combined_streaming = || -> Result<(Sessions, InferenceMode)> {
            if !has_combined_streaming {
                return Err(DfError::Config(
                    "Combined streaming requested but combined_streaming.onnx not found"
                        .to_string(),
                ));
            }
            let session = build_session(combined_streaming_path.clone())?;
            let has_h0 = session.inputs().iter().any(|i| i.name() == "h0");
            if !has_h0 {
                return Err(DfError::Config(
                    "combined_streaming.onnx exists but has no h0 input".to_string(),
                ));
            }
            log::info!("Loading combined streaming model (combined_streaming.onnx)");
            Ok((Sessions::Combined(session), InferenceMode::StatefulH0))
        };

        let load_stateless = || -> Result<(Sessions, InferenceMode)> {
            if !has_combined {
                return Err(DfError::Config(
                    "Stateless mode requested but combined.onnx not found".to_string(),
                ));
            }
            let session = build_session(combined_path.clone())?;
            let has_h0 = session.inputs().iter().any(|i| i.name() == "h0");
            if inference_mode == InferenceMode::StatefulH0 && !has_h0 {
                return Err(DfError::Config(
                    "Model directory ends with _h0 but encoder has no h0 input".to_string(),
                ));
            }
            Ok((Sessions::Combined(session), inference_mode))
        };

        let (sessions, inference_mode) = match session_mode {
            SessionMode::SplitStreaming => load_split()?,
            SessionMode::CombinedStreaming => load_combined_streaming()?,
            SessionMode::Stateless => load_stateless()?,
            SessionMode::Auto => {
                if has_split {
                    load_split()?
                } else if has_combined_streaming {
                    load_combined_streaming()?
                } else if has_combined {
                    load_stateless()?
                } else {
                    return Err(DfError::Config(
                        "No model files found. Need either enc_conv_streaming.onnx+enc_gru_streaming.onnx+erb_dec.onnx+df_dec.onnx \
                         (split streaming), combined_streaming.onnx (combined streaming), or combined.onnx (stateless).".to_string(),
                    ));
                }
            }
        };

        // enc_window: how many frames of temporal context the encoder convolutions see.
        // Split streaming: use STATELESS_WINDOW for maximum conv context (only enc_conv
        //   processes T frames; GRU/decoders process T=1, so it's still fast).
        // Combined StatefulH0: use enc_kernel_t (minimum conv context). Larger windows
        //   slow down combined inference since the entire graph processes T frames.
        // StatelessWindowLast: large window lets the (resetting) GRU warm up each frame.
        let enc_window = match (&sessions, inference_mode) {
            (Sessions::Streaming { .. }, _) => STATELESS_WINDOW.max(enc_kernel_t),
            (_, InferenceMode::StatefulH0) => enc_kernel_t,
            (_, InferenceMode::StatelessWindowLast) => STATELESS_WINDOW.max(enc_kernel_t),
        };

        let mut df_state = DFState::new(SAMPLE_RATE, FFT_SIZE, HOP_SIZE, NB_ERB, min_nb_erb_freqs);
        df_state.init_norm_states(NB_DF);

        let buf_x_len = DF_ORDER.max(lookahead.max(1));
        let mut rolling_spec_buf_x = VecDeque::with_capacity(buf_x_len);
        for _ in 0..buf_x_len {
            rolling_spec_buf_x.push_back(vec![Complex32::new(0.0, 0.0); FREQ_SIZE]);
        }
        let mut rolling_spec_buf_y = VecDeque::with_capacity(DF_ORDER + lookahead);
        for _ in 0..(DF_ORDER + lookahead) {
            rolling_spec_buf_y.push_back(vec![Complex32::new(0.0, 0.0); FREQ_SIZE]);
        }
        let erb_feat_buf = VecDeque::with_capacity(lookahead + 1);
        let spec_feat_buf = VecDeque::with_capacity(lookahead + 1);
        // Pre-filled flat ring buffers (zeros = silent frames for padding)
        let enc_erb_ring = vec![0.0f32; enc_window * NB_ERB];
        let enc_spec_ring = vec![0.0f32; enc_window * NB_DF * 2];
        // Start at position enc_window-1 so next write goes to slot 0 (wrapping),
        // and count = enc_window-1 means first real frame brings us to enc_window
        let enc_ring_pos = enc_window.saturating_sub(1);
        let enc_ring_count = enc_window.saturating_sub(1);
        let enc_h = vec![0.0f32; enc_hidden_dim];
        let erb_dec_h = vec![0.0f32; erb_dec_num_layers * erb_dec_hidden_dim];
        let df_dec_h = vec![0.0f32; df_dec_num_layers * df_dec_hidden_dim];

        Ok(Self {
            sessions,
            df_state,
            rolling_spec_buf_x,
            rolling_spec_buf_y,
            erb_feat_buf,
            spec_feat_buf,
            enc_erb_ring,
            enc_spec_ring,
            enc_ring_pos,
            enc_ring_count,
            frames_processed: 0,
            lookahead,
            enc_h,
            enc_hidden_dim,
            enc_window,
            inference_mode,
            norm_alpha,
            variant,
            work_spec: vec![Complex32::new(0.0, 0.0); FREQ_SIZE],
            work_erb_feat: vec![0.0f32; NB_ERB],
            work_spec_feat_input: vec![Complex32::new(0.0, 0.0); NB_DF],
            work_spec_feat_cplx: vec![Complex32::new(0.0, 0.0); NB_DF],
            work_out_spec: vec![Complex32::new(0.0, 0.0); FREQ_SIZE],
            work_zeros: vec![0.0f32; NB_ERB],
            inf_erb_data: vec![0.0f32; enc_window * NB_ERB],
            inf_spec_data: vec![0.0f32; 2 * enc_window * NB_DF],
            inf_mask: vec![0.0f32; NB_ERB],
            inf_df_coefs: vec![0.0f32; NB_DF * DF_ORDER * 2],
            pre_gru_emb_dim,
            erb_dec_h,
            erb_dec_hidden_dim,
            erb_dec_num_layers,
            df_dec_h,
            df_dec_hidden_dim,
            df_dec_num_layers,
        })
    }

    pub fn variant(&self) -> ModelVariant {
        self.variant
    }

    /// Returns a human-readable name for the active inference mode.
    ///
    /// - `"split streaming"` — separate enc_conv/enc_gru/erb_dec/df_dec sessions
    /// - `"combined streaming"` — single combined_streaming.onnx with GRU states
    /// - `"stateless window"` — combined.onnx with 40-frame window warm-up
    pub fn inference_mode_name(&self) -> &'static str {
        match (&self.sessions, self.inference_mode) {
            (Sessions::Streaming { .. }, _) => "split streaming",
            (Sessions::Combined(_), InferenceMode::StatefulH0) => "combined streaming",
            (Sessions::Combined(_), InferenceMode::StatelessWindowLast) => "stateless window",
        }
    }

    /// Model lookahead in frames (from config.ini conv_lookahead).
    ///
    /// - `0`: LL (low-latency) model — causal, 10ms delay
    /// - `2`: Standard model — 2-frame lookahead, 30ms delay, best quality
    pub fn lookahead(&self) -> usize {
        self.lookahead
    }

    /// Algorithmic delay in samples.
    ///
    /// This is the inherent processing delay: STFT overlap + model lookahead.
    /// To get time-aligned output, trim this many samples from the start of the output.
    ///
    /// - LL models (lookahead=0): 480 samples (10ms)
    /// - Standard models (lookahead=2): 1440 samples (30ms)
    pub fn delay_samples(&self) -> usize {
        (FFT_SIZE - HOP_SIZE) + self.lookahead * HOP_SIZE
    }

    pub fn reset(&mut self) {
        self.df_state.reset();
        self.df_state.init_norm_states(NB_DF);
        self.frames_processed = 0;
        self.rolling_spec_buf_x.clear();
        let buf_x_len = DF_ORDER.max(self.lookahead.max(1));
        for _ in 0..buf_x_len {
            self.rolling_spec_buf_x
                .push_back(vec![Complex32::new(0.0, 0.0); FREQ_SIZE]);
        }
        self.rolling_spec_buf_y.clear();
        for _ in 0..(DF_ORDER + self.lookahead) {
            self.rolling_spec_buf_y
                .push_back(vec![Complex32::new(0.0, 0.0); FREQ_SIZE]);
        }
        self.erb_feat_buf.clear();
        self.spec_feat_buf.clear();
        self.enc_erb_ring.fill(0.0);
        self.enc_spec_ring.fill(0.0);
        self.enc_ring_pos = self.enc_window.saturating_sub(1);
        self.enc_ring_count = self.enc_window.saturating_sub(1);
        self.enc_h.fill(0.0);
        self.erb_dec_h.fill(0.0);
        self.df_dec_h.fill(0.0);
    }

    /// Perform warm-up inference to avoid cold-start latency
    ///
    /// NNAPI and other execution providers often have significant first-inference
    /// latency (100ms+). Call this during initialization to "wake up" the GPU/NPU.
    /// The processor state is reset after warmup.
    pub fn warmup(&mut self) -> Result<()> {
        let dummy_input = vec![0.0f32; HOP_SIZE];
        let mut dummy_output = vec![0.0f32; HOP_SIZE];

        // Run a few frames to fully warm up the pipeline
        for _ in 0..3 {
            self.process_frame(&dummy_input, &mut dummy_output)?;
        }

        // Reset state so warmup doesn't affect real processing
        self.reset();
        Ok(())
    }

    /// Process single frame (480 samples @ 48kHz = 10ms)
    pub fn process_frame(&mut self, input: &[f32], output: &mut [f32]) -> Result<()> {
        debug_assert_eq!(input.len(), HOP_SIZE);
        debug_assert_eq!(output.len(), HOP_SIZE);

        // 1. STFT (reuse preallocated buffer)
        for v in self.work_spec.iter_mut() {
            *v = Complex32::new(0.0, 0.0);
        }
        self.df_state.analysis(input, &mut self.work_spec);

        // 2. Store ORIGINAL spectrum in rolling buffers BEFORE any processing (for DF)
        self.rolling_spec_buf_x.pop_front();
        self.rolling_spec_buf_x.push_back(self.work_spec.clone());
        self.rolling_spec_buf_y.pop_front();
        self.rolling_spec_buf_y.push_back(self.work_spec.clone());

        // 3. ERB features (reuse preallocated buffer)
        self.work_erb_feat.fill(0.0);
        self.df_state
            .feat_erb(&self.work_spec, self.norm_alpha, &mut self.work_erb_feat);

        // 4. Spec features - use preallocated buffers
        self.work_spec_feat_input
            .copy_from_slice(&self.work_spec[..NB_DF]);
        for v in self.work_spec_feat_cplx.iter_mut() {
            *v = Complex32::new(0.0, 0.0);
        }
        self.df_state.feat_cplx(
            &self.work_spec_feat_input,
            self.norm_alpha,
            &mut self.work_spec_feat_cplx,
        );

        // 5. Buffer features and write to ring buffer.
        // Stateful streaming (split or merged combined_streaming): no lookahead delay —
        //   features go directly to the ring. The ring's pre-filled zeros provide the
        //   conv context that pad_feat gives in batch mode.
        // Stateless windowed: lookahead delay aligns features for the batch encoder.
        let direct_features = self.inference_mode == InferenceMode::StatefulH0;
        if direct_features {
            // Write current features directly to ring (no delay)
            let ring_slot = self.enc_ring_pos % self.enc_window;
            let erb_off = ring_slot * NB_ERB;
            self.enc_erb_ring[erb_off..erb_off + NB_ERB].copy_from_slice(&self.work_erb_feat);
            let spec_off = ring_slot * NB_DF * 2;
            for (fi, c) in self.work_spec_feat_cplx.iter().enumerate() {
                self.enc_spec_ring[spec_off + fi * 2] = c.re;
                self.enc_spec_ring[spec_off + fi * 2 + 1] = c.im;
            }
            self.enc_ring_pos = self.enc_ring_pos.wrapping_add(1);
            self.enc_ring_count = self.enc_ring_count.saturating_add(1).min(self.enc_window);
        } else {
            // Combined mode: buffer features with lookahead delay
            self.erb_feat_buf.push_back(self.work_erb_feat.clone());
            self.spec_feat_buf
                .push_back(self.work_spec_feat_cplx.clone());
            if self.erb_feat_buf.len() <= self.lookahead {
                self.frames_processed = self.frames_processed.saturating_add(1);
                for v in self.work_out_spec.iter_mut() {
                    *v = Complex32::new(0.0, 0.0);
                }
                self.df_state.synthesis(&mut self.work_out_spec, output);
                return Ok(());
            }

            let ring_slot = self.enc_ring_pos % self.enc_window;
            let erb_off = ring_slot * NB_ERB;
            self.enc_erb_ring[erb_off..erb_off + NB_ERB]
                .copy_from_slice(&self.erb_feat_buf[self.lookahead]);
            let spec_off = ring_slot * NB_DF * 2;
            for (fi, c) in self.spec_feat_buf[self.lookahead].iter().enumerate() {
                self.enc_spec_ring[spec_off + fi * 2] = c.re;
                self.enc_spec_ring[spec_off + fi * 2 + 1] = c.im;
            }
            self.enc_ring_pos = self.enc_ring_pos.wrapping_add(1);
            self.enc_ring_count = self.enc_ring_count.saturating_add(1).min(self.enc_window);

            if self.erb_feat_buf.len() > self.lookahead + 1 {
                self.erb_feat_buf.pop_front();
            }
            if self.spec_feat_buf.len() > self.lookahead + 1 {
                self.spec_feat_buf.pop_front();
            }
        }

        let min_required = self.inference_mode.min_required_frames(self.lookahead);
        if self.enc_ring_count < min_required {
            self.frames_processed = self.frames_processed.saturating_add(1);
            for v in self.work_out_spec.iter_mut() {
                *v = Complex32::new(0.0, 0.0);
            }
            self.df_state.synthesis(&mut self.work_out_spec, output);
            return Ok(());
        }

        // Build inference tensors from ring buffer (oldest-first order)
        let t = self.enc_ring_count;
        let oldest = self.enc_ring_pos.wrapping_sub(t) % self.enc_window;
        self.fill_inference_tensors(t, oldest);

        let lsnr = self.run_inference(t)?;

        // 6. Apply ERB mask.
        // Delay output by conv_lookahead frames to give the encoder conv access to
        // "future" features — matching batch-mode pad_feat behavior.
        let mask_idx = self
            .rolling_spec_buf_y
            .len()
            .saturating_sub(1 + self.lookahead);
        let apply_mask = lsnr <= MAX_DB_ERB_THRESH;
        let apply_zero_mask = lsnr < MIN_DB_THRESH;
        if let Some(center_spec) = self.rolling_spec_buf_y.get_mut(mask_idx) {
            if apply_mask {
                if apply_zero_mask {
                    self.df_state.apply_mask(center_spec, &self.work_zeros);
                } else {
                    self.df_state.apply_mask(center_spec, &self.inf_mask);
                }
            }
        }

        // 7. Output the selected frame
        self.frames_processed = self.frames_processed.saturating_add(1);
        if let Some(src) = self.rolling_spec_buf_y.get(mask_idx) {
            self.work_out_spec.copy_from_slice(src);
        } else {
            for v in self.work_out_spec.iter_mut() {
                *v = Complex32::new(0.0, 0.0);
            }
        }

        let apply_df = lsnr <= MAX_DB_DF_THRESH && !apply_zero_mask;
        if apply_df && self.rolling_spec_buf_x.len() >= DF_ORDER {
            Self::apply_deep_filter_flat(
                &mut self.work_out_spec,
                &self.rolling_spec_buf_x,
                &self.inf_df_coefs,
            );
        }

        // 8. ISTFT
        self.df_state.synthesis(&mut self.work_out_spec, output);

        Ok(())
    }

    /// Fill pre-allocated inference tensor buffers from the ring buffer in oldest-first order.
    fn fill_inference_tensors(&mut self, t: usize, oldest_slot: usize) {
        // ERB tensor data: [1, 1, T, NB_ERB] flattened
        for ti in 0..t {
            let slot = (oldest_slot + ti) % self.enc_window;
            let src_off = slot * NB_ERB;
            let dst_off = ti * NB_ERB;
            self.inf_erb_data[dst_off..dst_off + NB_ERB]
                .copy_from_slice(&self.enc_erb_ring[src_off..src_off + NB_ERB]);
        }

        // Spec tensor data: [1, 2, T, NB_DF] = [real channel, imag channel]
        // Ring stores interleaved re/im per freq, we need planar layout
        let ch1_offset = t * NB_DF;
        for ti in 0..t {
            let slot = (oldest_slot + ti) % self.enc_window;
            let src_off = slot * NB_DF * 2;
            let re_off = ti * NB_DF;
            let im_off = ch1_offset + ti * NB_DF;
            for fi in 0..NB_DF {
                self.inf_spec_data[re_off + fi] = self.enc_spec_ring[src_off + fi * 2];
                self.inf_spec_data[im_off + fi] = self.enc_spec_ring[src_off + fi * 2 + 1];
            }
        }
    }

    /// Run inference using pre-filled inf_erb_data/inf_spec_data buffers.
    /// Writes results into inf_mask and inf_df_coefs. Returns lsnr.
    fn run_inference(&mut self, t: usize) -> Result<f32> {
        // Destructure self to get disjoint mutable borrows (sessions + state fields)
        let Self {
            sessions,
            enc_h,
            enc_hidden_dim,
            pre_gru_emb_dim,
            inference_mode,
            erb_dec_h,
            erb_dec_num_layers,
            erb_dec_hidden_dim,
            df_dec_h,
            df_dec_num_layers,
            df_dec_hidden_dim,
            inf_erb_data,
            inf_spec_data,
            inf_mask,
            inf_df_coefs,
            ..
        } = self;

        let erb_len = t * NB_ERB;
        let spec_len = 2 * t * NB_DF;
        let erb_tensor =
            Tensor::from_array(([1usize, 1, t, NB_ERB], inf_erb_data[..erb_len].to_vec()))?;
        let spec_tensor =
            Tensor::from_array(([1usize, 2, t, NB_DF], inf_spec_data[..spec_len].to_vec()))?;

        match sessions {
            Sessions::Combined(session) => {
                // --- Windowed batch inference (existing approach) ---
                // For StatefulH0 (patched streaming model), outputs are time-sliced
                // to T=1 inside the ONNX graph, so always take index 0.
                // For StatelessWindowLast, outputs have T=enc_window frames.
                let take_idx = if *inference_mode == InferenceMode::StatefulH0 {
                    0
                } else {
                    t.saturating_sub(1)
                };

                let outputs = if *inference_mode == InferenceMode::StatefulH0 {
                    let h0_tensor =
                        Tensor::from_array(([1usize, 1, *enc_hidden_dim], enc_h.clone()))?;
                    let has_erb_h0 = session.inputs().iter().any(|i| i.name() == "erb_h0");
                    let has_df_h0 = session.inputs().iter().any(|i| i.name() == "df_h0");
                    if has_erb_h0 && has_df_h0 {
                        let erb_h0_tensor = Tensor::from_array((
                            [*erb_dec_num_layers, 1, *erb_dec_hidden_dim],
                            erb_dec_h.clone(),
                        ))?;
                        let df_h0_tensor = Tensor::from_array((
                            [*df_dec_num_layers, 1, *df_dec_hidden_dim],
                            df_dec_h.clone(),
                        ))?;
                        session.run(ort::inputs![
                            "feat_erb" => erb_tensor,
                            "feat_spec" => spec_tensor,
                            "h0" => h0_tensor,
                            "erb_h0" => erb_h0_tensor,
                            "df_h0" => df_h0_tensor,
                        ])?
                    } else {
                        session.run(ort::inputs![
                            "feat_erb" => erb_tensor,
                            "feat_spec" => spec_tensor,
                            "h0" => h0_tensor,
                        ])?
                    }
                } else {
                    session.run(ort::inputs![
                        "feat_erb" => erb_tensor,
                        "feat_spec" => spec_tensor,
                    ])?
                };

                let (_lsnr_shape, lsnr_data) = outputs["lsnr"].try_extract_tensor::<f32>()?;
                let lsnr = lsnr_data.get(take_idx).copied().unwrap_or(0.0);

                if *inference_mode == InferenceMode::StatefulH0 {
                    let (_h1_shape, h1_data) = outputs["h1"].try_extract_tensor::<f32>()?;
                    if h1_data.len() >= *enc_hidden_dim {
                        enc_h.copy_from_slice(&h1_data[..*enc_hidden_dim]);
                    }
                    if let Some(val) = outputs.get("erb_h1") {
                        if let Ok((_shape, data)) = val.try_extract_tensor::<f32>() {
                            let n = erb_dec_h.len().min(data.len());
                            erb_dec_h[..n].copy_from_slice(&data[..n]);
                        }
                    }
                    if let Some(val) = outputs.get("df_h1") {
                        if let Ok((_shape, data)) = val.try_extract_tensor::<f32>() {
                            let n = df_dec_h.len().min(data.len());
                            df_dec_h[..n].copy_from_slice(&data[..n]);
                        }
                    }
                }

                let (mask_shape, mask_data) = outputs["m"].try_extract_tensor::<f32>()?;
                let mask_t = mask_shape[2] as usize;
                let mask_offset = take_idx.min(mask_t.saturating_sub(1)) * NB_ERB;
                inf_mask.copy_from_slice(&mask_data[mask_offset..mask_offset + NB_ERB]);

                let (coefs_shape, coefs_data) = outputs["coefs"].try_extract_tensor::<f32>()?;
                let coefs_t = coefs_shape[1] as usize;
                let coefs_frame_size = NB_DF * DF_ORDER * 2;
                let coefs_offset = take_idx.min(coefs_t.saturating_sub(1)) * coefs_frame_size;
                inf_df_coefs
                    .copy_from_slice(&coefs_data[coefs_offset..coefs_offset + coefs_frame_size]);

                Ok(lsnr)
            }

            Sessions::Streaming {
                enc_conv,
                enc_gru,
                erb_dec,
                df_dec,
            } => {
                // --- Streaming inference (true frame-by-frame) ---
                // Each GRU processes exactly 1 frame per call with persistent state.

                // Step 1: Encoder convolutions with T=t frames for context.
                // Output is T=1 (last frame only, sliced in the ONNX model).
                let conv_out = enc_conv.run(ort::inputs![
                    "feat_erb" => erb_tensor,
                    "feat_spec" => spec_tensor,
                ])?;

                // Extract pre-GRU embedding [1, 1, emb_dim]
                let (emb_shape, emb_data) = conv_out["emb"].try_extract_tensor::<f32>()?;
                let emb_h = *emb_shape.last().unwrap_or(&(*pre_gru_emb_dim as i64)) as usize;
                let emb_vec: Vec<f32> = emb_data[..emb_h].to_vec();

                // Step 2: Encoder GRU with single frame + hidden state.
                let emb_gru_in = Tensor::from_array(([1usize, 1, emb_h], emb_vec))?;
                let h0_tensor = Tensor::from_array(([1usize, 1, *enc_hidden_dim], enc_h.clone()))?;
                let gru_out = enc_gru.run(ort::inputs![
                    "emb" => emb_gru_in,
                    "h0" => h0_tensor,
                ])?;

                let (_lsnr_shape, lsnr_data) = gru_out["lsnr"].try_extract_tensor::<f32>()?;
                let lsnr = lsnr_data[0];

                let (_h1_shape, h1_data) = gru_out["h1"].try_extract_tensor::<f32>()?;
                enc_h.copy_from_slice(&h1_data[..*enc_hidden_dim]);

                let (emb_out_shape, emb_out_data) =
                    gru_out["emb_out"].try_extract_tensor::<f32>()?;
                let emb_out_h = *emb_out_shape.last().unwrap_or(&(emb_h as i64)) as usize;
                let emb_out_vec: Vec<f32> = emb_out_data[..emb_out_h].to_vec();

                // Step 3: Extract skip connections from conv output (all T=1)
                let to_shape =
                    |s: &[i64]| -> Vec<usize> { s.iter().map(|&d| d as usize).collect() };

                let (e0_s, e0_d) = conv_out["e0"].try_extract_tensor::<f32>()?;
                let (e1_s, e1_d) = conv_out["e1"].try_extract_tensor::<f32>()?;
                let (e2_s, e2_d) = conv_out["e2"].try_extract_tensor::<f32>()?;
                let (e3_s, e3_d) = conv_out["e3"].try_extract_tensor::<f32>()?;
                let (c0_s, c0_d) = conv_out["c0"].try_extract_tensor::<f32>()?;

                let e0_t = Tensor::from_array((to_shape(&e0_s), e0_d.to_vec()))?;
                let e1_t = Tensor::from_array((to_shape(&e1_s), e1_d.to_vec()))?;
                let e2_t = Tensor::from_array((to_shape(&e2_s), e2_d.to_vec()))?;
                let e3_t = Tensor::from_array((to_shape(&e3_s), e3_d.to_vec()))?;
                let c0_t = Tensor::from_array((to_shape(&c0_s), c0_d.to_vec()))?;

                // Step 4: ERB decoder with single frame + hidden state.
                let emb_erb_tensor =
                    Tensor::from_array(([1usize, 1, emb_out_h], emb_out_vec.clone()))?;
                let erb_h0_tensor = Tensor::from_array((
                    [*erb_dec_num_layers, 1, *erb_dec_hidden_dim],
                    erb_dec_h.clone(),
                ))?;
                let erb_out = erb_dec.run(ort::inputs![
                    "emb" => emb_erb_tensor,
                    "e3" => e3_t,
                    "e2" => e2_t,
                    "e1" => e1_t,
                    "e0" => e0_t,
                    "erb_h0" => erb_h0_tensor,
                ])?;

                let (_mask_shape, mask_data) = erb_out["m"].try_extract_tensor::<f32>()?;
                inf_mask.copy_from_slice(&mask_data[..NB_ERB]);

                if let Ok((_shape, data)) = erb_out["erb_h1"].try_extract_tensor::<f32>() {
                    let n = erb_dec_h.len().min(data.len());
                    erb_dec_h[..n].copy_from_slice(&data[..n]);
                }

                // Step 5: DF decoder with single frame + hidden state.
                let emb_df_tensor = Tensor::from_array(([1usize, 1, emb_out_h], emb_out_vec))?;
                let df_h0_tensor = Tensor::from_array((
                    [*df_dec_num_layers, 1, *df_dec_hidden_dim],
                    df_dec_h.clone(),
                ))?;
                let df_out = df_dec.run(ort::inputs![
                    "emb" => emb_df_tensor,
                    "c0" => c0_t,
                    "df_h0" => df_h0_tensor,
                ])?;

                let (_coefs_shape, coefs_data) = df_out["coefs"].try_extract_tensor::<f32>()?;
                let coefs_frame_size = NB_DF * DF_ORDER * 2;
                inf_df_coefs.copy_from_slice(&coefs_data[..coefs_frame_size]);

                if let Ok((_shape, data)) = df_out["df_h1"].try_extract_tensor::<f32>() {
                    let n = df_dec_h.len().min(data.len());
                    df_dec_h[..n].copy_from_slice(&data[..n]);
                }

                Ok(lsnr)
            }
        }
    }

    /// Apply deep filtering using flat coefs slice [NB_DF * DF_ORDER * 2].
    /// Layout: [freq][order * 2] where each pair is (re, im).
    fn apply_deep_filter_flat(
        spec: &mut [Complex32],
        rolling_spec_buf_x: &VecDeque<Vec<Complex32>>,
        df_coefs: &[f32],
    ) {
        debug_assert!(rolling_spec_buf_x.len() >= DF_ORDER);
        debug_assert_eq!(df_coefs.len(), NB_DF * DF_ORDER * 2);

        for freq in 0..NB_DF {
            let mut re = 0.0f32;
            let mut im = 0.0f32;
            let coef_base = freq * DF_ORDER * 2;

            for (order, frame) in rolling_spec_buf_x.iter().take(DF_ORDER).enumerate() {
                let s = frame[freq];
                let cr = df_coefs[coef_base + order * 2];
                let ci = df_coefs[coef_base + order * 2 + 1];
                // Complex multiply: (s.re + s.im*i) * (cr + ci*i)
                re += s.re * cr - s.im * ci;
                im += s.re * ci + s.im * cr;
            }

            spec[freq] = Complex32::new(re, im);
        }
    }
}

/// High-level streaming API with automatic buffering.
///
/// This is the recommended API for most use cases. Pass any length of audio,
/// get enhanced audio back. Handles frame buffering internally.
///
/// ```ignore
/// let mut stream = DeepFilterStream::new(Path::new("models/dfn3_h0"))?;
/// stream.warmup()?;
///
/// // Process any chunk size - buffering is handled internally
/// let enhanced = stream.process(&audio_chunk)?;
/// let remaining = stream.flush()?; // Get any buffered samples at end
/// ```
pub struct DeepFilterStream {
    processor: DeepFilterProcessor,
    input_buffer: Vec<f32>,
}

impl DeepFilterStream {
    /// Create stream from model directory.
    ///
    /// Auto-detects model variant from folder name and config.ini.
    pub fn new(model_dir: &Path) -> Result<Self> {
        Ok(Self {
            processor: DeepFilterProcessor::new(model_dir)?,
            input_buffer: Vec::with_capacity(HOP_SIZE * 2),
        })
    }

    /// Create stream with explicit thread count.
    ///
    /// Controls ONNX Runtime's intra-op parallelism:
    /// - For real-time audio: use 1-2 to minimize latency variance
    /// - For batch/offline: use 4+ for throughput
    pub fn with_threads(model_dir: &Path, intra_threads: usize) -> Result<Self> {
        Ok(Self {
            processor: DeepFilterProcessor::with_threads(model_dir, intra_threads)?,
            input_buffer: Vec::with_capacity(HOP_SIZE * 2),
        })
    }

    /// Create stream with explicit session mode.
    ///
    /// Use [`SessionMode::CombinedStreaming`] or [`SessionMode::Stateless`] to force
    /// a specific inference mode.
    pub fn with_mode(
        model_dir: &Path,
        session_mode: SessionMode,
        intra_threads: Option<usize>,
    ) -> Result<Self> {
        Ok(Self {
            processor: DeepFilterProcessor::with_mode(model_dir, session_mode, intra_threads)?,
            input_buffer: Vec::with_capacity(HOP_SIZE * 2),
        })
    }

    /// Create stream with explicit variant and thread count.
    ///
    /// Use when folder name doesn't match model or you need to override detection.
    pub fn with_variant_and_threads(
        model_dir: &Path,
        variant: ModelVariant,
        intra_threads: usize,
    ) -> Result<Self> {
        Ok(Self {
            processor: DeepFilterProcessor::with_variant_and_threads(
                model_dir,
                variant,
                Some(intra_threads),
            )?,
            input_buffer: Vec::with_capacity(HOP_SIZE * 2),
        })
    }

    /// Warm up the inference engine.
    ///
    /// Call once after construction to avoid cold-start latency on first real audio.
    /// Especially important for GPU/NPU backends (NNAPI, CoreML, CUDA).
    pub fn warmup(&mut self) -> Result<()> {
        self.processor.warmup()
    }

    /// Process audio samples.
    ///
    /// Input: 48kHz mono f32 samples, any length.
    /// Output: Enhanced samples. May be shorter than input due to internal buffering.
    /// Call [`flush`](Self::flush) at end of stream to get remaining samples.
    pub fn process(&mut self, input: &[f32]) -> Result<Vec<f32>> {
        self.input_buffer.extend_from_slice(input);
        let mut output = Vec::new();

        while self.input_buffer.len() >= HOP_SIZE {
            let frame_in: Vec<f32> = self.input_buffer.drain(..HOP_SIZE).collect();
            let mut frame_out = vec![0.0f32; HOP_SIZE];
            self.processor.process_frame(&frame_in, &mut frame_out)?;
            output.extend_from_slice(&frame_out);
        }

        Ok(output)
    }

    /// Flush remaining buffered samples.
    ///
    /// Call at end of stream to get any samples still in the internal buffer.
    /// Pads with zeros if needed to complete the final frame.
    pub fn flush(&mut self) -> Result<Vec<f32>> {
        if self.input_buffer.is_empty() {
            return Ok(Vec::new());
        }
        let valid_len = self.input_buffer.len();
        self.input_buffer.resize(HOP_SIZE, 0.0);
        let frame_in: Vec<f32> = self.input_buffer.drain(..).collect();
        let mut frame_out = vec![0.0f32; HOP_SIZE];
        self.processor.process_frame(&frame_in, &mut frame_out)?;
        Ok(frame_out[..valid_len].to_vec())
    }

    /// Reset processor state and clear buffers.
    ///
    /// Call between separate audio streams to avoid artifacts from previous audio.
    pub fn reset(&mut self) {
        self.processor.reset();
        self.input_buffer.clear();
    }

    /// Get the detected model variant.
    pub fn variant(&self) -> ModelVariant {
        self.processor.variant()
    }

    /// Required sample rate (48000 Hz).
    pub fn sample_rate(&self) -> usize {
        SAMPLE_RATE
    }

    /// Algorithmic latency in milliseconds.
    ///
    /// - LL models: 10ms
    /// - Standard models: 30ms
    pub fn latency_ms(&self) -> f32 {
        self.processor.delay_samples() as f32 / SAMPLE_RATE as f32 * 1000.0
    }

    /// Algorithmic delay in samples.
    ///
    /// To get time-aligned output (matching Python's `pad=True` behavior or the
    /// Tract CLI's `-D` flag), trim this many samples from the start of the output.
    pub fn delay_samples(&self) -> usize {
        self.processor.delay_samples()
    }

    /// Model lookahead in frames (0 for LL, 2 for standard).
    pub fn lookahead(&self) -> usize {
        self.processor.lookahead()
    }

    /// Returns a human-readable name for the active inference mode.
    pub fn inference_mode_name(&self) -> &'static str {
        self.processor.inference_mode_name()
    }

    /// Access the underlying processor for advanced use.
    pub fn processor_mut(&mut self) -> &mut DeepFilterProcessor {
        &mut self.processor
    }
}

// Verify that processors can be sent between threads
// This is a compile-time check - if it compiles, the types are Send
fn _assert_send<T: Send>() {}
fn _assert_processor_is_send() {
    _assert_send::<DeepFilterProcessor>();
    _assert_send::<DeepFilterStream>();
}
