//! RTMP client library for streaming H264 video and AAC audio.

pub mod aac_encoder;
pub mod client;

pub use aac_encoder::{AacEncoder, AacEncoderConfig, AacEncoderError};
pub use client::{
    AudioData, RtmpClient, RtmpClientConfig, RtmpClientError, VideoData,
    annexb_to_avc_decoder_config,
};
pub use fdk_aac::enc::Transport;
