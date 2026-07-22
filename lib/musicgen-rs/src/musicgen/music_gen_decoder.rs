use crate::musicgen::{
    delay_pattern_mask_ids::DelayedPatternMaskIds,
    music_gen_config::MusicGenConfig,
    music_gen_inputs::MusicGenInputs,
    music_gen_outputs::MusicGenOutputs,
    tensor_ops::{dupe_zeros_along_first_dim, zeros_tensor},
};
use num_traits::Zero;
use ort::{
    session::Session,
    value::{DynValue, PrimitiveTensorElementType, Tensor, TensorValueType},
};
use std::{
    fmt::Debug,
    marker::PhantomData,
    sync::mpsc::Receiver,
    sync::{Arc, Mutex},
};

pub trait MusicGenType: PrimitiveTensorElementType + Debug + Clone + Zero {}

impl MusicGenType for u8 {}
impl MusicGenType for i8 {}
impl MusicGenType for f32 {}
impl MusicGenType for half::f16 {}

// TODO: is this configurable?
const GUIDANCE_SCALE: usize = 3;

pub trait MusicGenDecoder: Send + Sync {
    fn generate_tokens(
        &self,
        last_hidden_state: DynValue,
        encoder_attention_mask: DynValue,
        max_len: usize,
    ) -> Result<Receiver<Result<[i64; 4], Error>>, Error>;
}

/// Error type for MusicGen decoder operations.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("ONNX Runtime error: {0}")]
    Ort(#[from] ort::Error),
    #[error("Generation aborted")]
    Aborted,
}

pub struct MusicGenMergedDecoder<T: MusicGenType> {
    pub decoder_model_merged: Arc<Mutex<Session>>,
    pub config: MusicGenConfig,
    pub _phantom_data: PhantomData<T>,
}

unsafe impl<T: MusicGenType> Send for MusicGenMergedDecoder<T> {}
unsafe impl<T: MusicGenType> Sync for MusicGenMergedDecoder<T> {}

impl<T: MusicGenType + 'static> MusicGenDecoder for MusicGenMergedDecoder<T> {
    fn generate_tokens(
        &self,
        last_hidden_state: DynValue,
        encoder_attention_mask: DynValue,
        max_len: usize,
    ) -> Result<Receiver<Result<[i64; 4], Error>>, Error> {
        let encoder_hidden_states =
            dupe_zeros_along_first_dim::<T>(last_hidden_state.downcast::<TensorValueType<T>>()?)?;
        let encoder_attention_mask = dupe_zeros_along_first_dim::<i64>(
            encoder_attention_mask.downcast::<TensorValueType<i64>>()?,
        )?;

        let decoder_model_merged = self.decoder_model_merged.clone();
        let num_hidden_layers = self.config.decoder.num_hidden_layers;
        let num_attention_heads = self.config.decoder.num_attention_heads;
        let pad_token_id = self.config.decoder.pad_token_id;
        let d_kv = self.config.text_encoder.d_kv;
        let top_k = self.config.decoder.top_k;
        let decoder_dims = [1, num_attention_heads, 0, d_kv];
        let encoder_dims = [1, num_attention_heads, 0, d_kv];

        let (tx, rx) = std::sync::mpsc::channel::<Result<[i64; 4], Error>>();
        let tx2 = tx.clone();

        std::thread::spawn(move || {
            let result: Result<(), Error> = (|| {
                let mut delay_pattern_mask_ids = DelayedPatternMaskIds::<4>::new();
                let mut inputs = MusicGenInputs::new();
                inputs.encoder_attention_mask(encoder_attention_mask.into_dyn());
                inputs.encoder_hidden_states(encoder_hidden_states.into_dyn());
                inputs.input_ids(Tensor::from_array(([8, 1], vec![pad_token_id; 8])).unwrap());

                for i in 0..num_hidden_layers {
                    inputs
                        .past_key_value_decoder_key(i, zeros_tensor::<T>(&decoder_dims).into_dyn());
                    inputs.past_key_value_decoder_value(
                        i,
                        zeros_tensor::<T>(&decoder_dims).into_dyn(),
                    );
                    inputs
                        .past_key_value_encoder_key(i, zeros_tensor::<T>(&encoder_dims).into_dyn());
                    inputs.past_key_value_encoder_value(
                        i,
                        zeros_tensor::<T>(&encoder_dims).into_dyn(),
                    );
                }
                inputs.use_cache_branch(false);

                let mut session = decoder_model_merged.lock().unwrap();
                for _ in 0..max_len {
                    let outputs = session.run(inputs.ort())?;
                    let mut outputs = MusicGenOutputs::new(outputs);

                    delay_pattern_mask_ids.push(
                        outputs
                            .take_logits()?
                            .apply_free_guidance(GUIDANCE_SCALE)
                            .sample(top_k)
                            .iter()
                            .map(|e| e.0),
                    );

                    let [a, b, c, d] = delay_pattern_mask_ids.last_delayed_masked(pad_token_id);
                    inputs.input_ids(
                        Tensor::from_array(([8, 1], vec![a, b, c, d, a, b, c, d])).unwrap(),
                    );

                    if let Some(last_de_delayed) = delay_pattern_mask_ids.last_de_delayed() {
                        let sent = tx.send(Ok(last_de_delayed));
                        if sent.is_err() {
                            break;
                        }
                    }

                    for j in 0..num_hidden_layers {
                        let v = outputs.take_present_decoder_key(j);
                        inputs.past_key_value_decoder_key(j, v);
                        let v = outputs.take_present_decoder_value(j);
                        inputs.past_key_value_decoder_value(j, v);
                        if !inputs.use_cache_branch {
                            let v = outputs.take_present_encoder_key(j);
                            inputs.past_key_value_encoder_key(j, v);
                            let v = outputs.take_present_encoder_value(j);
                            inputs.past_key_value_encoder_value(j, v);
                        }
                    }

                    inputs.use_cache_branch(true);
                    drop(outputs);
                }
                Ok(())
            })();

            if let Err(err) = result {
                let _ = tx2.send(Err(err));
            }
        });

        Ok(rx)
    }
}

