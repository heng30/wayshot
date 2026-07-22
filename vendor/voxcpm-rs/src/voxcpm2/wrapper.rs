//! High-level convenience wrapper around [`VoxCpm2Model`] that exposes a
//! Python-SDK-style `generate()` API.

use crate::tokenizer::TextTokenizer;
use crate::voxcpm2::model::{
    AUDIO_START_TOKEN, REF_AUDIO_END_TOKEN, REF_AUDIO_START_TOKEN, VoxCpm2Model,
};
use crate::VoxCpm2Config;
use burn::prelude::*;
use burn::tensor::{Int, TensorData};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Cooperative cancellation handle for [`VoxCPM::generate`].
///
/// Cheap to clone (`Arc<AtomicBool>` underneath). Signal cancellation from
/// any thread by calling [`CancelToken::cancel`]; the in-flight `generate`
/// call will check between autoregressive steps and bail with
/// [`crate::Error::Cancelled`]. Cancel latency is bounded by one diffusion
/// step (~200 ms on `wgpu` at default `timesteps=10`).
///
/// ```no_run
/// use std::thread;
/// use voxcpm_rs::{CancelToken, GenerateOptions, Prompt, VoxCPM};
/// # type B = burn::backend::NdArray<f32>;
/// # let model: VoxCPM<B> = unimplemented!();
/// let cancel = CancelToken::new();
///
/// // Cancel from a watchdog thread after 2 s.
/// {
///     let cancel = cancel.clone();
///     thread::spawn(move || {
///         thread::sleep(std::time::Duration::from_secs(2));
///         cancel.cancel();
///     });
/// }
///
/// let opts = GenerateOptions::builder().cancel(cancel).build();
/// match model.generate("a very long text...", opts) {
///     Ok(wav) => { /* completed */ }
///     Err(voxcpm_rs::Error::Cancelled) => { /* user cancelled */ }
///     Err(e) => return Err(e.into()),
/// }
/// # Ok::<_, Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    /// Create a new, un-cancelled token.
    pub fn new() -> Self {
        Self::default()
    }

    /// Signal cancellation. Idempotent; safe to call from any thread.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    /// Whether [`Self::cancel`] has been called.
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

/// Source of prompt audio for [`Prompt::Reference`], [`Prompt::Continuation`]
/// and [`Prompt::Combined`].
///
/// Supports three input modes — pick whichever fits your pipeline:
/// - [`PromptAudio::File`] — path to an encoded audio file (WAV/FLAC/MP3/...).
/// - [`PromptAudio::Encoded`] — encoded audio bytes already in memory
///   (same format support as `File`, just sourced from a `Vec<u8>`).
/// - [`PromptAudio::Pcm`] — raw mono `f32` samples and their sample rate.
///   Use this when you already have decoded audio (e.g. from a microphone
///   capture, an in-process resampler, or a TTS chain).
///
/// `From<PathBuf>` / `From<&Path>` / `From<&str>` are implemented for
/// ergonomics, so paths can be passed directly without wrapping.
#[derive(Debug, Clone)]
pub enum PromptAudio {
    /// Decode an audio file from disk.
    File(PathBuf),
    /// Decode an encoded audio buffer (any format Symphonia supports).
    Encoded(Vec<u8>),
    /// Use already-decoded mono `f32` samples at the given sample rate.
    Pcm {
        /// Mono PCM samples in `[-1.0, 1.0]`.
        samples: Vec<f32>,
        /// Sample rate of `samples` in Hz.
        sample_rate: u32,
    },
}

impl From<PathBuf> for PromptAudio {
    fn from(p: PathBuf) -> Self {
        PromptAudio::File(p)
    }
}
impl From<&Path> for PromptAudio {
    fn from(p: &Path) -> Self {
        PromptAudio::File(p.to_path_buf())
    }
}
impl From<&str> for PromptAudio {
    fn from(p: &str) -> Self {
        PromptAudio::File(PathBuf::from(p))
    }
}

/// How the model should be conditioned on prompt audio.
///
/// See [`VoxCPM::generate`] for what each mode does conceptually.
#[derive(Debug, Clone, Default)]
pub enum Prompt {
    /// No prompt audio — the model improvises a voice.
    #[default]
    None,
    /// Voice cloning via a structurally isolated reference audio prefix.
    /// No transcript required; the audio is bracketed by `[REF_AUDIO_*]` tokens.
    Reference {
        /// Audio of the speaker to clone. See [`PromptAudio`].
        audio: PromptAudio,
    },
    /// In-context continuation: the model literally finishes an utterance
    /// whose start is `audio` (transcribed by `text`).
    Continuation {
        /// Audio containing the start of the utterance. See [`PromptAudio`].
        audio: PromptAudio,
        /// Transcript of `audio` — prepended to the target text before tokenization.
        text: String,
    },
    /// Reference prefix *and* continuation suffix in the same sequence.
    /// Useful when continuation alone drifts off the speaker.
    Combined {
        /// Reference audio (prefix, isolated by `[REF_AUDIO_*]` tokens).
        reference_audio: PromptAudio,
        /// Continuation audio (suffix, autoregression starts from its end).
        prompt_audio: PromptAudio,
        /// Transcript of `prompt_audio`.
        prompt_text: String,
    },
}

