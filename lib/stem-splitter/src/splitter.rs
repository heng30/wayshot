use crate::{
    audio_data::{AudioData, audio_frame_count, fill_stereo_window},
    dsp::{IstftStereoWorkspace, istft_cac_stereo_into, stft_cac_stereo_centered_into},
    engine::EngineState,
    error::{Result, StemError},
    model::ModelHandle,
};
use std::collections::BTreeMap;

const DEMUCS_T: usize = 343_980;
const DEMUCS_NFFT: usize = 4096;
const DEMUCS_HOP: usize = 1024;

pub struct SplitResult {
    pub stems: BTreeMap<String, AudioData>,
}

pub fn split(
    audio: &AudioData,
    handle: &ModelHandle,
    on_progress: Option<&dyn Fn(f32)>,
) -> Result<SplitResult> {
    let engine = handle.engine();
    let manifest = &handle.manifest;
    let stem_names = manifest.default_stems();
    let n_sources = stem_names.len();

    let channels = audio.channels.max(1);
    let total_frames = audio_frame_count(&audio.samples, audio.channels);

    let mut acc: Vec<Vec<f32>> = (0..n_sources)
        .map(|_| vec![0.0f32; 2 * total_frames])
        .collect();

    let mut ws = IstftStereoWorkspace::default();

    split_windows_into_accumulators(
        &audio.samples,
        channels,
        total_frames,
        n_sources,
        &mut acc,
        &mut ws,
        handle,
        engine,
        on_progress,
    )?;

    let mut stems = BTreeMap::new();
    for (i, name) in stem_names.into_iter().enumerate() {
        stems.insert(
            name,
            AudioData {
                samples: std::mem::take(&mut acc[i]),
                sample_rate: audio.sample_rate,
                channels: 2,
            },
        );
    }

    Ok(SplitResult { stems })
}

fn split_windows_into_accumulators(
    samples: &[f32],
    channels: u16,
    total_frames: usize,
    n_sources: usize,
    acc: &mut [Vec<f32>],
    ws: &mut IstftStereoWorkspace,
    handle: &ModelHandle,
    engine: &EngineState,
    on_progress: Option<&dyn Fn(f32)>,
) -> Result<()> {
    let mut window_l = vec![0.0f32; DEMUCS_T];
    let mut window_r = vec![0.0f32; DEMUCS_T];

    let n_windows = if total_frames == 0 {
        0
    } else {
        (total_frames + DEMUCS_T - 1) / DEMUCS_T
    };

    let mut spec_buf = Vec::new();
    let mut overlap_count = vec![0u32; total_frames];

    for wi in 0..n_windows {
        if handle.is_cancelled() {
            return Err(StemError::Cancelled);
        }

        let start = wi * DEMUCS_T;
        let end = (start + DEMUCS_T).min(total_frames);
        let len = end - start;

        fill_stereo_window(samples, channels, start, &mut window_l, &mut window_r);

        let out = engine.run_window_demucs(&window_l[..DEMUCS_T], &window_r[..DEMUCS_T])?;
        let out_shape = out.shape();
        if out_shape[0] != n_sources || out_shape[1] != 2 {
            return Err(StemError::Inference(format!(
                "Unexpected output shape: {:?}",
                out_shape
            )));
        }

        let out_data = out
            .as_slice()
            .ok_or_else(|| StemError::Inference("Output array is not contiguous".into()))?;

        let source_time: Vec<&[f32]> = (0..n_sources)
            .map(|src| {
                let src_offset = src * 2 * DEMUCS_T;
                &out_data[src_offset..src_offset + 2 * DEMUCS_T]
            })
            .collect();

        for src_idx in 0..n_sources {
            let src_time = source_time[src_idx];
            let src_l = &src_time[..DEMUCS_T];
            let src_r = &src_time[DEMUCS_T..];

            spec_buf.clear();
            let (f_bins, frames) =
                stft_cac_stereo_centered_into(src_l, src_r, DEMUCS_NFFT, DEMUCS_HOP, &mut spec_buf);

            let mut istft_l = vec![0.0f32; DEMUCS_T];
            let mut istft_r = vec![0.0f32; DEMUCS_T];
            istft_cac_stereo_into(
                &spec_buf,
                f_bins,
                frames,
                DEMUCS_NFFT,
                DEMUCS_HOP,
                DEMUCS_T,
                ws,
                &mut istft_l,
                &mut istft_r,
            );

            let acc_data = &mut acc[src_idx];
            for i in 0..len {
                acc_data[2 * (start + i)] += istft_l[i];
                acc_data[2 * (start + i) + 1] += istft_r[i];
            }
        }

        for i in 0..len {
            overlap_count[start + i] += 1;
        }

        log::debug!("Window {}/{}: frames {}..{}", wi + 1, n_windows, start, end);

        if let Some(cb) = on_progress {
            cb((wi + 1) as f32 / n_windows as f32);
        }
    }

    for src_idx in 0..n_sources {
        let acc_data = &mut acc[src_idx];
        for i in 0..total_frames {
            let count = overlap_count[i];
            if count > 1 {
                acc_data[2 * i] /= count as f32;
                acc_data[2 * i + 1] /= count as f32;
            }
        }
    }

    Ok(())
}
