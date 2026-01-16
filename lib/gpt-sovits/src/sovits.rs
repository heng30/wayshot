use crate::{
    BertModel, G2PW, G2pEn, GSVError, LangId, OUTPUT_AUDIO_SAMPLE_RATE,
    REFERENCE_AUDIO_SAMPLE_RATE, Result, Sampler, SamplingParams, Stream, TextProcessor, argmax,
    create_session,
};
use async_stream::stream;
use derivative::Derivative;
use derive_setters::Setters;
use ndarray::{
    Array, Array2, ArrayBase, ArrayD, ArrayView2, Axis, IxDyn, OwnedRepr, concatenate, s,
};
use ort::{
    inputs,
    session::{RunOptions, Session, SessionOutputs},
    value::{Tensor, TensorRef},
};
use rodio::{Source, buffer::SamplesBuffer, decoder::Decoder, source::UniformSourceIterator};
use std::{
    io::Cursor,
    path::{Path, PathBuf},
    time::SystemTime,
};
use tokio::fs::read;

const NUM_LAYERS: usize = 24;
const VOCAB_SIZE: usize = 1025;
const T2S_DECODER_EOS: i64 = 1024;
const MAX_DECODER_STEPS: usize = 1500;
const INITIAL_CACHE_SIZE: usize = 2048;
const CACHE_REALLOC_INCREMENT: usize = 1024;

type KvDType = f32;
type KvCache = ArrayBase<OwnedRepr<KvDType>, IxDyn>;
type KvCacheTuple = (Vec<KvCache>, Vec<KvCache>, usize);

#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct GptSoVitsModelConfig {
    pub sovits_path: PathBuf,         // custom_vits.onnx,
    pub ssl_path: PathBuf,            // ssl.onnx,
    pub t2s_encoder_path: PathBuf,    // custom_t2s_encoder.onnx,
    pub t2s_fs_decoder_path: PathBuf, // custom_t2s_fs_decoder.onnx,
    pub t2s_s_decoder_path: PathBuf,  // custom_t2s_s_decoder.onnx,
    pub bert_path: Option<PathBuf>,   // bert.onnx
    pub g2pw_path: Option<PathBuf>,   // g2pW.onnx
    pub g2p_en_path: Option<PathBuf>, // g2p_en
}

struct DecoderLoopContext {
    y_vec: Vec<i64>,
    k_caches: Vec<KvCache>,
    v_caches: Vec<KvCache>,
    prefix_len: usize,
    initial_valid_len: usize,
}

#[derive(Clone)]
pub struct ReferenceData {
    ref_seq: Array2<i64>,
    ref_bert: Array2<f32>,
    ref_audio_32k: Array2<f32>,
    ssl_content: ArrayBase<OwnedRepr<f32>, IxDyn>,
}

pub struct GptSoVitsModel {
    text_processor: TextProcessor,
    sovits: Session,
    ssl: Session,
    t2s_encoder: Session,
    t2s_fs_decoder: Session,
    t2s_s_decoder: Session,
    num_layers: usize,
    run_options: RunOptions,
}

impl GptSoVitsModel {
    pub fn new(config: GptSoVitsModelConfig) -> Result<Self> {
        let text_processor = TextProcessor::new(
            G2PW::new(config.g2pw_path)?,
            G2pEn::new(config.g2p_en_path)?,
            BertModel::new(config.bert_path)?,
        )?;

        Ok(GptSoVitsModel {
            text_processor,
            sovits: create_session(config.sovits_path)?,
            ssl: create_session(config.ssl_path)?,
            t2s_encoder: create_session(config.t2s_encoder_path)?,
            t2s_fs_decoder: create_session(config.t2s_fs_decoder_path)?,
            t2s_s_decoder: create_session(config.t2s_s_decoder_path)?,
            num_layers: NUM_LAYERS,
            run_options: RunOptions::new()?,
        })
    }

