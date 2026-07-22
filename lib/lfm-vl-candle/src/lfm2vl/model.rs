//! LFM2.5-VL model: vision tower + multimodal projector + language model

use crate::lfm2::decoder::Lfm2Decoder;
use crate::lfm2vl::config::{Lfm2VLConfig, Lfm2VLVisionConfig};
use crate::util::interpolate::interpolate_bilinear;
use crate::util::modules::{NaiveAttnTwoLinearMLPBlock, get_layer_norm};
use crate::util::tensor_utils::{get_equal_mask, masked_scatter_dim0, prepare_mask};
use crate::error::{LfmError, Result};
use candle_core::{D, IndexOp, Tensor};
use candle_nn::{Activation, LayerNorm, Linear, Module, VarBuilder, embedding, linear_b};
use num::integer::Roots;

// ── Vision embeddings ───────────────────────────────────────────────

pub struct Siglip2VisionEmbeddings {
    patch_embedding: Linear,
    positional_embeddings: Tensor,
}

impl Siglip2VisionEmbeddings {
    pub fn new(vb: VarBuilder, cfg: &Lfm2VLVisionConfig) -> Result<Self> {
        let embed_dim = cfg.hidden_size;
        let patch_size = cfg.patch_size;
        let patch_embedding = linear_b(
            cfg.num_channels * patch_size * patch_size,
            embed_dim,
            true,
            vb.pp("patch_embedding"),
        )?;
        let position_embedding_size = cfg.num_patches.sqrt();
        let position_embedding =
            embedding(cfg.num_patches, embed_dim, vb.pp("position_embedding"))?;
        let positional_embeddings = position_embedding
            .embeddings()
            .reshape((position_embedding_size, position_embedding_size, ()))?
            .permute((2, 0, 1))?
            .unsqueeze(0)?;
        Ok(Self {
            patch_embedding,
            positional_embeddings,
        })
    }

    fn resize_positional_embeddings(
        &self,
        spatial_shapes: &Tensor,
        max_length: usize,
    ) -> Result<Tensor> {
        let mut result_pos_embeddings = vec![];
        let bs = spatial_shapes.dim(0)?;
        for i in 0..bs {
            let shape_i = spatial_shapes.i(i)?.to_vec1::<u32>()?;
            let height = *shape_i.first().unwrap_or(&32) as usize;
            let width = *shape_i.get(1).unwrap_or(&32) as usize;

            if height == 0 || width == 0 || height * width > max_length {
                return Err(LfmError::ImageProcessing("img height or width illegal".into()));
            }
            let resize_embeddings = interpolate_bilinear(
                &self.positional_embeddings,
                (height, width),
                Some(false),
                Some(true),
            )?
            .reshape(((), height * width))?
            .transpose(0, 1)?;
            let resize_embeddings = if height * width < max_length {
                let pad = max_length - height * width;
                let pad_embedding = resize_embeddings.i(0)?.unsqueeze(0)?.repeat((pad, 1))?;
                Tensor::cat(&[&resize_embeddings, &pad_embedding], 0)?
            } else {
                resize_embeddings
            };
            result_pos_embeddings.push(resize_embeddings);
        }
        let result_pos_embeddings = Tensor::stack(&result_pos_embeddings, 0)?;
        Ok(result_pos_embeddings)
    }

    pub fn forward(&self, pixel_values: &Tensor, spatial_shapes: &Tensor) -> Result<Tensor> {
        let patch_embeds = self.patch_embedding.forward(pixel_values)?;
        let max_length = pixel_values.dim(1)?;
        let resize_pos_embeddings =
            self.resize_positional_embeddings(spatial_shapes, max_length)?;

        let embedding = patch_embeds.add(&resize_pos_embeddings)?;
        Ok(embedding)
    }
}

// ── Vision encoder ──────────────────────────────────────────────────

pub struct Siglip2Encoder {
    layers: Vec<NaiveAttnTwoLinearMLPBlock>,
}

