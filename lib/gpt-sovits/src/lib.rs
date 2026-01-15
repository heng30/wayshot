mod error;
mod logits_sampler;
mod onnx_builder;
mod text;

use {
    async_stream::stream,
    log::{debug, info},
    ndarray::{
        Array, Array2, ArrayBase, ArrayD, ArrayView2, Axis, IxDyn, OwnedRepr, concatenate, s,
    },
    ort::{
        inputs,
        session::{RunOptions, Session, SessionOutputs},
        value::{Tensor, TensorRef},
    },
    rodio::{Source, buffer::SamplesBuffer, decoder::Decoder, source::UniformSourceIterator},
    std::{io::Cursor, path::Path, time::SystemTime},
    tokio::fs::read,
};
pub use {
    error::*,
    futures::{Stream, StreamExt},
    logits_sampler::*,
    onnx_builder::*,
    text::*,
};

const T2S_DECODER_EOS: i64 = 1024;
const VOCAB_SIZE: usize = 1025;
const NUM_LAYERS: usize = 24;
const MAX_DECODER_STEPS: usize = 1500;

type KvDType = f32;
/// Type alias for KV cache arrays
type KvCache = ArrayBase<OwnedRepr<KvDType>, IxDyn>;
/// Type alias for KV cache tuple
type KvCacheTuple = (Vec<KvCache>, Vec<KvCache>, usize);

/// Configuration for GPT-SoVITS model paths
pub struct GptSoVitsModelConfig<P> {
    pub sovits_path: P,
    pub ssl_path: P,
    pub t2s_encoder_path: P,
    pub t2s_fs_decoder_path: P,
    pub t2s_s_decoder_path: P,
    pub bert_path: Option<P>,
    pub g2pw_path: Option<P>,
    pub g2p_en_path: Option<P>,
}

/// Context for T2S decoder loop
struct DecoderLoopContext {
    y_vec: Vec<i64>,
    k_caches: Vec<KvCache>,
    v_caches: Vec<KvCache>,
    prefix_len: usize,
    initial_valid_len: usize,
}

/// Helper function to create a single-element i64 tensor
fn create_scalar_i64_tensor(value: i64) -> Result<Tensor<i64>, GSVError> {
    Tensor::from_array(Array::from_vec(vec![value]))
        .map_err(|e| GSVError::InternalError(format!("Failed to create scalar tensor: {}", e)))
}

