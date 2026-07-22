#![allow(missing_docs)]

use std::path::{Path, PathBuf};

#[cfg(feature = "online")]
use crate::inference::ExecutionMode;

const SEGMENTATION_ONNX: &str = "segmentation-3.0.onnx";
const EMBEDDING_ONNX: &str = "wespeaker-voxceleb-resnet34.onnx";

#[derive(Debug, Clone)]
pub struct ModelBundle {
    segmentation_onnx: PathBuf,
    embedding_onnx: PathBuf,
    plda_dir: PathBuf,
}

impl ModelBundle {
    pub fn from_dir(models_dir: impl Into<PathBuf>) -> Self {
        let dir = models_dir.into();
        Self {
            segmentation_onnx: dir.join(SEGMENTATION_ONNX),
            embedding_onnx: dir.join(EMBEDDING_ONNX),
            plda_dir: dir,
        }
    }

    #[cfg(feature = "online")]
    pub fn from_pretrained(mode: ExecutionMode) -> Result<Self, hf_hub::HFError> {
        let manager = ModelManager::new()?;
        let dir = manager.ensure(mode)?;
        Ok(Self::from_dir(dir))
    }

    pub fn segmentation_path(&self) -> &Path {
        &self.segmentation_onnx
    }

    pub fn embedding_path(&self) -> &Path {
        &self.embedding_onnx
    }

    pub fn plda_dir(&self) -> &Path {
        &self.plda_dir
    }
}

#[cfg(feature = "online")]
const HF_REPO_OWNER: &str = "avencera";
#[cfg(feature = "online")]
const HF_REPO_NAME: &str = "speakrs-models";

#[cfg(feature = "online")]
pub struct ModelManager {
    repo: hf_hub::HFRepositorySync<hf_hub::RepoTypeModel>,
}

#[cfg(feature = "online")]
impl ModelManager {
    pub fn new() -> Result<Self, hf_hub::HFError> {
        let client = hf_hub::HFClientSync::new()?;
        let repo = client.model(HF_REPO_OWNER, HF_REPO_NAME);
        Ok(Self { repo })
    }

    pub fn with_cache_dir(cache_dir: PathBuf) -> Result<Self, hf_hub::HFError> {
        let client = hf_hub::HFClientSync::new()?;
        let repo = client.model(HF_REPO_OWNER, HF_REPO_NAME);
        let _ = cache_dir;
        Ok(Self { repo })
    }

    pub fn get(&self, filename: impl AsRef<str>) -> Result<PathBuf, hf_hub::HFError> {
        self.repo.download_file().filename(filename.as_ref()).send()
    }

    pub fn ensure(&self, mode: ExecutionMode) -> Result<PathBuf, hf_hub::HFError> {
        let files = required_files(mode);
        let dir = self
            .repo
            .snapshot_download()
            .allow_patterns(files)
            .send()?;
        Ok(dir)
    }
}

#[cfg(feature = "online")]
const PLDA_FILES: &[&str] = &[
    "plda_lda.npy",
    "plda_tr.npy",
    "plda_mu.npy",
    "plda_psi.npy",
    "plda_mean1.npy",
    "plda_mean2.npy",
    "wespeaker-voxceleb-resnet34.min_num_samples.txt",
];

#[cfg(feature = "online")]
const ONNX_FILES: &[&str] = &[
    "segmentation-3.0.onnx",
    "wespeaker-voxceleb-resnet34.onnx",
    "wespeaker-voxceleb-resnet34.onnx.data",
];

#[cfg(feature = "online")]
fn required_files(mode: ExecutionMode) -> Vec<String> {
    let mut files: Vec<String> = PLDA_FILES.iter().map(|s| s.to_string()).collect();

    match mode {
        ExecutionMode::Cpu => {
            files.extend(ONNX_FILES.iter().map(|s| s.to_string()));
        }
        ExecutionMode::Cuda | ExecutionMode::CudaFast => {
            files.extend(ONNX_FILES.iter().map(|s| s.to_string()));
            files.push("wespeaker-fbank.onnx".to_string());
            files.push("wespeaker-fbank-b32.onnx".to_string());
            files.push("wespeaker-multimask-tail.onnx".to_string());
            files.push("wespeaker-multimask-tail-b32.onnx".to_string());
            files.push("segmentation-3.0-b32.onnx".to_string());
            files.push("wespeaker-voxceleb-resnet34-b64.onnx".to_string());
        }
    }

    files
}