    pub async fn get_reference_data(
        &mut self,
        reference_audio_path: impl AsRef<Path>,
        ref_text: &str,
        lang_id: LangId,
    ) -> Result<ReferenceData> {
        log::info!("Processing reference audio and text: {}", ref_text);
        let ref_text = ensure_end_with_punctuation(ref_text);
        let phones = self.text_processor.get_phone_and_bert(&ref_text, lang_id)?;
        let ref_seq: Vec<i64> = phones.iter().flat_map(|p| p.1.iter().copied()).collect();
        let ref_bert = concatenate(
            Axis(0),
            &phones.iter().map(|f| f.2.view()).collect::<Vec<_>>(),
        )?;

        let ref_seq = Array2::from_shape_vec((1, ref_seq.len()), ref_seq)?;
        let (ref_audio_16k, ref_audio_32k) = read_and_resample_audio(&reference_audio_path).await?;

        // Validate reference audio length, must be at least 0.5 seconds
        if ref_audio_16k.len() < REFERENCE_AUDIO_SAMPLE_RATE as usize / 2 {
            return Err(GSVError::InternalError(format!(
                "Reference audio too short: {} samples (minimum 8000 samples required at 16kHz)",
                ref_audio_16k.len()
            )));
        }

        let ssl_content = self.process_ssl(&ref_audio_16k).await?;

        Ok(ReferenceData {
            ref_seq,
            ref_bert,
            ref_audio_32k,
            ssl_content,
        })
    }

    pub async fn synthesize(
        &mut self,
        text: &str,
        reference_data: ReferenceData,
        sampling_param: SamplingParams,
        lang_id: LangId,
    ) -> Result<impl Stream<Item = Result<Vec<f32>>> + Send + Unpin> {
        let start_time = SystemTime::now();
        let texts_and_seqs = self.text_processor.get_phone_and_bert(text, lang_id)?;
        log::debug!("g2pw and preprocess time: {:?}", start_time.elapsed()?);

        let stream = stream! {
            for (text, seq, bert) in texts_and_seqs {
                log::debug!("process: {:?}", text);
                yield self.in_stream_once_gen(&bert, &seq, &reference_data, sampling_param).await;
            }
        };

        Ok(Box::pin(stream))
    }

    async fn process_ssl(
        &mut self,
        ref_audio_16k: &Array2<f32>,
    ) -> Result<ArrayBase<OwnedRepr<f32>, IxDyn>> {
        let time = SystemTime::now();
        let ssl_output = self
            .ssl
            .run_async(
                inputs!["ref_audio_16k" => TensorRef::from_array_view(ref_audio_16k)?],
                &self.run_options,
            )?
            .await?;
        log::debug!("SSL processing time: {:?}", time.elapsed()?);
        Ok(ssl_output["ssl_content"]
            .try_extract_array::<f32>()?
            .into_owned())
    }

