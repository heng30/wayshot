use crate::error::{FunAsrError, Result};
use candle_core::{DType, Device};
use std::path::Path;

/// Get the device to use (CPU or CUDA/Metal)
pub fn get_device(device: Option<&Device>) -> Device {
    device.cloned().unwrap_or_else(|| {
        #[cfg(feature = "cuda")]
        {
            Device::new_cuda(0).unwrap_or(Device::Cpu)
        }
        #[cfg(feature = "metal")]
        {
            Device::new_metal(0).unwrap_or(Device::Cpu)
        }
        #[cfg(not(any(feature = "cuda", feature = "metal")))]
        Device::Cpu
    })
}

/// Get the dtype to use
pub fn get_dtype(dtype: Option<DType>, cfg_dtype: &str) -> Result<DType> {
    if let Some(d) = dtype {
        return Ok(d);
    }

    match cfg_dtype.to_lowercase().as_str() {
        "f32" | "float32" => Ok(DType::F32),
        "f16" | "float16" => Ok(DType::F16),
        "bf16" | "bfloat16" => {
            // BF16 is not fully supported on CPU, use F32 instead
            Ok(DType::F32)
        }
        _ => Err(FunAsrError::Config(format!(
            "Unsupported dtype: {}",
            cfg_dtype
        ))),
    }
}

/// Find files with a specific extension in a directory
pub fn find_type_files(path: &str, ext: &str) -> Result<Vec<String>> {
    let path_obj = Path::new(path);
    if !path_obj.exists() {
        return Err(FunAsrError::NotFound(format!("Path not found: {}", path)));
    }

    let mut files = Vec::new();
    if path_obj.is_dir() {
        for entry in std::fs::read_dir(path_obj)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() {
                if let Some(extension) = path.extension() {
                    if extension == ext {
                        if let Some(file_path) = path.to_str() {
                            files.push(file_path.to_string());
                        }
                    }
                }
            }
        }
    } else if path_obj.is_file() {
        if let Some(extension) = path_obj.extension() {
            if extension == ext {
                files.push(path.to_string());
            }
        }
    }

    if files.is_empty() {
        return Err(FunAsrError::NotFound(format!(
            "No {} files found in {}",
            ext, path
        )));
    }

    Ok(files)
}
