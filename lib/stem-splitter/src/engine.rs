use crate::{
    dsp::{IstftBatchWorkspace, istft_cac_stereo_sources_add_into, stft_cac_stereo_centered_into},
    error::{Result, StemError},
};

use ndarray::Array3;
use once_cell::sync::OnceCell;
use ort::session::{
    Session,
    builder::{GraphOptimizationLevel, SessionBuilder},
};
use std::{
    path::Path,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

const DEMUCS_T: usize = 343_980;
const DEMUCS_F: usize = 2048;
const DEMUCS_FRAMES: usize = 336;
const DEMUCS_NFFT: usize = 4096;
const DEMUCS_HOP: usize = 1024;
static ORT_INIT: OnceCell<()> = OnceCell::new();

struct DemucsRawOutput {
    num_sources: usize,
    data_time: Vec<f32>,
    data_freq: Vec<f32>,
    time_max: f32,
    freq_max: f32,
}

#[derive(Default)]
struct InferenceScratch {
    time_branch: Vec<f32>,
    spec_branch: Vec<f32>,
}

impl InferenceScratch {
    fn with_demucs_capacity() -> Self {
        Self {
            time_branch: Vec::with_capacity(2 * DEMUCS_T),
            spec_branch: Vec::with_capacity(4 * DEMUCS_F * DEMUCS_FRAMES),
        }
    }

    fn fill_time_branch(&mut self, left: &[f32], right: &[f32]) {
        self.time_branch.clear();
        self.time_branch.extend_from_slice(left);
        self.time_branch.extend_from_slice(right);
    }
}

fn use_positional_inputs(input_names: &[&str]) -> bool {
    matches!(input_names, ["input", "x"])
}

fn inspect_engine_io(session: &Session) -> Result<bool> {
    let input_names: Vec<&str> = session.inputs().iter().map(|input| input.name()).collect();
    let output_names: Vec<&str> = session
        .outputs()
        .iter()
        .map(|output| output.name())
        .collect();

    if !output_names.contains(&"output") {
        return Err(StemError::Inference(
            "Model missing output 'output' (freq domain)".into(),
        ));
    }
    if !output_names.contains(&"add_67") {
        return Err(StemError::Inference(
            "Model missing output 'add_67' (time domain)".into(),
        ));
    }

    Ok(use_positional_inputs(&input_names))
}

fn commit_cpu_session(model_path: &std::path::Path, num_threads: usize) -> Result<Session> {
    Ok(SessionBuilder::new()?
        .with_optimization_level(GraphOptimizationLevel::Level3)?
        .with_intra_threads(num_threads.max(1))?
        .commit_from_file(model_path)?)
}

fn commit_ep_session(model_path: &std::path::Path, num_threads: usize) -> Result<Session> {
    let mut builder = SessionBuilder::new()?
        .with_optimization_level(GraphOptimizationLevel::Level3)?
        .with_intra_threads(num_threads.max(1))?;

    #[cfg(all(feature = "cuda", any(target_os = "linux", target_os = "windows")))]
    {
        use ort::ep::ExecutionProvider;
        let ep = ort::execution_providers::CUDAExecutionProvider::default();
        if ep.is_available().unwrap_or(false) {
            builder = builder.with_execution_providers(vec![ep.build()])?;
        }
    }

    #[cfg(all(feature = "coreml", target_os = "macos"))]
    {
        use ort::ep::ExecutionProvider;
        let ep = ort::execution_providers::CoreMLExecutionProvider::default();
        if ep.is_available().unwrap_or(false) {
            builder = builder.with_execution_providers(vec![ep.build()])?;
        }
    }

    #[cfg(all(feature = "directml", target_os = "windows"))]
    {
        use ort::ep::ExecutionProvider;
        let ep = ort::execution_providers::DirectMLExecutionProvider::default();
        if ep.is_available().unwrap_or(false) {
            builder = builder.with_execution_providers(vec![ep.build()])?;
        }
    }

    #[cfg(feature = "onednn")]
    {
        use ort::ep::ExecutionProvider;
        let ep = ort::execution_providers::OneDNNExecutionProvider::default();
        if ep.is_available().unwrap_or(false) {
            builder = builder.with_execution_providers(vec![ep.build()])?;
        }
    }

    #[cfg(feature = "xnnpack")]
    {
        use ort::ep::ExecutionProvider;
        let ep = ort::execution_providers::XNNPACKExecutionProvider::default();
        if ep.is_available().unwrap_or(false) {
            builder = builder.with_execution_providers(vec![ep.build()])?;
        }
    }

    Ok(builder.commit_from_file(model_path)?)
}

fn probe_session_health(session: &mut Session, use_positional: bool) -> Result<()> {
    let (left, right) = build_preload_probe_input();
    let mut scratch = InferenceScratch::with_demucs_capacity();
    let (t, f_bins, frames) = prepare_demucs_inputs(&left, &right, &mut scratch)?;

    let (out_time, out_freq) = run_demucs_raw_from_inputs(
        session,
        use_positional,
        t,
        f_bins,
        frames,
        &scratch.time_branch,
        &scratch.spec_branch,
    )?;
    let raw = decode_demucs_outputs(out_time, out_freq, t, f_bins, frames)?;
    ensure_output_is_not_near_silent(&left, &right, &raw)
}

fn try_create_best_session(model_path: &std::path::Path) -> Result<Session> {
    let num_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);

    match commit_ep_session(model_path, num_threads) {
        Ok(mut session) => {
            let use_pos = inspect_engine_io(&session)?;
            if let Ok(()) = probe_session_health(&mut session, use_pos) {
                return Ok(session);
            }
        }
        Err(_) => {}
    }

    commit_cpu_session(model_path, num_threads)
}