    async fn run_t2s_s_decoder_loop(
        &mut self,
        sampler: &mut Sampler,
        sampling_param: SamplingParams,
        ctx: DecoderLoopContext,
    ) -> Result<ArrayBase<OwnedRepr<i64>, IxDyn>> {
        let DecoderLoopContext {
            mut y_vec,
            mut k_caches,
            mut v_caches,
            prefix_len,
            initial_valid_len,
        } = ctx;

        let mut idx = 0;
        let mut valid_len = initial_valid_len;
        y_vec.reserve(INITIAL_CACHE_SIZE);

        loop {
            let mut inputs = inputs![
                "iy" => TensorRef::from_array_view(unsafe {ArrayView2::from_shape_ptr((1, y_vec.len()), y_vec.as_ptr())})?,
                "y_len" => Tensor::from_array(Array::from_vec(vec![prefix_len as i64]))?,
                "idx" => Tensor::from_array(Array::from_vec(vec![idx as i64]))?,
            ];

            for i in 0..self.num_layers {
                let k = k_caches[i].slice(s![.., 0..valid_len, ..]).to_owned();
                let v = v_caches[i].slice(s![.., 0..valid_len, ..]).to_owned();

                inputs.push((
                    format!("ik_cache_{}", i).into(),
                    Tensor::from_array(k)?.into(),
                ));
                inputs.push((
                    format!("iv_cache_{}", i).into(),
                    Tensor::from_array(v)?.into(),
                ));
            }

            let mut output = self
                .t2s_s_decoder
                .run_async(inputs, &self.run_options)?
                .await?;

            let mut logits = output["logits"].try_extract_array_mut::<f32>()?;
            let mut logits = logits
                .as_slice_mut()
                .map(|s| s.to_owned())
                .ok_or(GSVError::InternalError("Failed to get logits slice".into()))?;

            if idx < 11 {
                // Disable EOS token during first 11 steps to prevent early stopping
                if let Some(item) = logits.last_mut() {
                    *item = f32::NEG_INFINITY;
                }
            }

            y_vec.push(sampler.sample(&mut logits, &y_vec, &sampling_param));
            let argmax_value = argmax(&logits);

            // Check for reallocation and update caches
            let new_valid_len = valid_len + 1;
            if new_valid_len > k_caches[0].shape()[1] {
                for i in 0..self.num_layers {
                    let old_k = &k_caches[i];
                    let old_v = &v_caches[i];

                    let mut new_k_dims = old_k.raw_dim().clone();
                    new_k_dims[1] += CACHE_REALLOC_INCREMENT;
                    let mut new_v_dims = old_v.raw_dim().clone();
                    new_v_dims[1] += CACHE_REALLOC_INCREMENT;

                    let mut new_k = Array::zeros(new_k_dims);
                    let mut new_v = Array::zeros(new_v_dims);

                    new_k
                        .slice_mut(s![.., 0..valid_len, ..])
                        .assign(&old_k.slice(s![.., 0..valid_len, ..]));
                    new_v
                        .slice_mut(s![.., 0..valid_len, ..])
                        .assign(&old_v.slice(s![.., 0..valid_len, ..]));

                    k_caches[i] = new_k;
                    v_caches[i] = new_v;
                }
            }

            update_kv_cache(
                &mut k_caches,
                &mut v_caches,
                &output,
                valid_len,
                self.num_layers,
            )?;

            valid_len = new_valid_len;

            if idx >= MAX_DECODER_STEPS || argmax_value == T2S_DECODER_EOS {
                let mut sliced = y_vec[(y_vec.len() - idx + 1)..(y_vec.len() - 1)]
                    .iter()
                    .map(|&i| if i == T2S_DECODER_EOS { 0 } else { i })
                    .collect::<Vec<i64>>();
                sliced.push(0);
                log::debug!(
                    "t2s final len: {}, prefix_len: {}",
                    sliced.len(),
                    prefix_len
                );
                let y = ArrayD::from_shape_vec(IxDyn(&[1, 1, sliced.len()]), sliced)?;
                return Ok(y);
            }
            idx += 1;
        }
    }

    async fn in_stream_once_gen(
        &mut self,
        text_bert: &Array2<f32>,
        text_seq_vec: &[i64],
        ref_data: &ReferenceData,
        sampling_param: SamplingParams,
    ) -> Result<Vec<f32>> {
        let text_seq = ArrayView2::from_shape((1, text_seq_vec.len()), text_seq_vec)?;
        let mut sampler = Sampler::new(VOCAB_SIZE);

        let prompts = {
            let time = SystemTime::now();
            let encoder_output = self
                .t2s_encoder
                .run_async(
                    inputs!["ssl_content" => TensorRef::from_array_view(&ref_data.ssl_content)?],
                    &self.run_options,
                )?
                .await?;
            log::debug!("T2S Encoder time: {:?}", time.elapsed()?);
            encoder_output["prompts"]
                .try_extract_array::<i64>()?
                .into_owned()
        };

        let x = concatenate(Axis(1), &[ref_data.ref_seq.view(), text_seq.view()])?.to_owned();
        let bert = concatenate(
            Axis(1),
            &[
                ref_data.ref_bert.clone().permuted_axes([1, 0]).view(),
                text_bert.clone().permuted_axes([1, 0]).view(),
            ],
        )?
        .insert_axis(Axis(0))
        .to_owned();

        let (mut y_vec, _) = prompts.clone().into_raw_vec_and_offset();
        let prefix_len = y_vec.len();

        let (y_vec, k_caches, v_caches, initial_seq_len) = {
            let start_time = SystemTime::now();
            let fs_decoder_output = self
                .t2s_fs_decoder
                .run_async(
                    inputs![
                        "x" => Tensor::from_array(x)?,
                        "prompts" => TensorRef::from_array_view(&prompts)?,
                        "bert" => Tensor::from_array(bert)?,
                    ],
                    &self.run_options,
                )?
                .await?;
            log::debug!("T2S FS Decoder time: {:?}", start_time.elapsed()?);

            let logits = fs_decoder_output["logits"]
                .try_extract_array::<f32>()?
                .into_owned();
            let (k_caches, v_caches, initial_seq_len) =
                initialize_kv_caches(&fs_decoder_output, NUM_LAYERS)?;

            let (mut logits_vec, _) = logits.into_raw_vec_and_offset();
            logits_vec.pop();

            let sampling_rst = sampler.sample(&mut logits_vec, &y_vec, &sampling_param);
            y_vec.push(sampling_rst);

            (y_vec, k_caches, v_caches, initial_seq_len)
        };

        let start_time = SystemTime::now();
        let pred_semantic = self
            .run_t2s_s_decoder_loop(
                &mut sampler,
                sampling_param,
                DecoderLoopContext {
                    y_vec,
                    k_caches,
                    v_caches,
                    prefix_len,
                    initial_valid_len: initial_seq_len,
                },
            )
            .await?;
        log::debug!("T2S S Decoder all time: {:?}", start_time.elapsed()?);

        let sovits_start = SystemTime::now();
        let outputs = self
            .sovits
            .run_async(
                inputs![
                    "text_seq" => TensorRef::from_array_view(text_seq)?,
                    "pred_semantic" => TensorRef::from_array_view(&pred_semantic)?,
                    "ref_audio" => TensorRef::from_array_view(&ref_data.ref_audio_32k)?
                ],
                &self.run_options,
            )?
            .await?;
        log::debug!("SoVITS time: {:?}", sovits_start.elapsed()?);

        let output_audio = outputs["audio"].try_extract_array::<f32>()?;
        let (audio, _) = output_audio.into_owned().into_raw_vec_and_offset();
        let audio: Vec<f32> = audio.into_iter().map(|s| s.clamp(-1.0, 1.0)).collect();

        // Apply fade-in/fade-out to prevent clicks and smooth segment transitions
        let audio = apply_fade_in_out(&audio);

        Ok(audio)
    }
}

