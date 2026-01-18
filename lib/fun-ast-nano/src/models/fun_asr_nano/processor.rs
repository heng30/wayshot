use crate::error::Result;
use candle_core::{D, Device, Tensor};

use crate::{models::fun_asr_nano::config::FrontendConf, tokenizer::TokenizerModel};

use audio_utils::extract::{
    apply_lfr, get_waveform_and_window_properties, kaldi_fbank, kaldi_get_mel_banks,
};

pub struct FunAsrNanoProcessor {
    fronted_conf: FrontendConf,
    device: Device,
    prompt_prefix: String,
    prompt_suffix: String,
    window_shift: usize,
    window_size: usize,
    padded_window_size: usize,
    mel_energies: Tensor,
}

impl FunAsrNanoProcessor {
    pub fn new(fronted_conf: &FrontendConf, device: &Device) -> Result<Self> {
        let (window_shift, window_size, padded_window_size) = get_waveform_and_window_properties(
            fronted_conf.fs,
            fronted_conf.frame_shift,
            fronted_conf.frame_length,
            true,
        )?;
        let (mel_energies, _) = kaldi_get_mel_banks(
            fronted_conf.n_mels,
            padded_window_size,
            fronted_conf.fs as f32,
            20.0,
            0.0,
            device,
        )?;
        let mel_energies = mel_energies.pad_with_zeros(D::Minus1, 0, 1)?.t()?;
        Ok(Self {
            fronted_conf: fronted_conf.clone(),
            device: device.clone(),
            prompt_prefix:
                "<|im_start|>system\nYou are a helpful assistant.<|im_end|>\n<|im_start|>user\n"
                    .to_string(),
            prompt_suffix: "<|im_end|>\n<|im_start|>assistant\n".to_string(),
            window_shift,
            window_size,
            padded_window_size,
            mel_energies,
        })
    }

    pub fn extract_fbank(&self, audio: &Tensor) -> Result<(Tensor, usize)> {
        let waveform = audio.affine(32768.0, 0.0)?;
        let mut mat = kaldi_fbank(
            &waveform,
            &self.mel_energies,
            self.window_shift,
            self.window_size,
            self.padded_window_size,
            1.0,
        )?;
        mat = mat.squeeze(0)?;
        if self.fronted_conf.lfr_m != 1 || self.fronted_conf.lfr_n != 1 {
            mat = apply_lfr(&mat, self.fronted_conf.lfr_m, self.fronted_conf.lfr_n)?;
        }
        let feat_length = mat.dim(0)?;
        let mat = mat.unsqueeze(0)?;
        Ok((mat, feat_length))
    }

    /// Process audio data directly (simplified version without ChatCompletionParameters)
    pub fn process_audio(
        &self,
        audio_data: &[f32],
        user_prompt: Option<&str>,
        tokenizer: &TokenizerModel,
    ) -> Result<(Tensor, Tensor, Tensor)> {
        let user_text = user_prompt.unwrap_or("Transcribe the following audio.");
        let sub_prompt = self.prompt_prefix.clone() + user_text;
        let mut source_ids = vec![];
        let mut fbank_mask = vec![];
        let sub_token = tokenizer.text_encode_vec(sub_prompt, true)?;
        source_ids.extend_from_slice(&sub_token);
        fbank_mask.extend_from_slice(&vec![0u32; sub_token.len()]);

        // Convert audio data to tensor - reshape to 2D (1, num_samples)
        let audio = Tensor::from_vec(audio_data.to_vec(), (1, audio_data.len()), &self.device)?;
        let (speech, speech_lengths) = self.extract_fbank(&audio)?;
        let olens = 1 + (speech_lengths - 3 + 2) / 2;
        let olens = 1 + (olens - 3 + 2) / 2;
        let fake_token_len = (olens - 1) / 2 + 1;
        source_ids.extend_from_slice(&vec![0u32; fake_token_len]);
        fbank_mask.extend_from_slice(&vec![1u32; fake_token_len]);
        let sub_token = tokenizer.text_encode_vec(self.prompt_suffix.clone(), true)?;
        source_ids.extend_from_slice(&sub_token);
        fbank_mask.extend_from_slice(&vec![0u32; sub_token.len()]);
        let input_ids = Tensor::from_slice(&source_ids, (1, source_ids.len()), &self.device)?;
        let fbank_mask = Tensor::from_slice(&fbank_mask, (1, fbank_mask.len()), &self.device)?;

        Ok((speech, fbank_mask, input_ids))
    }
}