impl Siglip2Encoder {
    pub fn new(vb: VarBuilder, cfg: &Lfm2VLVisionConfig) -> Result<Self> {
        let vb_layers = vb.pp("layers");
        let mut layers = vec![];
        for i in 0..cfg.num_hidden_layers {
            let layer = NaiveAttnTwoLinearMLPBlock::new(
                vb_layers.pp(i),
                cfg.hidden_size,
                cfg.num_attention_heads,
                None,
                None,
                true,
                "self_attn",
                Some("out_proj"),
                cfg.intermediate_size,
                cfg.hidden_act,
                true,
                "mlp",
                "fc1",
                "fc2",
                cfg.layer_norm_eps,
                "layer_norm1",
                "layer_norm2",
            )?;
            layers.push(layer);
        }
        Ok(Self { layers })
    }

    pub fn forward(&self, xs: &Tensor, attention_mask: Option<&Tensor>) -> Result<Tensor> {
        let mut xs = xs.clone();
        for layer in &self.layers {
            xs = layer.forward(&xs, None, None, attention_mask, false)?;
        }
        Ok(xs)
    }
}

// ── Vision model ────────────────────────────────────────────────────

pub struct Siglip2VisionModel {
    embeddings: Siglip2VisionEmbeddings,
    encoder: Siglip2Encoder,
    post_layernorm: LayerNorm,
}

impl Siglip2VisionModel {
    pub fn new(vb: VarBuilder, cfg: &Lfm2VLVisionConfig) -> Result<Self> {
        let vb = vb.pp("vision_model");
        let embeddings = Siglip2VisionEmbeddings::new(vb.pp("embeddings"), cfg)?;
        let encoder = Siglip2Encoder::new(vb.pp("encoder"), cfg)?;
        let post_layernorm = get_layer_norm(
            vb.pp("post_layernorm"),
            cfg.layer_norm_eps,
            cfg.hidden_size,
            true,
        )?;
        Ok(Self {
            embeddings,
            encoder,
            post_layernorm,
        })
    }

    pub fn forward(
        &self,
        pixel_values: &Tensor,
        attention_mask: &Tensor,
        spatial_shapes: &Tensor,
    ) -> Result<Tensor> {
        let xs = self.embeddings.forward(pixel_values, spatial_shapes)?;
        let mask = prepare_mask(attention_mask)?.to_dtype(xs.dtype())?;
        let xs = self.encoder.forward(&xs, Some(&mask))?;
        let xs = self.post_layernorm.forward(&xs)?;
        // Zero out padded positions so they don't leak into downstream
        // computations (projector, language model).  Matches PyTorch's
        // SDPA nan_to_num(0) + the zero_output_mask pattern.
        let pad_mask = attention_mask
            .to_dtype(xs.dtype())?
            .unsqueeze(2)?;  // (B, seq_len, 1)
        let xs = xs.broadcast_mul(&pad_mask)?;
        Ok(xs)
    }
}

// ── Multimodal projector ────────────────────────────────────────────

pub struct Lfm2VlMultiModalProjector {
    factor: usize,
    layer_norm: Option<LayerNorm>,
    linear_1: Linear,
    act: Activation,
    linear_2: Linear,
}

impl Lfm2VlMultiModalProjector {
    pub fn new(vb: VarBuilder, cfg: &Lfm2VLConfig) -> Result<Self> {
        let in_channels = cfg.vision_config.hidden_size * (cfg.downsample_factor).pow(2);
        let factor = cfg.downsample_factor;
        let layer_norm = if let Some(flag) = cfg.projector_use_layernorm
            && !flag
        {
            None
        } else {
            let layer_norm = get_layer_norm(
                vb.pp("layer_norm"),
                cfg.vision_config.layer_norm_eps,
                in_channels,
                true,
            )?;
            Some(layer_norm)
        };
        let linear_1 = linear_b(
            in_channels,
            cfg.projector_hidden_size,
            cfg.projector_bias,
            vb.pp("linear_1"),
        )?;
        let act = cfg.projector_hidden_act;
        let linear_2 = linear_b(
            cfg.projector_hidden_size,
            cfg.text_config.hidden_size,
            cfg.projector_bias,
            vb.pp("linear_2"),
        )?;
        Ok(Self {
            factor,
            layer_norm,
            linear_1,
            act,
            linear_2,
        })
    }