async fn read_and_resample_audio(path: impl AsRef<Path>) -> Result<(Array2<f32>, Array2<f32>)> {
    let data = Cursor::new(read(path).await?);
    let decoder = Decoder::new(data)?;
    let sample_rate = decoder.sample_rate();
    let samples = if decoder.channels() == 1 {
        decoder.collect::<Vec<_>>()
    } else {
        UniformSourceIterator::new(decoder, 1, sample_rate).collect()
    };

    let ref_audio_16k = resample_audio(&samples, sample_rate, REFERENCE_AUDIO_SAMPLE_RATE);
    let ref_audio_32k = resample_audio(&samples, sample_rate, OUTPUT_AUDIO_SAMPLE_RATE);

    Ok((
        Array2::from_shape_vec((1, ref_audio_16k.len()), ref_audio_16k)?,
        Array2::from_shape_vec((1, ref_audio_32k.len()), ref_audio_32k)?,
    ))
}

#[inline]
fn resample_audio(input: &[f32], in_rate: u32, out_rate: u32) -> Vec<f32> {
    if in_rate == out_rate {
        return input.to_owned();
    }

    UniformSourceIterator::new(SamplesBuffer::new(1, in_rate, input), 1, out_rate).collect()
}

#[inline]
fn ensure_end_with_punctuation(text: &str) -> String {
    if text.ends_with(['。', '！', '？', '；', '.', '!', '?', ';']) {
        text.to_owned()
    } else {
        format!("{text}。")
    }
}

fn initialize_kv_caches(
    fs_decoder_output: &SessionOutputs,
    num_layers: usize,
) -> Result<KvCacheTuple> {
    let k_init_first = fs_decoder_output["k_cache_0"].try_extract_array::<KvDType>()?;
    let initial_dims_dyn = k_init_first.raw_dim();
    let initial_seq_len = initial_dims_dyn[1];

    let mut large_cache_dims = initial_dims_dyn.clone();
    large_cache_dims[1] = INITIAL_CACHE_SIZE;

    let mut k_caches = Vec::with_capacity(num_layers);
    let mut v_caches = Vec::with_capacity(num_layers);

    for i in 0..num_layers {
        let k_init = fs_decoder_output[format!("k_cache_{}", i)].try_extract_array::<KvDType>()?;
        let v_init = fs_decoder_output[format!("v_cache_{}", i)].try_extract_array::<KvDType>()?;

        let mut k_large = Array::zeros(large_cache_dims.clone());
        let mut v_large = Array::zeros(large_cache_dims.clone());

        k_large
            .slice_mut(s![.., 0..initial_seq_len, ..])
            .assign(&k_init);
        v_large
            .slice_mut(s![.., 0..initial_seq_len, ..])
            .assign(&v_init);

        k_caches.push(k_large);
        v_caches.push(v_large);
    }

    Ok((k_caches, v_caches, initial_seq_len))
}