/// Update KV cache with new data
fn update_kv_cache(
    k_caches: &mut [ArrayBase<OwnedRepr<KvDType>, IxDyn>],
    v_caches: &mut [ArrayBase<OwnedRepr<KvDType>, IxDyn>],
    output: &SessionOutputs,
    valid_len: usize,
    num_layers: usize,
) -> Result<(), GSVError> {
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

#[derive(Clone)]
pub struct ReferenceData {
    ref_seq: Array2<i64>,
    ref_bert: Array2<f32>,
    ref_audio_32k: Array2<f32>,
    ssl_content: ArrayBase<OwnedRepr<f32>, IxDyn>,
}

impl AsRef<Self> for ReferenceData {
    fn as_ref(&self) -> &Self {
        self
    }
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

// --- KV Cache Configuration ---
/// Initial size for the sequence length of the KV cache.
const INITIAL_CACHE_SIZE: usize = 2048;
/// How much to increment the KV cache size by when reallocating.
const CACHE_REALLOC_INCREMENT: usize = 1024;

impl GptSoVitsModel {
    /// Creates a new TTS instance from configuration
    pub fn from_config<P>(config: GptSoVitsModelConfig<P>) -> Result<Self, GSVError>
    where
        P: AsRef<Path>,
    {
        info!("Initializing TTSModel with ONNX sessions");

        let g2pw = G2PW::new(config.g2pw_path)?;

        let text_processor = TextProcessor::new(
            g2pw,
            G2pEn::new(config.g2p_en_path)?,
            BertModel::new(config.bert_path)?,
        )?;

        Ok(GptSoVitsModel {
            text_processor,
            sovits: create_onnx_cpu_session(config.sovits_path)?,
            ssl: create_onnx_cpu_session(config.ssl_path)?,
            t2s_encoder: create_onnx_cpu_session(config.t2s_encoder_path)?,
            t2s_fs_decoder: create_onnx_cpu_session(config.t2s_fs_decoder_path)?,
            t2s_s_decoder: create_onnx_cpu_session(config.t2s_s_decoder_path)?,
            num_layers: NUM_LAYERS,
            run_options: RunOptions::new()?,
        })
    }

    /// Creates a new TTS instance
    /// bert_path, g2pw_path and g2p_en_path can be None
    /// if bert path is none, the speech speed in chinese may become worse
    /// if g2pw path is none, the chinese speech quality may be worse
    /// g2p_en is still experimental, english speak quality may not be better because of bugs
    ///
    /// Note: Consider using `from_config` with `GptSoVitsModelConfig` for better code organization.
    #[allow(clippy::too_many_arguments)]
    pub fn new<P>(
        sovits_path: P,
        ssl_path: P,
        t2s_encoder_path: P,
        t2s_fs_decoder_path: P,
        t2s_s_decoder_path: P,
        bert_path: Option<P>,
        g2pw_path: Option<P>,
        g2p_en_path: Option<P>,
    ) -> Result<Self, GSVError>
    where
        P: AsRef<Path>,
    {
        Self::from_config(GptSoVitsModelConfig {
            sovits_path,
            ssl_path,
            t2s_encoder_path,
            t2s_fs_decoder_path,
            t2s_s_decoder_path,
            bert_path,
            g2pw_path,
            g2p_en_path,
        })
    }

    pub async fn get_reference_data<P, S>(
        &mut self,
        reference_audio_path: P,
        ref_text: S,
        lang_id: LangId,
    ) -> Result<ReferenceData, GSVError>
    where
        P: AsRef<Path>,
        S: AsRef<str>,
    {
        info!("Processing reference audio and text: {}", ref_text.as_ref());
        let ref_text = ensure_punctuation(ref_text);
        let phones = self.text_processor.get_phone_and_bert(&ref_text, lang_id)?;

        // Flatten phone sequences
        let ref_seq: Vec<i64> = phones.iter().flat_map(|p| p.1.iter().copied()).collect();

        // Concatenate BERT features along dimension 0
        let ref_bert = concatenate(
            Axis(0),
            &phones.iter().map(|f| f.2.view()).collect::<Vec<_>>(),
        )?;

        let ref_seq = Array2::from_shape_vec((1, ref_seq.len()), ref_seq)?;
        let (ref_audio_16k, ref_audio_32k) = read_and_resample_audio(&reference_audio_path).await?;
        let ssl_content = self.process_ssl(&ref_audio_16k).await?;

        Ok(ReferenceData {
            ref_seq,
            ref_bert,
            ref_audio_32k,
            ssl_content,
        })
    }

    async fn process_ssl(
        &mut self,
        ref_audio_16k: &Array2<f32>,
    ) -> Result<ArrayBase<OwnedRepr<f32>, IxDyn>, GSVError> {
        let time = SystemTime::now();
        let ssl_output = self
            .ssl
            .run_async(
                inputs!["ref_audio_16k" => TensorRef::from_array_view(ref_audio_16k)?],
                &self.run_options,
            )?
            .await?;
        debug!("SSL processing time: {:?}", time.elapsed()?);
        Ok(ssl_output["ssl_content"]
            .try_extract_array::<f32>()?
            .into_owned())
    }

    /// Efficiently runs the streaming decoder loop with a pre-allocated, resizable KV cache.
    async fn run_t2s_s_decoder_loop(
        &mut self,
        sampler: &mut Sampler,
        sampling_param: SamplingParams,
        ctx: DecoderLoopContext,
    ) -> Result<ArrayBase<OwnedRepr<i64>, IxDyn>, GSVError> {
        let DecoderLoopContext {
            mut y_vec,
            mut k_caches,
            mut v_caches,
            prefix_len,
            initial_valid_len,
        } = ctx;

        let mut idx = 0;
        let mut valid_len = initial_valid_len;
        y_vec.reserve(2048);

        loop {
            // --- 1. Prepare inputs using views of the valid cache portion ---
            let mut inputs = inputs![
                "iy" => TensorRef::from_array_view(unsafe {ArrayView2::from_shape_ptr((1, y_vec.len()), y_vec.as_ptr())})?,
                "y_len" => create_scalar_i64_tensor(prefix_len as i64)?,
                "idx" => create_scalar_i64_tensor(idx as i64)?,
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

            // --- 2. Run the decoder model for one step ---
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
                logits.pop();
            }

            y_vec.push(sampler.sample(&mut logits, &y_vec, &sampling_param));
            let argmax_value = argmax(&logits);

            // --- 3. Check for reallocation and update caches ---
            let new_valid_len = valid_len + 1;

            // Check if we need to reallocate BEFORE writing to the new index
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

            // Update KV caches with new data
            update_kv_cache(
                &mut k_caches,
                &mut v_caches,
                &output,
                valid_len,
                self.num_layers,
            )?;

            // --- 4. Update valid length and check stop condition ---
            valid_len = new_valid_len;

            if idx >= MAX_DECODER_STEPS || argmax_value == T2S_DECODER_EOS {
                let mut sliced = y_vec[(y_vec.len() - idx + 1)..(y_vec.len() - 1)]
                    .iter()
                    .map(|&i| if i == T2S_DECODER_EOS { 0 } else { i })
                    .collect::<Vec<i64>>();
                sliced.push(0);
                debug!(
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

    /// Helper to initialize KV caches from decoder output
    fn initialize_kv_caches(
        fs_decoder_output: &SessionOutputs,
        num_layers: usize,
    ) -> Result<KvCacheTuple, GSVError> {
        let k_init_first = fs_decoder_output["k_cache_0"].try_extract_array::<KvDType>()?;
        let initial_dims_dyn = k_init_first.raw_dim();
        let initial_seq_len = initial_dims_dyn[1];

        let mut large_cache_dims = initial_dims_dyn.clone();
        large_cache_dims[1] = INITIAL_CACHE_SIZE;

        let mut k_caches = Vec::with_capacity(num_layers);
        let mut v_caches = Vec::with_capacity(num_layers);

        for i in 0..num_layers {
            let k_init =
                fs_decoder_output[format!("k_cache_{}", i)].try_extract_array::<KvDType>()?;
            let v_init =
                fs_decoder_output[format!("v_cache_{}", i)].try_extract_array::<KvDType>()?;

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

    /// synthesize async
    ///
    /// `text` is input text for run
    ///
    /// `lang_id` can be LangId::Auto(Mandarin) or LangId::AutoYue（cantonese）
    ///
    pub async fn synthesize<R, S>(
        &mut self,
        text: S,
        reference_data: R,
        sampling_param: SamplingParams,
        lang_id: LangId,
    ) -> Result<impl Stream<Item = Result<Vec<f32>, GSVError>> + Send + Unpin, GSVError>
    where
        R: AsRef<ReferenceData>,
        S: AsRef<str>,
    {
        let start_time = SystemTime::now();
        let texts_and_seqs = self
            .text_processor
            .get_phone_and_bert(text.as_ref(), lang_id)?;
        debug!("g2pw and preprocess time: {:?}", start_time.elapsed()?);
        let ref_data = reference_data.as_ref().clone();

        let stream = stream! {
            for (text, seq, bert) in texts_and_seqs {
                debug!("process: {:?}", text);
                yield self.in_stream_once_gen(&text, &bert, &seq, &ref_data, sampling_param).await;
            }
        };

        Ok(Box::pin(stream))
    }

    async fn in_stream_once_gen(
        &mut self,
        _text: &str,
        text_bert: &Array2<f32>,
        text_seq_vec: &[i64],
        ref_data: &ReferenceData,
        sampling_param: SamplingParams,
    ) -> Result<Vec<f32>, GSVError> {
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
            debug!("T2S Encoder time: {:?}", time.elapsed()?);
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
            debug!("T2S FS Decoder time: {:?}", start_time.elapsed()?);

            let logits = fs_decoder_output["logits"]
                .try_extract_array::<f32>()?
                .into_owned();
            let (k_caches, v_caches, initial_seq_len) =
                Self::initialize_kv_caches(&fs_decoder_output, NUM_LAYERS)?;

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
        debug!("T2S S Decoder all time: {:?}", start_time.elapsed()?);

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
        debug!("SoVITS time: {:?}", sovits_start.elapsed()?);
        let output_audio = outputs["audio"].try_extract_array::<f32>()?;
        let (mut audio, _) = output_audio.into_owned().into_raw_vec_and_offset();

        audio.iter_mut().for_each(|s| *s *= 4.0);

        let max_audio = audio
            .iter()
            .filter(|&&x| x.is_finite())
            .map(|&x| x.abs())
            .fold(0.0f32, |acc, x| acc.max(x));

        if max_audio > 1.0 {
            audio.iter_mut().for_each(|s| *s /= max_audio);
        }

        Ok(audio)
    }
}

fn ensure_punctuation<S>(text: S) -> String
where
    S: AsRef<str>,
{
    let text_ref = text.as_ref();
    if text_ref.ends_with(['。', '！', '？', '；', '.', '!', '?', ';']) {
        text_ref.to_owned()
    } else {
        format!("{}。", text_ref)
    }
}

fn resample_audio(input: &[f32], in_rate: u32, out_rate: u32) -> Vec<f32> {
    if in_rate == out_rate {
        return input.to_owned();
    }

    UniformSourceIterator::new(SamplesBuffer::new(1, in_rate, input), 1, out_rate).collect()
}

async fn read_and_resample_audio<P>(path: P) -> Result<(Array2<f32>, Array2<f32>), GSVError>
where
    P: AsRef<Path>,
{
    let data = Cursor::new(read(path).await?);
    let decoder = Decoder::new(data)?;
    let sample_rate = decoder.sample_rate();
    let samples = if decoder.channels() == 1 {
        decoder.collect::<Vec<_>>()
    } else {
        UniformSourceIterator::new(decoder, 1, sample_rate).collect()
    };

    // Resample to 16kHz and 32kHz
    let mut ref_audio_16k = resample_audio(&samples, sample_rate, 16000);
    let ref_audio_32k = resample_audio(&samples, sample_rate, 32000);

    // Prepend 0.3 seconds of silence
    let silence_16k = vec![0.0; (0.3 * 16000.0) as usize]; // 8000 samples for 16kHz

    ref_audio_16k.splice(0..0, silence_16k);

    // Convert to Array2
    Ok((
        Array2::from_shape_vec((1, ref_audio_16k.len()), ref_audio_16k)?,
        Array2::from_shape_vec((1, ref_audio_32k.len()), ref_audio_32k)?,
    ))
}