fn prepare_demucs_inputs(
    left: &[f32],
    right: &[f32],
    scratch: &mut InferenceScratch,
) -> Result<(usize, usize, usize)> {
    if left.len() != right.len() {
        return Err(StemError::Inference("L/R length mismatch".into()));
    }
    let t = left.len();
    if t != DEMUCS_T {
        return Err(StemError::Inference(format!(
            "Bad window length {} (expected {})",
            t, DEMUCS_T
        )));
    }

    scratch.fill_time_branch(left, right);

    let (f_bins, frames) = stft_cac_stereo_centered_into(
        left,
        right,
        DEMUCS_NFFT,
        DEMUCS_HOP,
        &mut scratch.spec_branch,
    );
    if f_bins != DEMUCS_F || frames != DEMUCS_FRAMES {
        return Err(StemError::Dsp(format!(
            "Spec dims mismatch: got F={},Frames={}, expected F={},Frames={}",
            f_bins, frames, DEMUCS_F, DEMUCS_FRAMES
        )));
    }

    Ok((t, f_bins, frames))
}

fn run_demucs_raw_from_inputs(
    session: &mut Session,
    use_positional: bool,
    t: usize,
    f_bins: usize,
    frames: usize,
    time_branch: &[f32],
    spec_branch: &[f32],
) -> Result<(ort::value::DynValue, ort::value::DynValue)> {
    let time_value = ort::value::TensorRef::from_array_view(([1usize, 2, t], time_branch))?;
    let spec_value =
        ort::value::TensorRef::from_array_view(([1usize, 4, f_bins, frames], spec_branch))?;

    let mut outputs = if use_positional {
        session.run(ort::inputs![time_value, spec_value])?
    } else {
        session.run(ort::inputs!["input" => time_value, "x" => spec_value])?
    };

    let out_freq = outputs.remove("output").ok_or_else(|| {
        StemError::Inference("Model did not return 'output' (freq domain)".into())
    })?;
    let out_time = outputs.remove("add_67").ok_or_else(|| {
        StemError::Inference("Model did not return 'add_67' (time domain)".into())
    })?;

    Ok((out_time, out_freq))
}