    pub fn forward(&self, xs: &Tensor) -> Result<Tensor> {
        let (bs, w, h, c) = xs.dims4()?;
        let xs = xs.reshape((bs, w, h / self.factor, c * self.factor))?;
        let xs = xs.permute((0, 2, 1, 3))?;
        let xs = xs.reshape((
            bs,
            h / self.factor,
            w / self.factor,
            c * self.factor * self.factor,
        ))?;
        let mut xs = xs.permute((0, 2, 1, 3))?.contiguous()?;
        if let Some(norm) = &self.layer_norm {
            xs = norm.forward(&xs)?;
        }
        xs = self.linear_1.forward(&xs)?.apply(&self.act)?;
        xs = self.linear_2.forward(&xs)?;
        Ok(xs)
    }
}

// ── Full LFM2.5-VL model ────────────────────────────────────────────

pub struct LFM2VLModel {
    vision_tower: Siglip2VisionModel,
    multi_modal_projector: Lfm2VlMultiModalProjector,
    language_model: Lfm2Decoder,
    lm_head: Linear,
    img_id: u32,
    stop_token_ids: Vec<u32>,
}

impl LFM2VLModel {
    pub fn new(vb: VarBuilder, cfg: &Lfm2VLConfig, eos_ids: Vec<u32>) -> Result<Self> {
        let vb = vb.pp("model");
        let vision_tower = Siglip2VisionModel::new(vb.pp("vision_tower"), &cfg.vision_config)?;
        let multi_modal_projector =
            Lfm2VlMultiModalProjector::new(vb.pp("multi_modal_projector"), cfg)?;
        let language_model = Lfm2Decoder::new(vb.pp("language_model"), &cfg.text_config)?;
        let lm_head = Linear::new(language_model.embed_tokens.embeddings().clone(), None);
        Ok(Self {
            vision_tower,
            multi_modal_projector,
            language_model,
            lm_head,
            img_id: cfg.image_token_id,
            stop_token_ids: eos_ids,
        })
    }

    pub fn forward(
        &mut self,
        input_ids: &Tensor,
        pixel_values: Option<&Tensor>,
        pixel_attention_mask: Option<&Tensor>,
        spatial_shapes: Option<&Tensor>,
        seqlen_offset: usize,
    ) -> Result<Tensor> {
        let mut inputs_embeds = self.language_model.embed_tokens.forward(input_ids)?;
        if let Some(pixel) = pixel_values
            && let Some(mask) = pixel_attention_mask
            && let Some(shapes) = spatial_shapes
        {
            let image_embeds = self.vision_tower.forward(pixel, mask, shapes)?;

            let bs = image_embeds.dim(0)?;
            let img_feature_length = mask.sum(1)?.to_vec1::<u32>()?;
            let mut image_features = vec![];
            for img_idx in 0..bs {
                let feature = image_embeds.i(img_idx)?;
                let feature = feature.narrow(0, 0, img_feature_length[img_idx] as usize)?;
                let shape = shapes.i(img_idx)?.to_vec1::<u32>()?;
                let h = shape[0];
                let w = shape[1];
                let feature = feature
                    .reshape((1, h as usize, w as usize, ()))?
                    .contiguous()?;
                let img_embedding = self.multi_modal_projector.forward(&feature)?;
                let dim = img_embedding.dim(D::Minus1)?;
                let img_embedding = img_embedding.reshape(((), dim))?;
                image_features.push(img_embedding);
            }
            let image_embeds = Tensor::cat(&image_features, 0)?;

            let image_mask = get_equal_mask(input_ids, self.img_id)?;
            inputs_embeds = masked_scatter_dim0(&inputs_embeds, &image_embeds, &image_mask)?;
        }
        let output = self
            .language_model
            .forward(input_ids, Some(&inputs_embeds), seqlen_offset)?;
        let seq_len = output.dim(1)?;
        let last = output.narrow(1, seq_len - 1, 1)?;
        let logits = self.lm_head.forward(&last)?;
        Ok(logits)
    }

    pub fn clear_cache(&mut self) {
        self.language_model.clear_cache();
    }

    pub fn stop_token_ids(&self) -> &[u32] {
        &self.stop_token_ids
    }
}