/// Sampling / decoding knobs for [`VoxCPM::generate`].
#[derive(Debug, Clone)]
pub struct GenerateOptions {
    /// Classifier-free guidance scale. Higher → closer to the conditioning
    /// (text + prompt), lower → more diverse. Typical range: `1.5..=3.0`.
    pub cfg_value: f32,
    /// Number of Euler steps in the diffusion sampler. Linear cost; lower is
    /// faster but quality drops below ~6.
    pub inference_timesteps: usize,
    /// Minimum number of latent patches to generate before the stop head is
    /// allowed to fire. Guards against immediate cutoffs on very short text.
    pub min_len: usize,
    /// Hard upper bound on the number of latent patches per call. Each patch
    /// is `patch_size * chunk_size / sample_rate` seconds of audio (~80 ms).
    pub max_len: usize,
    /// Prompt-conditioning mode. See [`Prompt`].
    pub prompt: Prompt,
    /// Optional [`CancelToken`] for cooperative cancellation. When `Some`,
    /// the autoregressive loop checks the token between every step and
    /// returns [`crate::Error::Cancelled`] if it has been signalled.
    pub cancel: Option<CancelToken>,
    /// Streaming-only: number of latent patches to accumulate per emitted
    /// audio chunk in [`VoxCPM::generate_stream`]. Smaller = lower
    /// per-chunk latency but more redundant VAE-decode work; larger = fewer
    /// chunks, more samples per chunk. Ignored by the non-streaming
    /// [`VoxCPM::generate`] path. Default: `5` (~400 ms / chunk @ default
    /// model config).
    pub chunk_patches: usize,
    /// **Opt-in parallel-segment generation.** When `Some(N)` and the input
    /// text contains multiple sentences, [`VoxCPM::generate`] splits the
    /// text on sentence boundaries and decodes up to `N` segments in a
    /// single batched forward pass. On a launch-bound GPU (most consumer
    /// cards at batch=1 / seq=1 decode) this yields near-`N`× throughput.
    ///
    /// Voice consistency:
    /// - With [`Prompt::Reference`]: the user's reference is used for every
    ///   segment — voice is consistent across the whole output.
    /// - With [`Prompt::None`]: the *first* segment is generated serially
    ///   to establish a voice, then its audio is used as a reference for
    ///   the remaining segments (which are decoded in batched groups).
    /// - With [`Prompt::Continuation`] / [`Prompt::Combined`]: parallel
    ///   mode is silently disabled and the call falls back to the
    ///   single-segment serial path.
    ///
    /// `None` (the default) preserves the original behaviour exactly. Set
    /// to `Some(2)` for the conservative 2× sweet spot, `Some(4)` for
    /// aggressive throughput on a strong GPU, or `Some(8)` to push as far
    /// as the launch-bound regime allows.
    pub parallel_segments: Option<usize>,
}

impl Default for GenerateOptions {
    fn default() -> Self {
        Self {
            cfg_value: 2.0,
            inference_timesteps: 10,
            min_len: 2,
            max_len: 2000,
            prompt: Prompt::None,
            cancel: None,
            chunk_patches: 5,
            parallel_segments: None,
        }
    }
}

impl GenerateOptions {
    /// Start a fluent builder for `GenerateOptions`.
    ///
    /// ```no_run
    /// use voxcpm_rs::{GenerateOptions, Prompt};
    /// let opts = GenerateOptions::builder()
    ///     .timesteps(8)
    ///     .cfg(2.0)
    ///     .prompt(Prompt::Reference { audio: "speaker.wav".into() })
    ///     .build();
    /// ```
    pub fn builder() -> GenerateOptionsBuilder {
        GenerateOptionsBuilder::default()
    }
}

/// Fluent builder for [`GenerateOptions`]. Created via
/// [`GenerateOptions::builder`].
#[derive(Debug, Clone, Default)]
pub struct GenerateOptionsBuilder {
    inner: GenerateOptions,
}

impl GenerateOptionsBuilder {
    /// Set [`GenerateOptions::cfg_value`].
    pub fn cfg(mut self, v: f32) -> Self {
        self.inner.cfg_value = v;
        self
    }
    /// Set [`GenerateOptions::inference_timesteps`].
    pub fn timesteps(mut self, n: usize) -> Self {
        self.inner.inference_timesteps = n;
        self
    }
    /// Set [`GenerateOptions::min_len`].
    pub fn min_len(mut self, n: usize) -> Self {
        self.inner.min_len = n;
        self
    }
    /// Set [`GenerateOptions::max_len`].
    pub fn max_len(mut self, n: usize) -> Self {
        self.inner.max_len = n;
        self
    }
    /// Set [`GenerateOptions::prompt`].
    pub fn prompt(mut self, p: Prompt) -> Self {
        self.inner.prompt = p;
        self
    }
    /// Set [`GenerateOptions::cancel`].
    pub fn cancel(mut self, token: CancelToken) -> Self {
        self.inner.cancel = Some(token);
        self
    }
    /// Set [`GenerateOptions::chunk_patches`] (only used by
    /// [`VoxCPM::generate_stream`]).
    pub fn chunk_patches(mut self, n: usize) -> Self {
        self.inner.chunk_patches = n;
        self
    }
    /// Enable opt-in parallel-segment generation with batch size `n`.
    /// See [`GenerateOptions::parallel_segments`].
    pub fn parallel_segments(mut self, n: usize) -> Self {
        self.inner.parallel_segments = Some(n);
        self
    }
    /// Finalize into a [`GenerateOptions`].
    pub fn build(self) -> GenerateOptions {
        self.inner
    }
}

#[derive(Debug)]
pub struct VoxCPM<B: Backend> {
    pub model: VoxCpm2Model<B>,
    pub tokenizer: TextTokenizer,
    device: B::Device,
}

impl<B: Backend> VoxCPM<B> {
    /// Construct a fresh (randomly-initialized) model from a
    /// [`VoxCpm2Config`][crate::VoxCpm2Config]. For inference you almost
    /// certainly want [`Self::from_local`] instead.
    pub fn from_config(
        config: VoxCpm2Config,
        tokenizer: TextTokenizer,
        device: &B::Device,
    ) -> Self {
        Self {
            model: VoxCpm2Model::new(config, device),
            tokenizer,
            device: device.clone(),
        }
    }

