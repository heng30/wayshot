//! VAE-only test: decode a pre-computed latent and dump output waveform.
use burn::prelude::*;
use burn::tensor::TensorData;
use voxcpm_rs::VoxCPM;

#[cfg(feature = "wgpu")]
type B = burn::backend::Wgpu<f32, i32>;
#[cfg(all(not(feature = "wgpu"), feature = "cpu"))]
type B = burn::backend::NdArray<f32>;

fn read_npy_f32(path: &str) -> (Vec<usize>, Vec<f32>) {
    let bytes = std::fs::read(path).unwrap();
    assert_eq!(&bytes[..6], b"\x93NUMPY");
    let header_len = u16::from_le_bytes([bytes[8], bytes[9]]) as usize;
    let header = std::str::from_utf8(&bytes[10..10 + header_len]).unwrap();
    let shape_start = header.find("'shape': (").unwrap() + "'shape': (".len();
    let shape_end = shape_start + header[shape_start..].find(')').unwrap();
    let shape: Vec<usize> = header[shape_start..shape_end]
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.parse().unwrap())
        .collect();
    let data = &bytes[10 + header_len..];
    let floats: Vec<f32> = data
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    (shape, floats)
}

fn main() {
    let model_dir = std::env::args().nth(1).unwrap_or_else(|| "/home/nipah/dev/ai_space/VoxCPM2".into());
    let feat_path = std::env::args().nth(2).unwrap_or_else(|| "/tmp/py_feat.npy".into());
    let device = Default::default();

    let model: VoxCPM<B> = VoxCPM::from_local(&model_dir, &device).unwrap();
    let (shape, data) = read_npy_f32(&feat_path);
    eprintln!("latent shape: {shape:?}");
    let feat: Tensor<B, 3> = Tensor::from_data(TensorData::new(data, shape.clone()), &device);
    let wav = model.audio_vae_decode(feat);
    let dims = wav.dims();
    eprintln!("wav dims: {dims:?}");
    let vec = wav.into_data().as_slice::<f32>().unwrap().to_vec();
    let peak = vec.iter().map(|x| x.abs()).fold(0f32, f32::max);
    let mean = vec.iter().map(|x| x.abs()).sum::<f32>() / vec.len() as f32;
    eprintln!("rust wav: peak={peak}, mean={mean}");
    let bytes: Vec<u8> = vec.iter().flat_map(|x| x.to_le_bytes()).collect();
    std::fs::write("/tmp/rust_wav.bin", bytes).unwrap();
}