fn decode_demucs_outputs(
    out_time: ort::value::DynValue,
    out_freq: ort::value::DynValue,
    t: usize,
    f_bins: usize,
    frames: usize,
) -> Result<DemucsRawOutput> {
    let (shape_time, data_time) = out_time.try_extract_tensor::<f32>()?;

    if shape_time.len() != 4
        || shape_time[0] != 1
        || shape_time[2] != 2
        || shape_time[3] != t as i64
    {
        return Err(StemError::Inference(format!(
            "Unexpected time output shape: {:?}, expected [1, sources, 2, {}]",
            shape_time, t
        )));
    }
    let num_sources = shape_time[1] as usize;

    let (shape_freq, data_freq) = out_freq.try_extract_tensor::<f32>()?;

    if shape_freq.len() != 5
        || shape_freq[0] != 1
        || shape_freq[1] != num_sources as i64
        || shape_freq[2] != 4
        || shape_freq[3] != f_bins as i64
        || shape_freq[4] != frames as i64
    {
        return Err(StemError::Inference(format!(
            "Unexpected freq output shape: {:?}, expected [1, {}, 4, {}, {}]",
            shape_freq, num_sources, f_bins, frames
        )));
    }

    let time_max = data_time.iter().map(|x| x.abs()).fold(0.0f32, f32::max);
    let freq_max = data_freq.iter().map(|x| x.abs()).fold(0.0f32, f32::max);

    Ok(DemucsRawOutput {
        num_sources,
        data_time: data_time.to_vec(),
        data_freq: data_freq.to_vec(),
        time_max,
        freq_max,
    })
}

fn output_is_near_silent(time_max: f32, freq_max: f32) -> bool {
    time_max < 1e-6 && freq_max < 1e-3
}

fn input_is_near_silent(left: &[f32], right: &[f32]) -> bool {
    let left_max = left.iter().map(|x| x.abs()).fold(0.0f32, f32::max);
    let right_max = right.iter().map(|x| x.abs()).fold(0.0f32, f32::max);
    left_max.max(right_max) < 1e-4
}

fn ensure_output_is_not_near_silent(
    left: &[f32],
    right: &[f32],
    raw: &DemucsRawOutput,
) -> Result<()> {
    if !input_is_near_silent(left, right) && output_is_near_silent(raw.time_max, raw.freq_max) {
        return Err(StemError::Inference(format!(
            "near-silent execution output (time_max={:.3e}, freq_max={:.3e})",
            raw.time_max, raw.freq_max
        )));
    }
    Ok(())
}

fn build_preload_probe_input() -> (Vec<f32>, Vec<f32>) {
    use std::f32::consts::TAU;

    let sample_rate = 44_100.0f32;
    let mut left = Vec::with_capacity(DEMUCS_T);
    let mut right = Vec::with_capacity(DEMUCS_T);

    for i in 0..DEMUCS_T {
        let t = i as f32 / sample_rate;
        left.push(0.22 * (TAU * 220.0 * t).sin() + 0.11 * (TAU * 660.0 * t).sin());
        right.push(0.20 * (TAU * 330.0 * t).sin() + 0.09 * (TAU * 880.0 * t).cos());
    }

    (left, right)
}

fn postprocess_demucs_output(
    mut raw: DemucsRawOutput,
    left: &[f32],
    right: &[f32],
    istft_ws: &mut IstftBatchWorkspace,
) -> Result<Array3<f32>> {
    let t = left.len();

    ensure_output_is_not_near_silent(left, right, &raw)?;

    let source_specs: Vec<&[f32]> = (0..raw.num_sources)
        .map(|src| {
            let src_freq_offset = src * 4 * DEMUCS_F * DEMUCS_FRAMES;
            &raw.data_freq[src_freq_offset..src_freq_offset + 4 * DEMUCS_F * DEMUCS_FRAMES]
        })
        .collect();

    istft_cac_stereo_sources_add_into(
        &source_specs,
        DEMUCS_F,
        DEMUCS_FRAMES,
        DEMUCS_NFFT,
        DEMUCS_HOP,
        t,
        istft_ws,
        &mut raw.data_time,
    );

    Ok(Array3::from_shape_vec(
        (raw.num_sources, 2, t),
        raw.data_time,
    )?)
}

