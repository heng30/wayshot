pub mod model;
pub mod musicgen;
pub use model::Model;

use log::info;
use musicgen::{
    DecoderError, MusicGenAudioEncodec, MusicGenConfig, MusicGenDecoder, MusicGenMergedDecoder,
    MusicGenSplitDecoder, MusicGenTextEncoder,
};
use ort::session::Session;
use std::{
    collections::VecDeque,
    path::Path,
    sync::{Arc, Mutex},
};
use tokenizers::Tokenizer;

/// Number of token batches generated per second of audio.
pub const INPUT_IDS_BATCH_PER_SECOND: usize = 50;

/// Whether the decoder ONNX model is split into two files
/// (`decoder_model.onnx` + `decoder_with_past_model.onnx`)
/// or merged into a single file (`decoder_model_merged.onnx`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecoderMode {
    Split,
    Merged,
}

/// The output of audio generation: raw sample data plus metadata.
#[derive(Debug, Clone)]
pub struct AudioOutput {
    /// The audio sample values as f32, mono channel.
    pub samples: Vec<f32>,
    /// The sampling rate in Hz (e.g. 32000 for MusicGen small/medium).
    pub sample_rate: u32,
    /// Number of audio channels (always 1 for MusicGen).
    pub channels: u16,
}

/// Error type for the MusicGen library.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("ONNX Runtime error: {0}")]
    Ort(#[from] ort::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Tokenizer error: {0}")]
    Tokenizer(#[from] tokenizers::Error),
    #[error("Decoder error: {0}")]
    Decoder(#[from] DecoderError),
    #[error("Invalid duration: {0}")]
    InvalidDuration(String),
    #[error("Generation aborted")]
    Aborted,
}

/// The main MusicGen inference handle.
///
/// Load a model from a local directory with [`MusicGen::load`], then call
/// [`MusicGen::generate`] to produce audio from a text prompt.
pub struct MusicGen {
    text_encoder: MusicGenTextEncoder,
    decoder: Box<dyn MusicGenDecoder>,
    audio_encodec: MusicGenAudioEncodec,
    sampling_rate: u32,
}

impl MusicGen {
    /// Load a MusicGen model from a local directory.
    ///
    /// The directory must contain the ONNX model files, `config.json`, and
    /// `tokenizer.json`. These are the same files that the original MusicGPT
    /// downloads from HuggingFace — you can obtain them manually or from a
    /// previous MusicGPT data directory.
    ///
    /// # Arguments
    ///
    /// * `model_dir` - Path to the directory containing model files.
    /// * `model` - Which model variant to load.
    /// * `decoder_mode` - Whether the decoder is split or merged.
    ///
    /// # Expected directory layout
    ///
    /// For `DecoderMode::Merged`:
    /// ```text
    /// model_dir/
    ///   config.json
    ///   tokenizer.json
    ///   text_encoder.onnx
    ///   decoder_model_merged.onnx
    ///   encodec_decode.onnx
    /// ```
    ///
    /// For `DecoderMode::Split`:
    /// ```text
    /// model_dir/
    ///   config.json
    ///   tokenizer.json
    ///   text_encoder.onnx
    ///   decoder_model.onnx
    ///   decoder_with_past_model.onnx
    ///   encodec_decode.onnx
    /// ```
    pub fn load(model_dir: &Path, model: Model, decoder_mode: DecoderMode) -> Result<Self, Error> {
        let config_path = model_dir.join("config.json");
        let tokenizer_path = model_dir.join("tokenizer.json");

        // Load config
        let config_str = std::fs::read_to_string(&config_path)?;
        let config: MusicGenConfig = serde_json::from_str(&config_str)?;
        let sampling_rate = config.audio_encoder.sampling_rate as u32;

        // Load tokenizer
        let mut tokenizer = Tokenizer::from_file(&tokenizer_path)?;
        tokenizer
            .with_padding(None)
            .with_truncation(None)
            .expect("Could not configure tokenizer");

        // Load text encoder
        let text_encoder_path = model_dir.join("text_encoder.onnx");
        info!("Loading text encoder from {:?}", text_encoder_path);
        let text_encoder_session = Session::builder()?.commit_from_file(&text_encoder_path)?;

        // Load decoder(s)
        let decoder: Box<dyn MusicGenDecoder> = match decoder_mode {
            DecoderMode::Merged => {
                let decoder_path = model_dir.join("decoder_model_merged.onnx");
                info!("Loading merged decoder from {:?}", decoder_path);
                let session = Session::builder()?.commit_from_file(&decoder_path)?;
                if matches!(model, Model::SmallFp16 | Model::MediumFp16) {
                    Box::new(MusicGenMergedDecoder::<half::f16> {
                        decoder_model_merged: Arc::new(Mutex::new(session)),
                        config,
                        _phantom_data: Default::default(),
                    })
                } else {
                    Box::new(MusicGenMergedDecoder::<f32> {
                        decoder_model_merged: Arc::new(Mutex::new(session)),
                        config,
                        _phantom_data: Default::default(),
                    })
                }
            }
            DecoderMode::Split => {
                let decoder_path = model_dir.join("decoder_model.onnx");
                let decoder_with_past_path = model_dir.join("decoder_with_past_model.onnx");
                info!("Loading split decoder from {:?}", decoder_path);
                let decoder_session = Session::builder()?.commit_from_file(&decoder_path)?;
                info!(
                    "Loading decoder_with_past from {:?}",
                    decoder_with_past_path
                );
                let decoder_with_past_session =
                    Session::builder()?.commit_from_file(&decoder_with_past_path)?;
                if matches!(model, Model::SmallFp16 | Model::MediumFp16) {
                    Box::new(MusicGenSplitDecoder::<half::f16> {
                        decoder_model: Arc::new(Mutex::new(decoder_session)),
                        decoder_with_past_model: Arc::new(Mutex::new(decoder_with_past_session)),
                        config,
                        _phantom_data: Default::default(),
                    })
                } else {
                    Box::new(MusicGenSplitDecoder::<f32> {
                        decoder_model: Arc::new(Mutex::new(decoder_session)),
                        decoder_with_past_model: Arc::new(Mutex::new(decoder_with_past_session)),
                        config,
                        _phantom_data: Default::default(),
                    })
                }
            }
        };

        // Load audio encodec
        let encodec_path = model_dir.join("encodec_decode.onnx");
        info!("Loading audio encodec from {:?}", encodec_path);
        let encodec_session = Session::builder()?.commit_from_file(&encodec_path)?;

        Ok(MusicGen {
            text_encoder: MusicGenTextEncoder {
                tokenizer,
                text_encoder: text_encoder_session,
            },
            decoder,
            audio_encodec: MusicGenAudioEncodec {
                audio_encodec_decode: encodec_session,
            },
            sampling_rate,
        })
    }

    /// Generate audio from a text prompt.
    ///
    /// Returns an [`AudioOutput`] containing the raw f32 samples, sampling rate,
    /// and channel count. The caller decides how to save, play, or otherwise
    /// process the audio data.
    ///
    /// # Arguments
    ///
    /// * `prompt` - The text description of the music to generate.
    /// * `secs` - Duration of audio to generate in seconds (1–30).
    /// * `on_progress` - Callback invoked with `(current, total)` token counts.
    ///   Return `true` from the callback to abort generation.
    pub fn generate(
        &mut self,
        prompt: &str,
        secs: usize,
        on_progress: Box<dyn Fn(f32, f32) -> bool + Sync + Send + 'static>,
    ) -> Result<AudioOutput, Error> {
        if secs < 1 {
            return Err(Error::InvalidDuration(
                "Duration must be at least 1 second".to_string(),
            ));
        }
        if secs > 30 {
            return Err(Error::InvalidDuration(
                "Duration must be at most 30 seconds".to_string(),
            ));
        }

        let max_len = secs * INPUT_IDS_BATCH_PER_SECOND;

        let (lhs, am) = self.text_encoder.encode(prompt)?;
        let token_stream = self.decoder.generate_tokens(lhs, am, max_len)?;

        let mut tokens: Vec<[i64; 4]> = Vec::with_capacity(max_len);
        while let Ok(result) = token_stream.recv() {
            let t = result?;
            tokens.push(t);
            let should_exit = on_progress(tokens.len() as f32, max_len as f32);
            if should_exit {
                return Err(Error::Aborted);
            }
        }

        let samples: VecDeque<f32> = self.audio_encodec.encode(tokens)?;

        Ok(AudioOutput {
            samples: samples.into(),
            sample_rate: self.sampling_rate,
            channels: 1,
        })
    }

    /// Convenience method: generate audio without a progress callback.
    pub fn generate_simple(&mut self, prompt: &str, secs: usize) -> Result<AudioOutput, Error> {
        self.generate(prompt, secs, Box::new(|_, _| false))
    }
}