    /// Load a pretrained VoxCPM2 checkpoint from a local directory.
    ///
    /// The directory is expected to contain:
    /// - `config.json` — a [`VoxCpm2Config`] JSON.
    /// - `tokenizer.json` — a HuggingFace `tokenizers` file.
    /// - `model.safetensors` — the main model weights.
    /// - `audiovae.safetensors` — the AudioVAE weights.
    ///
    /// Weight-load progress is reported through the [`log`] crate (`info` for
    /// the summary, `warn` for missing/unused tensors, `error` for load
    /// errors). Wire up `env_logger`, `tracing-log`, etc. to surface them.
    pub fn from_local(path: impl AsRef<Path>, device: &B::Device) -> crate::Result<Self> {
        let path = path.as_ref();
        let config_bytes = std::fs::read_to_string(path.join("config.json"))?;
        let config: VoxCpm2Config = serde_json::from_str(&config_bytes)?;
        let tokenizer = TextTokenizer::from_local(path)?;
        let mut model = VoxCpm2Model::<B>::new(config, device);
        let result = crate::weights::load_pretrained(&mut model, path)?;
        log::info!(
            "weights loaded — applied={}, skipped={}, missing={}, unused={}, errors={}",
            result.applied.len(),
            result.skipped.len(),
            result.missing.len(),
            result.unused.len(),
            result.errors.len(),
        );
        if !result.missing.is_empty() {
            log::warn!("missing module params (first 20):");
            for (k, ctx) in result.missing.iter().take(20) {
                log::warn!("  {k} [{ctx}]");
            }
        }
        if !result.unused.is_empty() {
            log::warn!("unused checkpoint tensors (first 20):");
            for k in result.unused.iter().take(20) {
                log::warn!("  {k}");
            }
        }
        if !result.errors.is_empty() {
            log::error!("load errors (first 20):");
            for e in result.errors.iter().take(20) {
                log::error!("  {e:?}");
            }
        }
        Ok(Self {
            model,
            tokenizer,
            device: device.clone(),
        })
    }

    pub fn sample_rate(&self) -> u32 {
        self.model.sample_rate() as u32
    }

    /// Decode a pre-computed latent `[1, D, T]` through the AudioVAE and
    /// return the waveform tensor `[1, 1, T_out]`.
    pub fn audio_vae_decode(&self, feat: Tensor<B, 3>) -> Tensor<B, 3> {
        self.model.audio_vae.decode(feat)
    }

    /// Padding mode for prompt audio prior to VAE encoding.
    /// - `Right`: pad zeros at the end (used for reference audio in voice cloning).
    /// - `Left`: pad zeros at the start so the *valid* audio sits at the end of
    ///   the sequence (used for continuation prompts).
    fn encode_prompt_audio(
        &self,
        audio: &PromptAudio,
        padding_mode: PadMode,
    ) -> crate::Result<Tensor<B, 3>> {
        let encoder_sr = self.model.audio_vae.sample_rate() as u32;
        let mut samples = match audio {
            PromptAudio::File(path) => crate::audio::load_audio_as(path, encoder_sr)?,
            PromptAudio::Encoded(bytes) => crate::audio::load_audio_bytes_as(bytes, encoder_sr)?,
            PromptAudio::Pcm { samples, sample_rate } => {
                crate::audio::resample(samples, *sample_rate, encoder_sr)?
            }
        };
        let p = self.model.patch_size();
        let chunk = self.model.audio_vae.config.chunk_size();
        let patch_len = p * chunk;
        let n = samples.len();
        if n == 0 {
            return Err(crate::Error::AudioDecode(
                "prompt audio decoded to 0 samples".into(),
            ));
        }
        let rem = n % patch_len;
        if rem != 0 {
            let pad = patch_len - rem;
            match padding_mode {
                PadMode::Right => samples.resize(n + pad, 0.0),
                PadMode::Left => {
                    let mut new = vec![0.0f32; pad];
                    new.extend_from_slice(&samples);
                    samples = new;
                }
            }
        }
        let n_padded = samples.len();
        let audio: Tensor<B, 3> =
            Tensor::from_data(TensorData::new(samples, [1, 1, n_padded]), &self.device);
        let feat = self.model.audio_vae.encode(audio); // [1, D, T*P]
        let [_, d, tp] = feat.dims();
        debug_assert_eq!(tp % p, 0);
        let t = tp / p;
        // [1, D, T*P] -> [D, T, P] -> [T, P, D]
        let feat: Tensor<B, 3> = feat.reshape([d, t, p]);
        let feat = feat.swap_dims(0, 1).swap_dims(1, 2);
        Ok(feat)
    }

    /// Generate an audio waveform (mono `f32` samples at [`Self::sample_rate`]).
    ///
    /// The generation mode is selected by [`GenerateOptions::prompt`]:
    /// - [`Prompt::None`] — zero-shot, model improvises a voice.
    /// - [`Prompt::Reference`] — voice cloning via a structurally isolated
    ///   reference audio prefix (no transcript required).
    /// - [`Prompt::Continuation`] — model continues from the prompt audio in
    ///   the same speaker's voice. The prompt's transcript is prepended to
    ///   `text`.
    /// - [`Prompt::Combined`] — both a reference prefix and a continuation
    ///   suffix.
    pub fn generate(&self, text: &str, opts: GenerateOptions) -> crate::Result<Vec<f32>> {
        // Parallel-segment fast path: opt-in via `parallel_segments`.
        // Falls back to serial below if any precondition isn't met.
        if let Some(parallel_n) = opts.parallel_segments {
            if parallel_n >= 2 && matches!(opts.prompt, Prompt::None | Prompt::Reference { .. }) {
                let segments = split_sentences(text);
                if segments.len() >= 2 {
                    return self.generate_parallel(&segments, parallel_n, &opts);
                }
            }
        }

        let inputs = self.build_inference_inputs(text, &opts.prompt)?;

        // Wrap the cancel token (if any) into a `dyn Fn() -> bool` so the
        // model layer doesn't need to know about `CancelToken` directly.
        let cancel_fn: Option<Box<dyn Fn() -> bool>> = opts.cancel.as_ref().map(|c| {
            let c = c.clone();
            Box::new(move || c.is_cancelled()) as Box<dyn Fn() -> bool>
        });
        let (latent, _stop_steps) = self.model.inference(
            inputs.text_token,
            inputs.text_mask,
            inputs.feat,
            inputs.feat_mask,
            opts.min_len,
            opts.max_len,
            opts.inference_timesteps,
            opts.cfg_value as f64,
            cancel_fn.as_deref(),
        )?;

        Ok(decode_latent_to_samples(&self.model.audio_vae, latent)?)
    }