pub struct EngineState {
    session: Mutex<Session>,
    use_positional: bool,
    input_scratch: Mutex<InferenceScratch>,
    istft_scratch: Mutex<IstftBatchWorkspace>,
    ep_fallback_used: AtomicBool,
    model_path: std::path::PathBuf,
    num_threads: usize,
}

impl EngineState {
    pub fn new(model_path: &Path) -> Result<Self> {
        ORT_INIT.get_or_try_init::<_, StemError>(|| {
            let _ = ort::init().commit();
            Ok(())
        })?;

        let session = try_create_best_session(model_path)?;
        let use_pos = inspect_engine_io(&session)?;

        let num_threads = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);

        Ok(Self {
            session: Mutex::new(session),
            use_positional: use_pos,
            input_scratch: Mutex::new(InferenceScratch::with_demucs_capacity()),
            istft_scratch: Mutex::new(IstftBatchWorkspace::default()),
            ep_fallback_used: AtomicBool::new(false),
            model_path: model_path.to_path_buf(),
            num_threads,
        })
    }

    pub fn run_window_demucs(&self, left: &[f32], right: &[f32]) -> Result<Array3<f32>> {
        if left.len() != right.len() {
            return Err(StemError::Inference("L/R length mismatch".into()));
        }
        if left.len() != DEMUCS_T {
            return Err(StemError::Inference(format!(
                "Bad window length {} (expected {})",
                left.len(),
                DEMUCS_T
            )));
        }

        match self.run_window_demucs_once(left, right) {
            Ok(out) => Ok(out),
            Err(e) => {
                let error_text = e.to_string();
                let forced_non_cpu_ep = std::env::var("STEMMER_EP_FORCE")
                    .map(|v| !v.trim().is_empty() && v.trim().to_ascii_lowercase() != "cpu")
                    .unwrap_or(false);
                let fallback_already_used = self.ep_fallback_used.load(Ordering::SeqCst);

                if !error_text.contains("near-silent") || forced_non_cpu_ep || fallback_already_used
                {
                    return Err(e);
                }

                self.ep_fallback_used.store(true, Ordering::SeqCst);

                log::warn!("Runtime EP output was near-silent; switching to CPU and retrying");

                let cpu_session = commit_cpu_session(&self.model_path, self.num_threads)?;
                {
                    let mut session = self.session.lock().expect("session poisoned");
                    *session = cpu_session;
                }

                match self.run_window_demucs_once(left, right) {
                    Ok(out) => {
                        log::debug!("Runtime fallback succeeded: CPU is now active");
                        Ok(out)
                    }
                    Err(retry_error) => {
                        log::warn!("Runtime fallback to CPU failed: {}", retry_error);
                        Err(retry_error)
                    }
                }
            }
        }
    }

    fn run_window_demucs_once(&self, left: &[f32], right: &[f32]) -> Result<Array3<f32>> {
        let raw = {
            let mut scratch = self.input_scratch.lock().expect("input scratch poisoned");
            let (t, f_bins, frames) = prepare_demucs_inputs(left, right, &mut scratch)?;

            let mut session = self.session.lock().expect("session poisoned");
            let (out_time, out_freq) = run_demucs_raw_from_inputs(
                &mut session,
                self.use_positional,
                t,
                f_bins,
                frames,
                &scratch.time_branch,
                &scratch.spec_branch,
            )?;
            drop(session);
            drop(scratch);

            decode_demucs_outputs(out_time, out_freq, t, f_bins, frames)?
        };

        let mut istft_ws = self.istft_scratch.lock().expect("iSTFT scratch poisoned");
        postprocess_demucs_output(raw, left, right, &mut istft_ws)
    }
}
