use crate::{
    cache::KvCache,
    image::{create_vision_inputs, preprocess_image},
    model::LfmVlModel,
    tokenizer::LfmTokenizer,
};
use ort::{
    session::Session,
    value::{DynValue, Tensor},
};

/// Run vision encoder to get image embeddings.
fn encode_image(
    vision_encoder: &mut Session,
    pixel_values: DynValue,
    pixel_attention_mask: DynValue,
    spatial_shapes: DynValue,
) -> crate::Result<DynValue> {
    let outputs = vision_encoder.run(ort::inputs! {
        "pixel_values" => pixel_values,
        "pixel_attention_mask" => pixel_attention_mask,
        "spatial_shapes" => spatial_shapes,
    })?;

    let (shape, data) = outputs["image_features"]
        .try_extract_tensor::<f32>()
        .map_err(|e| {
            crate::Error::Inference(format!("Failed to extract image features: {:?}", e))
        })?;
    let shape_vec: Vec<i64> = shape.iter().copied().collect();
    let data_vec: Vec<f32> = data.to_vec();
    let tensor = Tensor::from_array((shape_vec, data_vec.into_boxed_slice()))?;
    Ok(tensor.upcast().into())
}

/// Get token embeddings from input IDs.
fn embed_tokens(
    embed_tokens_session: &mut Session,
    input_ids: DynValue,
) -> crate::Result<DynValue> {
    let outputs = embed_tokens_session.run(ort::inputs! {
        "input_ids" => input_ids,
    })?;

    let (shape, data) = outputs["inputs_embeds"]
        .try_extract_tensor::<f32>()
        .map_err(|e| crate::Error::Inference(format!("Failed to extract embeddings: {:?}", e)))?;
    let shape_vec: Vec<i64> = shape.iter().copied().collect();
    let data_vec: Vec<f32> = data.to_vec();
    let tensor = Tensor::from_array((shape_vec, data_vec.into_boxed_slice()))?;
    Ok(tensor.upcast().into())
}

/// Greedy decode: find the token with the highest logit.
fn argmax(logits: &[f32]) -> u32 {
    logits
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(i, _)| i as u32)
        .unwrap_or(0)
}

/// Generate text from an image and text prompt.
pub fn generate(
    model: &mut LfmVlModel,
    tokenizer: &LfmTokenizer,
    image: &image::DynamicImage,
    prompt: &str,
    max_tokens: usize,
) -> crate::Result<String> {
    let config = &model.config;

    // 1. Preprocess image and run vision encoder
    let img_input = preprocess_image(image, config)?;
    let (pixel_values, pixel_attention_mask, spatial_shapes) = create_vision_inputs(&img_input)?;

    let image_features = encode_image(
        &mut model.vision_encoder,
        pixel_values.upcast().into(),
        pixel_attention_mask.upcast().into(),
        spatial_shapes.upcast().into(),
    )?;

    // Extract image features: [num_image_tokens, hidden_size]
    let (img_shape, img_data) = image_features.try_extract_tensor::<f32>().map_err(|e| {
        crate::Error::Inference(format!("Failed to extract image features: {:?}", e))
    })?;
    let num_image_tokens = img_shape[0] as usize;
    let hidden_size = img_shape[1] as usize;
    let img_data: Vec<f32> = img_data.to_vec();

    // 2. Format prompt with image tokens and get token embeddings
    let chat_prompt = tokenizer.format_chat_prompt(prompt, num_image_tokens);
    let input_ids = tokenizer.encode(&chat_prompt)?;
    let seq_len = input_ids.len();

    let input_ids_tensor: Tensor<i64> = Tensor::from_array((
        vec![1i64, seq_len as i64],
        input_ids
            .iter()
            .map(|&id| id as i64)
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    ))?;
    let mut token_embeds = embed_tokens(&mut model.embed_tokens, input_ids_tensor.upcast().into())?;

    // 3. Replace <image> token positions with image embeddings
    let image_positions = tokenizer.find_image_positions(&input_ids);

    let (embed_shape, embed_data) = token_embeds.try_extract_tensor_mut::<f32>().map_err(|e| {
        crate::Error::Inference(format!("Failed to extract token embeddings: {:?}", e))
    })?;
    let embed_seq_len = embed_shape[1] as usize;
    let embed_hidden = embed_shape[2] as usize;

    for (i, pos) in image_positions.iter().enumerate() {
        if i < num_image_tokens && *pos < embed_seq_len {
            let embed_start = pos * embed_hidden;
            let img_start = i * hidden_size;
            let copy_len = embed_hidden.min(hidden_size);
            embed_data[embed_start..embed_start + copy_len]
                .copy_from_slice(&img_data[img_start..img_start + copy_len]);
        }
    }
    let _ = embed_data; // drop mutable borrow

    // 4. Autoregressive generation loop
    let mut cache = KvCache::new(config)?;
    let mut generated_tokens: Vec<u32> = Vec::with_capacity(max_tokens);
    let mut cur_seq_len = seq_len;
    let mut embeds_for_step = token_embeds;

    for _step in 0..max_tokens {
        let mut decoder_inputs: Vec<(String, DynValue)> =
            Vec::with_capacity(2 + config.conv_layers.len() + config.attn_layers.len() * 2);
        decoder_inputs.push(("inputs_embeds".to_string(), embeds_for_step));

        let attn_mask = Tensor::from_array((
            vec![1i64, cur_seq_len as i64],
            vec![1i64; cur_seq_len].into_boxed_slice(),
        ))?;
        decoder_inputs.push(("attention_mask".to_string(), attn_mask.upcast().into()));
        decoder_inputs.extend(cache.to_inputs(config));

        let outputs = model.decoder.run(decoder_inputs)?;

        // Extract logits: [batch_size, seq_len, vocab_size]
        let (logits_shape, logits_data) = outputs["logits"]
            .try_extract_tensor::<f32>()
            .map_err(|e| crate::Error::Inference(format!("Failed to extract logits: {:?}", e)))?;

        let logits_seq_len = logits_shape[1] as usize;
        let vocab_size = logits_shape[2] as usize;
        let last_token_start = (logits_seq_len - 1) * vocab_size;
        let next_token = argmax(&logits_data[last_token_start..last_token_start + vocab_size]);
        generated_tokens.push(next_token);

        if next_token == tokenizer.eos_token_id() {
            break;
        }

        cache.update_from_outputs(&outputs, config);

        // Prepare embeddings for the next step
        let next_token_ids =
            Tensor::from_array((vec![1i64, 1i64], vec![next_token as i64].into_boxed_slice()))?;
        embeds_for_step = embed_tokens(&mut model.embed_tokens, next_token_ids.upcast().into())?;
        cur_seq_len += 1;
    }

    tokenizer.decode(&generated_tokens, true)
}