    /// Streaming variant of [`Self::generate`]: returns an iterator that
    /// yields chunks of mono `f32` audio samples (at [`Self::sample_rate`])
    /// as they become available, instead of returning the entire waveform
    /// at once.
    ///
    /// Each call to [`Iterator::next`] runs up to
    /// [`GenerateOptions::chunk_patches`] autoregressive steps, then decodes
    /// the accumulated latent through the AudioVAE and yields only the new
    /// audio samples since the previous chunk. Audio is bit-identical to
    /// what [`Self::generate`] would produce — chunk boundaries are
    /// seamless because the AudioVAE decoder is causal.
    ///
    /// The iterator stops when the model emits a stop token (or `max_len`
    /// is hit). [`crate::Error::Cancelled`] is yielded if the
    /// [`CancelToken`] is signalled mid-generation.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use voxcpm_rs::{GenerateOptions, VoxCPM};
    /// # type B = burn::backend::NdArray<f32>;
    /// # let model: VoxCPM<B> = unimplemented!();
    /// let opts = GenerateOptions::builder().chunk_patches(5).build();
    /// let mut all = Vec::<f32>::new();
    /// for chunk in model.generate_stream("Hello, world!", opts)? {
    ///     let chunk = chunk?;
    ///     // play / send / write `chunk` here as soon as it arrives
    ///     all.extend_from_slice(&chunk);
    /// }
    /// # Ok::<_, voxcpm_rs::Error>(())
    /// ```
    ///
    /// # Latency vs. throughput
    ///
    /// Smaller [`GenerateOptions::chunk_patches`] → lower per-chunk latency
    /// but more redundant VAE-decode work (each chunk re-decodes the full
    /// accumulated latent). The default `5` is a sensible balance for
    /// real-time playback (~400 ms / chunk @ default model config). Setting
    /// it to `1` minimises latency at the cost of `O(N²)` decode work
    /// across the whole utterance.
    pub fn generate_stream(
        &self,
        text: &str,
        opts: GenerateOptions,
    ) -> crate::Result<GenerateStream<'_, B>> {
        let inputs = self.build_inference_inputs(text, &opts.prompt)?;
        let state = self.model.prefill(
            inputs.text_token,
            inputs.text_mask,
            inputs.feat,
            inputs.feat_mask,
            opts.max_len,
        );
        Ok(GenerateStream {
            model: &self.model,
            state,
            pred_feats: Vec::new(),
            samples_emitted: 0,
            step: 0,
            min_len: opts.min_len,
            max_len: opts.max_len,
            inference_timesteps: opts.inference_timesteps,
            cfg_value: opts.cfg_value as f64,
            chunk_patches: opts.chunk_patches.max(1),
            cancel: opts.cancel,
            finished: false,
        })
    }

    /// Shared between [`Self::generate`] and [`Self::generate_stream`]:
    /// tokenize text, encode optional prompt audios, and assemble the
    /// `[1, S, P, D]` feat tensor with its text/feat masks.
    fn build_inference_inputs(
        &self,
        text: &str,
        prompt: &Prompt,
    ) -> crate::Result<InferenceInputs<B>> {
        // Decompose the prompt into the (optional) reference + continuation
        // pieces the sequence builder consumes.
        let (ref_audio, prompt_audio, prompt_text) = match prompt {
            Prompt::None => (None, None, None),
            Prompt::Reference { audio } => (Some(audio), None, None),
            Prompt::Continuation { audio, text } => (None, Some(audio), Some(text.as_str())),
            Prompt::Combined {
                reference_audio,
                prompt_audio,
                prompt_text,
            } => (
                Some(reference_audio),
                Some(prompt_audio),
                Some(prompt_text.as_str()),
            ),
        };

        let device = &self.device;
        let p = self.model.patch_size();
        let d = self.model.latent_dim();

        // 1) Tokenize text. In continuation modes prompt_text is prepended.
        let full_text: String = match prompt_text {
            Some(pt) => format!("{pt}{text}"),
            None => text.to_string(),
        };
        let mut text_tokens = self.tokenizer.encode(&full_text)?;
        text_tokens.push(AUDIO_START_TOKEN);
        let text_len = text_tokens.len();

        // 2) Encode optional prompt audios.
        let ref_feat_opt = match ref_audio {
            Some(audio) => Some(self.encode_prompt_audio(audio, PadMode::Right)?),
            None => None,
        };
        let prompt_feat_opt = match prompt_audio {
            Some(audio) => Some(self.encode_prompt_audio(audio, PadMode::Left)?),
            None => None,
        };

        // 3) Build the full sequence of tokens / masks / feats.
        let z_patch = |n: usize| -> Tensor<B, 3> { Tensor::<B, 3>::zeros([n, p, d], device) };

        let mut tokens: Vec<i64> = Vec::new();
        let mut t_mask: Vec<f32> = Vec::new();
        let mut f_mask: Vec<f32> = Vec::new();
        let mut feat_chunks: Vec<Tensor<B, 3>> = Vec::new();

        // [a] Optional reference prefix: [REF_START, ref×N, REF_END]
        if let Some(ref_feat) = ref_feat_opt {
            let ref_len = ref_feat.dims()[0];
            tokens.push(REF_AUDIO_START_TOKEN);
            tokens.extend(std::iter::repeat_n(0i64, ref_len));
            tokens.push(REF_AUDIO_END_TOKEN);
            t_mask.push(1.0);
            t_mask.extend(std::iter::repeat_n(0.0, ref_len));
            t_mask.push(1.0);
            f_mask.push(0.0);
            f_mask.extend(std::iter::repeat_n(1.0, ref_len));
            f_mask.push(0.0);
            feat_chunks.push(z_patch(1));
            feat_chunks.push(ref_feat);
            feat_chunks.push(z_patch(1));
        }

        // [b] Text tokens (always present).
        tokens.extend_from_slice(&text_tokens);
        t_mask.extend(std::iter::repeat_n(1.0, text_len));
        f_mask.extend(std::iter::repeat_n(0.0, text_len));
        feat_chunks.push(z_patch(text_len));

        // [c] Optional continuation suffix: zero text-tokens at the audio
        //     positions, ones in the audio mask, and the prompt latent patches.
        if let Some(prompt_feat) = prompt_feat_opt {
            let prompt_len = prompt_feat.dims()[0];
            tokens.extend(std::iter::repeat_n(0i64, prompt_len));
            t_mask.extend(std::iter::repeat_n(0.0, prompt_len));
            f_mask.extend(std::iter::repeat_n(1.0, prompt_len));
            feat_chunks.push(prompt_feat);
        }

        let s = tokens.len();
        let feat_seq = if feat_chunks.len() == 1 {
            feat_chunks.pop().unwrap()
        } else {
            Tensor::cat(feat_chunks, 0)
        };
        let text_token: Tensor<B, 2, Int> =
            Tensor::from_data(TensorData::new(tokens, [1, s]), device);
        let text_mask: Tensor<B, 2> =
            Tensor::from_data(TensorData::new(t_mask, [1, s]), device);
        let feat_mask: Tensor<B, 2> =
            Tensor::from_data(TensorData::new(f_mask, [1, s]), device);
        let feat: Tensor<B, 4> = feat_seq.unsqueeze_dim(0); // [1, S, P, D]

        Ok(InferenceInputs {
            text_token,
            text_mask,
            feat,
            feat_mask,
        })
    }

    /// Generate a single segment with a specific `Prompt`. Used by the
    /// parallel-segment fast path to produce the first-segment seed audio.
    /// Returns the (latent, samples) pair.
    fn generate_one_with_prompt(
        &self,
        text: &str,
        prompt: &Prompt,
        opts: &GenerateOptions,
        cancel_fn: Option<&dyn Fn() -> bool>,
    ) -> crate::Result<Vec<f32>> {
        let inputs = self.build_inference_inputs(text, prompt)?;
        let (latent, _stops) = self.model.inference(
            inputs.text_token,
            inputs.text_mask,
            inputs.feat,
            inputs.feat_mask,
            opts.min_len,
            opts.max_len,
            opts.inference_timesteps,
            opts.cfg_value as f64,
            cancel_fn,
        )?;
        decode_latent_to_samples(&self.model.audio_vae, latent)
    }

    /// Build a right-padded batched inputs tensor for `texts.len()` segments
    /// that all share the same (optional) reference audio prefix.
    /// Returns `(text_token[B,S], text_mask[B,S], feat[B,S,P,D],
    /// feat_mask[B,S], prefill_lengths[B])`.
    fn build_batched_inputs(
        &self,
        texts: &[&str],
        ref_feat_opt: Option<&Tensor<B, 3>>,
    ) -> crate::Result<(
        Tensor<B, 2, Int>,
        Tensor<B, 2>,
        Tensor<B, 4>,
        Tensor<B, 2>,
        Vec<usize>,
    )> {
        let device = &self.device;
        let p = self.model.patch_size();
        let d = self.model.latent_dim();

        // Per-row tokens / masks / feats.
        let mut rows_tt: Vec<Vec<i64>> = Vec::with_capacity(texts.len());
        let mut rows_tm: Vec<Vec<f32>> = Vec::with_capacity(texts.len());
        let mut rows_fm: Vec<Vec<f32>> = Vec::with_capacity(texts.len());
        let mut rows_feat_chunks: Vec<Vec<Tensor<B, 3>>> = Vec::with_capacity(texts.len());
        let mut row_lens: Vec<usize> = Vec::with_capacity(texts.len());

        let z_patch = |n: usize| -> Tensor<B, 3> { Tensor::<B, 3>::zeros([n, p, d], device) };

        // Reference prefix (shared across all rows). If ref_feat is given,
        // each row starts with [REF_START, ref×K, REF_END] of length K+2.
        let ref_len = ref_feat_opt.map(|f| f.dims()[0]).unwrap_or(0);

        for &text in texts {
            let mut text_tokens = self.tokenizer.encode(text)?;
            text_tokens.push(AUDIO_START_TOKEN);
            let text_len = text_tokens.len();

            let mut tt: Vec<i64> = Vec::new();
            let mut tm: Vec<f32> = Vec::new();
            let mut fm: Vec<f32> = Vec::new();
            let mut feat_chunks: Vec<Tensor<B, 3>> = Vec::new();

            if let Some(rf) = ref_feat_opt {
                tt.push(REF_AUDIO_START_TOKEN);
                tt.extend(std::iter::repeat_n(0i64, ref_len));
                tt.push(REF_AUDIO_END_TOKEN);
                tm.push(1.0);
                tm.extend(std::iter::repeat_n(0.0, ref_len));
                tm.push(1.0);
                fm.push(0.0);
                fm.extend(std::iter::repeat_n(1.0, ref_len));
                fm.push(0.0);
                feat_chunks.push(z_patch(1));
                feat_chunks.push(rf.clone());
                feat_chunks.push(z_patch(1));
            }

            tt.extend_from_slice(&text_tokens);
            tm.extend(std::iter::repeat_n(1.0, text_len));
            fm.extend(std::iter::repeat_n(0.0, text_len));
            feat_chunks.push(z_patch(text_len));

            row_lens.push(tt.len());
            rows_tt.push(tt);
            rows_tm.push(tm);
            rows_fm.push(fm);
            rows_feat_chunks.push(feat_chunks);
        }

        let max_s = *row_lens.iter().max().unwrap_or(&0);
        if max_s == 0 {
            return Err(crate::Error::Other("no text in any segment".into()));
        }

        // Right-pad each row to max_s.
        let batch = texts.len();
        let mut pad_tt = vec![0i64; batch * max_s];
        let mut pad_tm = vec![0.0f32; batch * max_s];
        let mut pad_fm = vec![0.0f32; batch * max_s];
        let mut row_feats: Vec<Tensor<B, 4>> = Vec::with_capacity(batch);

        for (b, ((tt, tm), (fm, mut chunks))) in rows_tt.iter().zip(rows_tm.iter())
            .zip(rows_fm.iter().zip(rows_feat_chunks.into_iter()))
            .enumerate()
        {
            let s = tt.len();
            for j in 0..s {
                pad_tt[b * max_s + j] = tt[j];
                pad_tm[b * max_s + j] = tm[j];
                pad_fm[b * max_s + j] = fm[j];
            }
            // Build [S, P, D] feat for this row, then pad.
            let feat_row: Tensor<B, 3> = if chunks.len() == 1 {
                chunks.pop().unwrap()
            } else {
                Tensor::cat(chunks, 0)
            };
            let feat_row = if s == max_s {
                feat_row
            } else {
                let pad = max_s - s;
                Tensor::cat(vec![feat_row, z_patch(pad)], 0)
            };
            row_feats.push(feat_row.unsqueeze::<4>()); // [1, max_s, P, D]
        }

        let text_token: Tensor<B, 2, Int> =
            Tensor::from_data(TensorData::new(pad_tt, [batch, max_s]), device);
        let text_mask: Tensor<B, 2> =
            Tensor::from_data(TensorData::new(pad_tm, [batch, max_s]), device);
        let feat_mask: Tensor<B, 2> =
            Tensor::from_data(TensorData::new(pad_fm, [batch, max_s]), device);
        let feat: Tensor<B, 4> = Tensor::cat(row_feats, 0);

        Ok((text_token, text_mask, feat, feat_mask, row_lens))
    }

    /// Encode a slice of mono PCM samples (assumed at this model's audio
    /// VAE sample rate) into a `[T, P, D]` reference-feature tensor,
    /// right-padding the audio if needed. Used to turn a self-seeded first
    /// segment into a reference for the remaining segments.
    fn pcm_to_ref_feat(&self, samples: &[f32]) -> crate::Result<Tensor<B, 3>> {
        // Resample from output sr (sample_rate()) to the AudioVAE encoder sr.
        let in_sr = self.model.sample_rate() as u32;
        let enc_sr = self.model.audio_vae.sample_rate() as u32;
        let resampled = if in_sr == enc_sr {
            samples.to_vec()
        } else {
            crate::audio::resample(samples, in_sr, enc_sr)?
        };
        self.encode_prompt_audio(
            &PromptAudio::Pcm { samples: resampled, sample_rate: enc_sr },
            PadMode::Right,
        )
    }

    /// Run the parallel-segment generation pipeline. See
    /// [`GenerateOptions::parallel_segments`] for the public contract.
    /// Start building a batch of independent generations that will run in
    /// a single batched forward pass. See [`BatchBuilder`] for the full
    /// contract and a worked example.
    pub fn batch(&self) -> BatchBuilder<'_, B> {
        BatchBuilder { voxcpm: self, items: Vec::new() }
    }

    /// Internal: build per-row inputs, right-pad to max_S, batch-cat, run
    /// `inference_with_lengths`, and decode each row's latent slice.
    fn run_batch(
        &self,
        items: Vec<(String, Prompt)>,
        opts: GenerateOptions,
    ) -> crate::Result<Vec<Vec<f32>>> {
        let device = &self.device;
        let p = self.model.patch_size();
        let d = self.model.latent_dim();

        // 1) Build per-item B=1 inputs reusing the existing single-item path.
        let mut rows: Vec<InferenceInputs<B>> = Vec::with_capacity(items.len());
        let mut lens: Vec<usize> = Vec::with_capacity(items.len());
        for (text, prompt) in &items {
            let inp = self.build_inference_inputs(text, prompt)?;
            lens.push(inp.text_token.dims()[1]);
            rows.push(inp);
        }
        let max_s = *lens.iter().max().unwrap();

        // 2) Right-pad each row to max_s with zeros, then cat along dim 0.
        let mut tt_rows: Vec<Tensor<B, 2, Int>> = Vec::with_capacity(rows.len());
        let mut tm_rows: Vec<Tensor<B, 2>> = Vec::with_capacity(rows.len());
        let mut fm_rows: Vec<Tensor<B, 2>> = Vec::with_capacity(rows.len());
        let mut feat_rows: Vec<Tensor<B, 4>> = Vec::with_capacity(rows.len());
        for (i, inp) in rows.into_iter().enumerate() {
            let s = lens[i];
            let pad = max_s - s;
            let (tt, tm, ft, fm) = if pad == 0 {
                (inp.text_token, inp.text_mask, inp.feat, inp.feat_mask)
            } else {
                let tt_pad: Tensor<B, 2, Int> =
                    Tensor::zeros([1, pad], device);
                let tm_pad: Tensor<B, 2> = Tensor::zeros([1, pad], device);
                let fm_pad: Tensor<B, 2> = Tensor::zeros([1, pad], device);
                let ft_pad: Tensor<B, 4> = Tensor::zeros([1, pad, p, d], device);
                (
                    Tensor::cat(vec![inp.text_token, tt_pad], 1),
                    Tensor::cat(vec![inp.text_mask, tm_pad], 1),
                    Tensor::cat(vec![inp.feat, ft_pad], 1),
                    Tensor::cat(vec![inp.feat_mask, fm_pad], 1),
                )
            };
            tt_rows.push(tt);
            tm_rows.push(tm);
            feat_rows.push(ft);
            fm_rows.push(fm);
        }
        let text_token = Tensor::cat(tt_rows, 0);
        let text_mask = Tensor::cat(tm_rows, 0);
        let feat = Tensor::cat(feat_rows, 0);
        let feat_mask = Tensor::cat(fm_rows, 0);

        // 3) Run batched inference.
        let cancel_fn: Option<Box<dyn Fn() -> bool>> = opts.cancel.as_ref().map(|c| {
            let c = c.clone();
            Box::new(move || c.is_cancelled()) as Box<dyn Fn() -> bool>
        });
        let (latent, stops) = self.model.inference_with_lengths(
            text_token,
            text_mask,
            feat,
            feat_mask,
            opts.min_len,
            opts.max_len,
            opts.inference_timesteps,
            opts.cfg_value as f64,
            cancel_fn.as_deref(),
            Some(lens),
        )?;

        // 4) Decode each row independently using its own stop_step.
        let dims = latent.dims();
        let mut out: Vec<Vec<f32>> = Vec::with_capacity(items.len());
        for i in 0..items.len() {
            let stop_i = stops[i];
            let pat = (stop_i * p).min(dims[2]);
            if pat == 0 {
                out.push(Vec::new());
                continue;
            }
            let lat_i = latent.clone().slice([i..i + 1, 0..dims[1], 0..pat]);
            let pcm = decode_latent_to_samples(&self.model.audio_vae, lat_i)?;
            out.push(pcm);
        }
        Ok(out)
    }

    fn generate_parallel(
        &self,
        segments: &[String],
        parallel_n: usize,
        opts: &GenerateOptions,
    ) -> crate::Result<Vec<f32>> {
        debug_assert!(parallel_n >= 2);
        debug_assert!(segments.len() >= 2);

        let cancel_fn: Option<Box<dyn Fn() -> bool>> = opts.cancel.as_ref().map(|c| {
            let c = c.clone();
            Box::new(move || c.is_cancelled()) as Box<dyn Fn() -> bool>
        });

        // Establish a voice reference: either the user's explicit
        // `Prompt::Reference` audio, or a self-seed = the first segment
        // generated serially.
        let (ref_feat, mut output_audio): (Tensor<B, 3>, Vec<f32>) = match &opts.prompt {
            Prompt::Reference { audio } => {
                let rf = self.encode_prompt_audio(audio, PadMode::Right)?;
                (rf, Vec::new())
            }
            Prompt::None => {
                let seed_text = segments[0].as_str();
                let seed_pcm = self.generate_one_with_prompt(
                    seed_text,
                    &Prompt::None,
                    opts,
                    cancel_fn.as_deref(),
                )?;
                let rf = self.pcm_to_ref_feat(&seed_pcm)?;
                (rf, seed_pcm)
            }
            // Other prompt modes are filtered out by the dispatch in `generate`.
            _ => unreachable!(),
        };

        // Decide which segments still need generation: skip the first if we
        // self-seeded with it.
        let remaining: &[String] = match &opts.prompt {
            Prompt::None => &segments[1..],
            _ => &segments[..],
        };

        // Process remaining segments in batched groups of `parallel_n`.
        for group in remaining.chunks(parallel_n) {
            let texts_ref: Vec<&str> = group.iter().map(|s| s.as_str()).collect();
            let (tt, tm, ft, fm, lens) =
                self.build_batched_inputs(&texts_ref, Some(&ref_feat))?;
            let (latent, stops) = self.model.inference_with_lengths(
                tt, tm, ft, fm,
                opts.min_len,
                opts.max_len,
                opts.inference_timesteps,
                opts.cfg_value as f64,
                cancel_fn.as_deref(),
                Some(lens),
            )?;
            let dims = latent.dims();
            let p = self.model.patch_size();
            for i in 0..group.len() {
                let stop_i = stops[i];
                let pat = (stop_i * p).min(dims[2]);
                if pat == 0 {
                    continue;
                }
                let lat_i = latent.clone().slice([i..i + 1, 0..dims[1], 0..pat]);
                let pcm = decode_latent_to_samples(&self.model.audio_vae, lat_i)?;
                output_audio.extend_from_slice(&pcm);
            }
        }

        Ok(output_audio)
    }
}

