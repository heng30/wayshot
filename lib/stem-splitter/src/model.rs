use crate::{engine::EngineState, error::Result};
use serde::Deserialize;
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

pub const DEMUCS_MODEL_URL: &str =
    "https://huggingface.co/gentij/htdemucs-ort/resolve/main/htdemucs.ort";

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Layout {
    #[default]
    ChannelsFirst,
    ChannelsLast,
}

#[derive(Clone, Debug, Deserialize)]
pub struct IODesc {
    pub name: String,
    #[serde(default)]
    pub shape: Vec<serde_json::Value>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ModelManifest {
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub backend: String,
    #[serde(default)]
    pub format: String,
    #[serde(default)]
    pub opset: u32,
    #[serde(default = "default_sample_rate", alias = "sample_rate_hz")]
    pub sample_rate: u32,
    #[serde(default = "default_window")]
    pub window: usize,
    #[serde(default = "default_hop")]
    pub hop: usize,
    #[serde(default)]
    pub stems: Vec<String>,
    #[serde(default)]
    pub input_layout: Layout,
    #[serde(default)]
    pub output_layout: Layout,
    #[serde(default)]
    pub inputs: Vec<IODesc>,
    #[serde(default)]
    pub outputs: Vec<IODesc>,
}

fn default_sample_rate() -> u32 {
    44100
}

fn default_window() -> usize {
    512
}

fn default_hop() -> usize {
    512
}

impl ModelManifest {
    const EMBEDDED_MANIFEST: &str = include_str!("../manifest.json");

    pub fn from_json_str(json: &str) -> Result<Self> {
        Ok(serde_json::from_str(json)?)
    }

    pub fn from_embedded() -> Result<Self> {
        Ok(serde_json::from_str(Self::EMBEDDED_MANIFEST)?)
    }

    pub fn default_stems(&self) -> Vec<String> {
        if self.stems.is_empty() {
            vec![
                "vocals".into(),
                "drums".into(),
                "bass".into(),
                "other".into(),
            ]
        } else {
            self.stems.clone()
        }
    }

    pub fn input_name(&self, idx: usize) -> Option<&str> {
        self.inputs.get(idx).map(|d| d.name.as_str())
    }

    pub fn output_name(&self, idx: usize) -> Option<&str> {
        self.outputs.get(idx).map(|d| d.name.as_str())
    }

    pub fn frequency_bins(&self) -> usize {
        self.window / 2 + 1
    }
}

pub struct ModelHandle {
    pub manifest: ModelManifest,
    pub local_path: PathBuf,
    cancel: Arc<AtomicBool>,
    engine: Arc<EngineState>,
}

impl ModelHandle {
    pub fn new(manifest: ModelManifest, local_path: PathBuf) -> Result<Self> {
        let engine = EngineState::new(&local_path)?;
        Ok(Self {
            manifest,
            local_path,
            cancel: Arc::new(AtomicBool::new(false)),
            engine: Arc::new(engine),
        })
    }

    pub fn from_embedded(model_path: PathBuf) -> Result<Self> {
        let manifest = ModelManifest::from_embedded()?;
        Self::new(manifest, model_path)
    }

    pub fn from_json_file(manifest_path: &std::path::Path, model_path: PathBuf) -> Result<Self> {
        let json = std::fs::read_to_string(manifest_path)?;
        let manifest: ModelManifest = serde_json::from_str(&json)?;
        Self::new(manifest, model_path)
    }

    pub fn request_cancel(&self) {
        self.cancel.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::Acquire)
    }

    pub fn reset_cancel(&self) {
        self.cancel.store(false, Ordering::Release);
    }

    pub fn engine(&self) -> &EngineState {
        &self.engine
    }
}

pub fn load_manifest_from_json(json: &str) -> Result<ModelManifest> {
    Ok(serde_json::from_str(json)?)
}