fn update_kv_cache(
    k_caches: &mut [ArrayBase<OwnedRepr<KvDType>, IxDyn>],
    v_caches: &mut [ArrayBase<OwnedRepr<KvDType>, IxDyn>],
    output: &SessionOutputs,
    valid_len: usize,
    num_layers: usize,
) -> Result<()> {
    for i in 0..num_layers {
        let inc_k_cache = output[format!("k_cache_{}", i)].try_extract_array::<KvDType>()?;
        let inc_v_cache = output[format!("v_cache_{}", i)].try_extract_array::<KvDType>()?;
        let k_new_slice = inc_k_cache.slice(s![.., valid_len, ..]);
        let v_new_slice = inc_v_cache.slice(s![.., valid_len, ..]);

        k_caches[i]
            .slice_mut(s![.., valid_len, ..])
            .assign(&k_new_slice);
        v_caches[i]
            .slice_mut(s![.., valid_len, ..])
            .assign(&v_new_slice);
    }
    Ok(())
}

/// Apply fade-in and fade-out to audio to prevent clicks and smooth segment transitions
///
/// Fade duration is 20ms at 32kHz sample rate (640 samples)
pub(crate) fn apply_fade_in_out(audio: &[f32]) -> Vec<f32> {
    const SAMPLE_RATE: u32 = OUTPUT_AUDIO_SAMPLE_RATE;
    const FADE_DURATION_MS: u32 = 20;
    const FADE_SAMPLES: usize = (SAMPLE_RATE * FADE_DURATION_MS / 1000) as usize;

    if audio.len() < FADE_SAMPLES * 2 {
        // Audio too short, apply gentle fade to entire buffer
        return audio.to_vec();
    }

    let mut result = Vec::with_capacity(audio.len());
    let fade_in_samples = FADE_SAMPLES.min(audio.len() / 2);
    let fade_out_samples = FADE_SAMPLES.min(audio.len() / 2);

    for (i, &sample) in audio.iter().enumerate() {
        let mut gain = 1.0;

        // Fade in
        if i < fade_in_samples {
            gain = i as f32 / fade_in_samples as f32;
        }
        // Fade out
        else if i >= audio.len() - fade_out_samples {
            let remaining = audio.len() - i;
            gain = remaining as f32 / fade_out_samples as f32;
        }

        result.push(sample * gain);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fade_in_out() {
        // Test with normal audio
        let audio = vec![1.0; 2000];
        let result = apply_fade_in_out(&audio);

        // Check fade-in at start
        assert!(result[0] < 0.1, "First sample should be near zero");
        assert!(result[10] > result[0], "Should be fading in");
        assert!(result[100] < 1.0, "Should still be fading in");

        // Check middle is unchanged
        let middle_idx = result.len() / 2;
        assert!((result[middle_idx] - 1.0).abs() < 0.01, "Middle should be at full gain");

        // Check fade-out at end
        let last = *result.last().unwrap();
        assert!(last < 0.1, "Last sample should be near zero");
        assert!(result[result.len() - 100] < 1.0, "Should be fading out");
    }

    #[test]
    fn test_short_audio() {
        // Test with very short audio (less than fade duration)
        let audio = vec![0.5; 100];
        let result = apply_fade_in_out(&audio);

        // Should return original audio without modification
        assert_eq!(result.len(), audio.len());
        assert_eq!(result, audio);
    }

    #[test]
    fn test_fade_symmetry() {
        // Test that fade-in and fade-out are symmetric
        let audio = vec![1.0; 2000];
        let result = apply_fade_in_out(&audio);

        // Compare symmetric positions
        let fade_samples = 640; // 20ms at 32kHz
        for i in 0..fade_samples {
            let fade_in_val = result[i];
            let fade_out_val = result[result.len() - 1 - i];
            let diff = (fade_in_val - fade_out_val).abs();

            // Should be approximately symmetric (allowing small numerical differences)
            assert!(diff < 0.01, "Fade asymmetry at index {}: {} vs {}", i, fade_in_val, fade_out_val);
        }
    }
}