/// Builder returned by [`VoxCPM::batch`] for generating several utterances
/// in one batched forward pass.
///
/// Each item carries its own text and `Prompt`, so different items can use
/// different voice references (or none at all). All items in a batch share
/// the same [`GenerateOptions`] passed to [`Self::run`]; per-item
/// `GenerateOptions::prompt` is ignored — use the prompt argument of
/// [`Self::add`] instead.
///
/// # When to use this vs `parallel_segments`
///
/// - [`GenerateOptions::parallel_segments`]: ONE long text, automatically
///   split into sentences sharing one voice. Self-seeds when no
///   reference is given. Best when you want one utterance read faster.
/// - [`VoxCPM::batch`]: MANY independent utterances, possibly with
///   different voices. No self-seeding, no audio concatenation — you get
///   one PCM buffer per item, in input order. Best when you have a
///   workload of independent requests.
///
/// Both share the same right-pad batched prefill + per-element stop
/// machinery, so throughput scales identically with batch size.
///
/// # Example
///
/// ```no_run
/// use voxcpm_rs::{GenerateOptions, Prompt, VoxCPM};
/// # type B = burn::backend::NdArray<f32>;
/// # let model: VoxCPM<B> = unimplemented!();
/// let opts = GenerateOptions::builder().timesteps(10).build();
/// let outs: Vec<Vec<f32>> = model
///     .batch()
///     .add("Hello, world!", Prompt::None)
///     .add("Goodbye, world!", Prompt::None)
///     .run(opts)?;
/// assert_eq!(outs.len(), 2);
/// # Ok::<_, voxcpm_rs::Error>(())
/// ```
#[derive(Debug)]
pub struct BatchBuilder<'a, B: Backend> {
    voxcpm: &'a VoxCPM<B>,
    items: Vec<(String, Prompt)>,
}

