use crate::{FunAsrError, Result};
use candle_core::{DType, Device};

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

pub fn get_dtype(dtype: Option<DType>, cfg_dtype: &str) -> Result<DType> {
    if let Some(d) = dtype {
        return Ok(d);
    }

    match cfg_dtype.to_lowercase().as_str() {
        "f32" | "float32" => Ok(DType::F32),
        "f16" | "float16" => Ok(DType::F16),
        "bf16" | "bfloat16" => {
            cfg_if::cfg_if! {
                if #[cfg(any(feature = "cuda", feature = "metal"))]
                {
                    Ok(DType::BF16)
                } else {
                    Ok(DType::F32)
                }
            }
        }
        _ => Err(FunAsrError::Config(format!(
            "Unsupported dtype: {cfg_dtype}"
        ))),
    }
}
