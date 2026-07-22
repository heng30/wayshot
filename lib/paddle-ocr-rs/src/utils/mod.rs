pub mod img_utils;
pub mod tensor_utils;
pub mod interpolate;
pub mod loc_parser;

use candle_core::DType;
use candle_core::Device;

/// Get device based on available features and user preference
pub fn get_device(device: Option<&Device>) -> Device {
    match device {
        Some(d) => d.clone(),
        None => {
            #[cfg(feature = "cuda")]
            {
                Device::new_cuda(0).unwrap_or(Device::Cpu)
            }
            #[cfg(all(not(feature = "cuda"), feature = "metal"))]
            {
                Device::new_metal(0).unwrap_or(Device::Cpu)
            }
            #[cfg(all(not(feature = "cuda"), not(feature = "metal")))]
            {
                Device::Cpu
            }
        }
    }
}

/// Get GPU SM architecture for BF16 support check
pub fn get_gpu_sm_arch() -> Result<f32, crate::Error> {
    use std::process::Command;
    let output = Command::new("nvidia-smi")
        .arg("--query-gpu=compute_cap")
        .arg("--format=csv,noheader")
        .output()?;
    if !output.status.success() {
        return Err(crate::Error::Gpu(format!(
            "nvidia-smi failed with status: {}\nError: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    let output_str = String::from_utf8_lossy(&output.stdout);
    let output_str = output_str.trim();
    output_str.parse::<f32>().map_err(|_| {
        crate::Error::Gpu(format!("Failed to parse GPU SM architecture: {}", output_str))
    })
}

/// Get dtype based on user preference and config
pub fn get_dtype(dtype: Option<DType>, cfg_dtype: &str) -> DType {
    match dtype {
        Some(d) => d,
        None => {
            #[cfg(feature = "cuda")]
            {
                match cfg_dtype {
                    "float32" | "float" => DType::F32,
                    "float64" | "double" => DType::F64,
                    "float16" => DType::F16,
                    "bfloat16" => {
                        let arch = get_gpu_sm_arch();
                        match arch {
                            Err(_) => DType::F16,
                            Ok(a) => {
                                // NVIDIA GPUs with SM >= 8.0 support BF16
                                if a >= 8.0 { DType::BF16 } else { DType::F16 }
                            }
                        }
                    }
                    "uint8" => DType::U8,
                    "int8" | "int16" | "int32" | "int64" => DType::I64,
                    _ => DType::F32,
                }
            }
            #[cfg(not(feature = "cuda"))]
            {
                match cfg_dtype {
                    "float32" | "float" => DType::F32,
                    "float64" | "double" => DType::F64,
                    "float16" | "bfloat16" => DType::F16, // BF16 has issues on CPU
                    "uint8" => DType::U8,
                    "int8" | "int16" | "int32" | "int64" => DType::I64,
                    _ => DType::F32,
                }
            }
        }
    }
}

/// Find all files with a specific extension in a directory
pub fn find_type_files(path: &str, extension_type: &str) -> Result<Vec<String>, crate::Error> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let file_path = entry.path();
        if file_path.is_file() {
            if let Some(extension) = file_path.extension() {
                if extension == extension_type {
                    files.push(file_path.to_string_lossy().to_string());
                }
            }
        }
    }
    Ok(files)
}

/// Round a number to nearest multiple of factor
pub fn round_by_factor(num: u32, factor: u32) -> u32 {
    let round = (num as f32 / factor as f32).round() as u32;
    round * factor
}

/// Floor a number to nearest multiple of factor
pub fn floor_by_factor(num: f32, factor: u32) -> u32 {
    let floor = (num / factor as f32).floor() as u32;
    floor * factor
}

/// Ceil a number to nearest multiple of factor
pub fn ceil_by_factor(num: f32, factor: u32) -> u32 {
    let ceil = (num / factor as f32).ceil() as u32;
    ceil * factor
}