impl<'a, B: Backend> BatchBuilder<'a, B> {
    /// Append an item to the batch. Returns `self` for chaining.
    pub fn add(mut self, text: impl Into<String>, prompt: Prompt) -> Self {
        self.items.push((text.into(), prompt));
        self
    }

    /// Number of items currently in the batch.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// `true` if no items have been added.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Run the batch and return one PCM buffer (mono `f32`,
    /// [`VoxCPM::sample_rate`]) per item, in the order they were added.
    ///
    /// The `opts.prompt` field is ignored — per-item prompts come from
    /// [`Self::add`]. The `opts.parallel_segments` field is also ignored
    /// here; this API IS the parallel batch primitive.
    pub fn run(self, opts: GenerateOptions) -> crate::Result<Vec<Vec<f32>>> {
        if self.items.is_empty() {
            return Ok(Vec::new());
        }
        if self.items.len() == 1 {
            // Single-item shortcut: avoid all the padding overhead.
            let (text, prompt) = self.items.into_iter().next().unwrap();
            let cancel_fn: Option<Box<dyn Fn() -> bool>> = opts.cancel.as_ref().map(|c| {
                let c = c.clone();
                Box::new(move || c.is_cancelled()) as Box<dyn Fn() -> bool>
            });
            let pcm = self
                .voxcpm
                .generate_one_with_prompt(&text, &prompt, &opts, cancel_fn.as_deref())?;
            return Ok(vec![pcm]);
        }
        self.voxcpm.run_batch(self.items, opts)
    }
}