pub struct MusicGenSplitDecoder<T: MusicGenType> {
    pub decoder_model: Arc<Mutex<Session>>,
    pub decoder_with_past_model: Arc<Mutex<Session>>,
    pub config: MusicGenConfig,
    pub _phantom_data: PhantomData<T>,
}

unsafe impl<T: MusicGenType> Send for MusicGenSplitDecoder<T> {}
unsafe impl<T: MusicGenType> Sync for MusicGenSplitDecoder<T> {}

impl<T: MusicGenType + 'static> MusicGenDecoder for MusicGenSplitDecoder<T> {
    fn generate_tokens(
        &self,
        last_hidden_state: DynValue,
        encoder_attention_mask: DynValue,
        max_len: usize,
    ) -> Result<Receiver<Result<[i64; 4], Error>>, Error> {
        let encoder_hidden_states =
            dupe_zeros_along_first_dim::<T>(last_hidden_state.downcast::<TensorValueType<T>>()?)?;
        let encoder_attention_mask = dupe_zeros_along_first_dim::<i64>(
            encoder_attention_mask.downcast::<TensorValueType<i64>>()?,
        )?;

        let decoder_model = self.decoder_model.clone();
        let decoder_with_past = self.decoder_with_past_model.clone();
        let num_hidden_layers = self.config.decoder.num_hidden_layers;
        let pad_token_id = self.config.decoder.pad_token_id;
        let top_k = self.config.decoder.top_k;

        let (tx, rx) = std::sync::mpsc::channel::<Result<[i64; 4], Error>>();
        let tx2 = tx.clone();

        std::thread::spawn(move || {
            let result: Result<(), Error> = (|| {
                let mut delay_pattern_mask_ids = DelayedPatternMaskIds::<4>::new();
                let mut inputs = MusicGenInputs::new();
                inputs.encoder_attention_mask(encoder_attention_mask.into_dyn());
                inputs.input_ids(Tensor::from_array(([8, 1], vec![pad_token_id; 8]))?);
                inputs.encoder_hidden_states(encoder_hidden_states.into_dyn());

                // First pass: run decoder_model, extract KV cache.
                {
                    let mut first_session = decoder_model.lock().unwrap();
                    let outputs = first_session.run(inputs.ort())?;
                    let mut outputs = MusicGenOutputs::new(outputs);

                    delay_pattern_mask_ids.push(
                        outputs
                            .take_logits()?
                            .apply_free_guidance(GUIDANCE_SCALE)
                            .sample(top_k)
                            .iter()
                            .map(|e| e.0),
                    );

                    for j in 0..num_hidden_layers {
                        let v = outputs.take_present_decoder_key(j);
                        inputs.past_key_value_decoder_key(j, v);
                        let v = outputs.take_present_decoder_value(j);
                        inputs.past_key_value_decoder_value(j, v);
                        let v = outputs.take_present_encoder_key(j);
                        inputs.past_key_value_encoder_key(j, v);
                        let v = outputs.take_present_encoder_value(j);
                        inputs.past_key_value_encoder_value(j, v);
                    }
                    // SessionOutputs dropped here, releasing the borrow on first_session.
                }
                // first_session mutex released here.

                inputs.remove_encoder_hidden_states();

                // Subsequent passes: run decoder_with_past_model.
                let mut session = decoder_with_past.lock().unwrap();
                for _ in 0..max_len {
                    let [a, b, c, d] = delay_pattern_mask_ids.last_delayed_masked(pad_token_id);
                    inputs.input_ids(Tensor::from_array(([8, 1], vec![a, b, c, d, a, b, c, d]))?);
                    let outputs = session.run(inputs.ort())?;
                    let mut outputs = MusicGenOutputs::new(outputs);

                    delay_pattern_mask_ids.push(
                        outputs
                            .take_logits()?
                            .apply_free_guidance(GUIDANCE_SCALE)
                            .sample(top_k)
                            .iter()
                            .map(|e| e.0),
                    );

                    if let Some(last_de_delayed) = delay_pattern_mask_ids.last_de_delayed() {
                        let sent = tx.send(Ok(last_de_delayed));
                        if sent.is_err() {
                            break;
                        }
                    }

                    for j in 0..num_hidden_layers {
                        let v = outputs.take_present_decoder_key(j);
                        inputs.past_key_value_decoder_key(j, v);
                        let v = outputs.take_present_decoder_value(j);
                        inputs.past_key_value_decoder_value(j, v);
                    }

                    drop(outputs);
                }
                Ok(())
            })();

            if let Err(err) = result {
                let _ = tx2.send(Err(err));
            }
        });

        Ok(rx)
    }
}
