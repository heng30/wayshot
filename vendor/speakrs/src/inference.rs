#![allow(missing_docs, unexpected_cfgs)]

pub(crate) mod embedding;
pub(crate) mod segmentation;

use std::fmt;
use std::path::PathBuf;

pub use embedding::EmbeddingModel;
pub use segmentation::{SegmentationError, SegmentationModel};

use ort::ep;
use ort::session::builder::SessionBuilder;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CoreMlComputeUnits {
    #[default]
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExecutionMode {
    #[default]
    Cpu,
    Cuda,
    CudaFast,
}

impl ExecutionMode {
    pub const fn is_coreml(self) -> bool {
        false
    }

    pub const fn is_cuda(self) -> bool {
        matches!(self, Self::Cuda | Self::CudaFast)
    }

    pub const fn is_migraphx(self) -> bool {
        false
    }

    pub(crate) fn validate(self) -> Result<(), ExecutionModeError> {
        if self == Self::Cpu {
            return Ok(());
        }

        if self.is_cuda() {
            #[cfg(feature = "cuda")]
            {
                return Ok(());
            }

            #[cfg(not(feature = "cuda"))]
            {
                return Err(ExecutionModeError {
                    mode: self,
                    feature: "cuda",
                });
            }
        }

        Ok(())
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::Cuda => "cuda",
            Self::CudaFast => "cuda-fast",
        }
    }
}

impl fmt::Display for ExecutionMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ModelLoadError {
    #[error(transparent)]
    UnsupportedExecutionMode(#[from] ExecutionModeError),
    #[error(transparent)]
    Ort(#[from] ort::Error),
    #[error("{mode} requires native asset `{path}`")]
    MissingNativeAsset {
        mode: ExecutionMode,
        path: PathBuf,
    },
    #[error("{mode} failed to load native asset `{path}`: {message}")]
    NativeAssetLoad {
        mode: ExecutionMode,
        path: PathBuf,
        message: String,
    },
}

#[derive(Debug, Clone, thiserror::Error)]
#[non_exhaustive]
pub enum OrtRuntimeError {
    #[error("failed to initialize ONNX Runtime: {message}")]
    Initialization { message: String },
}

#[derive(Debug, Clone, thiserror::Error)]
#[non_exhaustive]
#[error("{mode} requires the `{feature}` Cargo feature")]
pub struct ExecutionModeError {
    mode: ExecutionMode,
    feature: &'static str,
}

impl From<ExecutionModeError> for ort::Error {
    fn from(error: ExecutionModeError) -> Self {
        ort::Error::new(error.to_string())
    }
}

pub fn with_execution_mode(
    builder: SessionBuilder,
    mode: ExecutionMode,
) -> Result<SessionBuilder, ort::Error> {
    mode.validate()?;

    match mode {
        ExecutionMode::Cpu => Ok(builder
            .with_execution_providers([ep::CPU::default().with_arena_allocator(false).build()])?),
        ExecutionMode::Cuda | ExecutionMode::CudaFast => {
            #[cfg(feature = "cuda")]
            {
                Ok(builder.with_execution_providers([ep::CUDA::default()
                    .with_device_id(0)
                    .with_tf32(true)
                    .with_conv_algorithm_search(ep::cuda::ConvAlgorithmSearch::Exhaustive)
                    .with_conv_max_workspace(true)
                    .with_arena_extend_strategy(ep::ArenaExtendStrategy::SameAsRequested)
                    .with_prefer_nhwc(true)
                    .build()
                    .error_on_failure()])?)
            }

            #[cfg(not(feature = "cuda"))]
            {
                unreachable!("mode validation rejects CUDA modes without the `cuda` feature")
            }
        }
    }
}

pub(crate) fn ensure_ort_ready() -> Result<(), ModelLoadError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::ExecutionMode;

    #[cfg(not(feature = "cuda"))]
    #[test]
    fn cuda_modes_require_feature() {
        let error = ExecutionMode::Cuda.validate().unwrap_err();
        assert_eq!(error.to_string(), "cuda requires the `cuda` Cargo feature");

        let error = ExecutionMode::CudaFast.validate().unwrap_err();
        assert_eq!(
            error.to_string(),
            "cuda-fast requires the `cuda` Cargo feature"
        );
    }
}