/// Designed for parallel-segment generation: keeps trailing whitespace
/// trimmed and skips empty segments. Not a full ICU sentence splitter; it
/// handles common cases (`.`, `!`, `?`, `\n`) and treats them as hard
/// boundaries. Abbreviations like "Dr." or "U.S.A." will get split (false
/// boundaries) but the resulting segments are still individually
/// pronounceable, just with slightly different prosody.
pub fn split_sentences(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut buf = String::new();
    for c in text.chars() {
        buf.push(c);
        if matches!(c, '.' | '!' | '?' | '\n') {
            let trimmed = buf.trim();
            if !trimmed.is_empty() {
                out.push(trimmed.to_string());
            }
            buf.clear();
        }
    }
    let trimmed = buf.trim();
    if !trimmed.is_empty() {
        out.push(trimmed.to_string());
    }
    out
}

/// Output of [`VoxCPM::build_inference_inputs`].
struct InferenceInputs<B: Backend> {
    text_token: Tensor<B, 2, Int>,
    text_mask: Tensor<B, 2>,
    feat: Tensor<B, 4>,
    feat_mask: Tensor<B, 2>,
}

/// Run AudioVAE decode on a stacked latent and pull the result back as `f32`
/// PCM. Shared between [`VoxCPM::generate`] and [`GenerateStream`].
fn decode_latent_to_samples<B: Backend>(
    audio_vae: &crate::audiovae::AudioVae<B>,
    latent: Tensor<B, 3>,
) -> crate::Result<Vec<f32>> {
    let wav = audio_vae.decode(latent);
    let wav = wav.squeeze_dim::<2>(1); // [B, T_out]
    let wav = wav.squeeze_dim::<1>(0); // [T_out]
    let data = wav.into_data();
    // Backend-agnostic: VAE may produce f32, f16 or bf16 depending on the
    // active Backend; convert to f32 for output regardless.
    data.convert::<f32>()
        .into_vec::<f32>()
        .map_err(|_| crate::Error::Other("unexpected VAE output dtype".into()))
}

