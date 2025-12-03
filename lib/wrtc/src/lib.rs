pub mod errors;
pub mod opus2aac;
pub mod rtp_queue;
pub mod session;
pub mod webrtc;
pub mod whep;

use async_trait::async_trait;
use bytes::BytesMut;
use tokio::sync::mpsc;

#[derive(Clone)]
pub enum PacketData {
    Video { timestamp: u32, data: BytesMut },
    Audio { timestamp: u32, data: BytesMut },
}

#[derive(Clone)]
pub enum FrameData {
    Video { timestamp: u32, data: BytesMut },
    Audio { timestamp: u32, data: BytesMut },
    MetaData { timestamp: u32, data: BytesMut },
    MediaInfo { media_info: MediaInfo },
}

#[derive(Clone)]
pub struct MediaInfo {
    pub audio_clock_rate: u32,
    pub video_clock_rate: u32,
    pub vcodec: VideoCodecType,
}

#[derive(Clone, PartialEq)]
pub enum VideoCodecType {
    H264,
    H265,
}

pub enum Event {
    Subscribe,
    UnSubscribe,
}

#[derive(Debug, Clone)]
pub enum DataSender {
    Frame { sender: FrameDataSender },
    Packet { sender: PacketDataSender },
}

pub type PacketDataSender = mpsc::UnboundedSender<PacketData>;
pub type PacketDataReceiver = mpsc::UnboundedReceiver<PacketData>;

pub type FrameDataSender = mpsc::UnboundedSender<FrameData>;
pub type FrameDataReceiver = mpsc::UnboundedReceiver<FrameData>;

pub type EventSender = mpsc::UnboundedSender<Event>;
pub type EventReceiver = mpsc::UnboundedReceiver<Event>;

#[async_trait]
pub trait TStreamHandler: Send + Sync {
    async fn send_prior_data(&self, sender: DataSender) -> Result<(), errors::StreamError>;
}