/// Iterator returned by [`VoxCPM::generate_stream`]. Yields `Result<Vec<f32>>`
/// chunks of mono PCM at [`VoxCPM::sample_rate`] until generation stops.
///
/// Borrows the underlying [`crate::voxcpm2::VoxCpm2Model`] for its lifetime;
/// to send a stream across threads, collect the chunks in the producing
/// thread and forward them through a channel.
#[derive(Debug)]
pub struct GenerateStream<'a, B: Backend> {
    model: &'a crate::voxcpm2::VoxCpm2Model<B>,
    state: crate::voxcpm2::model::InferenceState<B>,
    pred_feats: Vec<Tensor<B, 4>>,
    samples_emitted: usize,
    step: usize,
    min_len: usize,
    max_len: usize,
    inference_timesteps: usize,
    cfg_value: f64,
    chunk_patches: usize,
    cancel: Option<CancelToken>,
    finished: bool,
}

impl<B: Backend> GenerateStream<'_, B> {
    /// Sample rate (Hz) of the chunks this stream yields. Always equal to
    /// the producing [`VoxCPM::sample_rate`].
    pub fn sample_rate(&self) -> u32 {
        self.model.sample_rate() as u32
    }

    /// Number of autoregressive steps consumed so far. One step ≈ one
    /// latent patch ≈ ~80 ms of audio at the default model config.
    pub fn steps_taken(&self) -> usize {
        self.state.steps_taken
    }

    /// Drive the stream forward up to `chunk_patches` AR steps and either
    /// return `Some(chunk)` of new samples, or `None` when generation is
    /// complete. Errors are returned via the `Result`.
    fn step_chunk(&mut self) -> crate::Result<Option<Vec<f32>>> {
        if self.finished {
            return Ok(None);
        }

        let mut produced_any = false;
        for _ in 0..self.chunk_patches {
            if self.step >= self.max_len {
                self.finished = true;
                break;
            }
            if let Some(c) = &self.cancel
                && c.is_cancelled()
            {
                self.finished = true;
                return Err(crate::Error::Cancelled);
            }

            // `i` matches the loop index from `VoxCpm2Model::inference`: the
            // 0-based index of the patch we're about to produce. The stop
            // head is only honored once `i > min_len` (mirroring the
            // non-streaming path so the streamed audio is bit-identical).
            let i = self.step;
            let crate::voxcpm2::model::DitStep { pred_feat, stops } =
                self.model.dit_step(&mut self.state, self.inference_timesteps, self.cfg_value);
            self.pred_feats.push(pred_feat.clone());
            produced_any = true;

            // Streaming path is B=1; honor element 0's stop bit.
            let stop = stops.first().copied().unwrap_or(false);
            if i > self.min_len && stop {
                self.finished = true;
                self.step += 1;
                break;
            }
            self.model.lm_step(&mut self.state, pred_feat);
            self.step += 1;
        }

        if !produced_any {
            return Ok(None);
        }

        // Decode the cumulative latent and emit only the new tail samples.
        // The AudioVAE decoder is causal, so re-decoding a longer prefix
        // produces bit-identical samples for the already-emitted portion —
        // this gives us seamless chunks without porting Python's stateful
        // StreamingVAEDecoder.
        let latent = crate::voxcpm2::VoxCpm2Model::stack_pred_feats(&self.pred_feats);
        let all = decode_latent_to_samples(&self.model.audio_vae, latent)?;
        if all.len() <= self.samples_emitted {
            // No new samples this round (shouldn't happen, but be safe).
            return Ok(Some(Vec::new()));
        }
        let chunk = all[self.samples_emitted..].to_vec();
        self.samples_emitted = all.len();
        Ok(Some(chunk))
    }
}

impl<B: Backend> Iterator for GenerateStream<'_, B> {
    type Item = crate::Result<Vec<f32>>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.step_chunk() {
                Ok(Some(chunk)) if chunk.is_empty() => {
                    // Avoid yielding empty chunks; either keep stepping or
                    // finish.
                    if self.finished {
                        return None;
                    }
                    continue;
                }
                Ok(Some(chunk)) => return Some(Ok(chunk)),
                Ok(None) => return None,
                Err(e) => return Some(Err(e)),
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum PadMode {
    Right,
    Left,
}

