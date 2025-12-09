use crate::{
    ClientError,
    opus::{OPUS_SAMPLE_RATE, OpusCoder},
    session::MediaInfo,
    whep::ICE_SERVERS,
};
use audiopus::Channels;
use crossbeam::channel::Sender;
use derive_setters::Setters;
use http::{
    header::{AUTHORIZATION, CONTENT_TYPE},
    {HeaderMap, HeaderValue},
};
use log::{debug, info, trace, warn};
use openh264::decoder::Decoder;
use std::sync::Arc;
use tokio::{sync::Notify, time::Duration};
use webrtc::{
    api::{
        APIBuilder,
        interceptor_registry::register_default_interceptors,
        media_engine::{MIME_TYPE_H264, MIME_TYPE_OPUS, MediaEngine},
    },
    ice_transport::{ice_connection_state::RTCIceConnectionState, ice_server::RTCIceServer},
    interceptor::registry::Registry,
    peer_connection::{
        configuration::RTCConfiguration, sdp::session_description::RTCSessionDescription,
    },
    rtcp::payload_feedbacks::picture_loss_indication::PictureLossIndication,
    rtp_transceiver::rtp_codec::{RTCRtpCodecCapability, RTCRtpCodecParameters, RTPCodecType},
    track::track_remote::TrackRemote,
};

pub type RGBFrame = (u32, u32, Vec<u8>); // (width, height, rgb_data)
pub type AudioSamples = Vec<f32>;
pub type ClientResult<T> = std::result::Result<T, ClientError>;

#[non_exhaustive]
#[derive(Debug, Setters, Clone)]
#[setters[prefix = "with_"]]
pub struct WHEPClientConfig {
    pub server_url: String,

    #[setters(strip_option)]
    pub auth_token: Option<String>,

    pub ice_servers: Vec<String>,
}

impl WHEPClientConfig {
    pub fn new(server_url: String) -> Self {
        Self {
            server_url,
            auth_token: None,
            ice_servers: ICE_SERVERS
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>(),
        }
    }
}

pub struct WHEPClient {
    pub config: WHEPClientConfig,
    pub media_info: MediaInfo,
    pub video_sender: Option<Sender<RGBFrame>>,
    pub audio_sender: Option<Sender<AudioSamples>>,
}

impl WHEPClient {
    pub async fn new(
        config: WHEPClientConfig,
        video_sender: Option<Sender<RGBFrame>>,
        audio_sender: Option<Sender<AudioSamples>>,
    ) -> ClientResult<WHEPClient> {
        let media_info = match fetch_media_info(&config.server_url).await {
            Ok(info) => info,
            Err(e) => {
                warn!("Failed to fetch media info: {e}");
                return Err(ClientError::ConnectionError(format!(
                    "Failed to fetch media info: {e}"
                )));
            }
        };

        info!(
            "Fetched media info: video {}x{} @ {}fps, audio {}ch @ {}Hz",
            media_info.video.width,
            media_info.video.height,
            media_info.video.fps,
            media_info.audio.channels,
            media_info.audio.sample_rate
        );

        info!("ice servers: {:#?}", media_info.ice_servers);

        Ok(Self {
            config,
            media_info,
            video_sender,
            audio_sender,
        })
    }
}

impl WHEPClient {
    pub async fn connect(&self) -> ClientResult<()> {
        let mut m = MediaEngine::default();
        m.register_codec(
            RTCRtpCodecParameters {
                capability: RTCRtpCodecCapability {
                    mime_type: MIME_TYPE_H264.to_owned(),
                    clock_rate: video_encoder::VIDEO_TIMESCALE,
                    channels: 0,
                    sdp_fmtp_line: "".to_owned(),
                    rtcp_feedback: vec![],
                },
                payload_type: 102,
                ..Default::default()
            },
            RTPCodecType::Video,
        )?;

        m.register_codec(
            RTCRtpCodecParameters {
                capability: RTCRtpCodecCapability {
                    mime_type: MIME_TYPE_OPUS.to_owned(),
                    clock_rate: OPUS_SAMPLE_RATE as u32,
                    channels: self.media_info.audio.channels,
                    sdp_fmtp_line: "".to_owned(),
                    rtcp_feedback: vec![],
                },
                payload_type: 111,
                ..Default::default()
            },
            RTPCodecType::Audio,
        )?;

        let mut registry = Registry::new();
        registry = register_default_interceptors(registry, &mut m)?;

        let api = APIBuilder::new()
            .with_media_engine(m)
            .with_interceptor_registry(registry)
            .build();

        let config = RTCConfiguration {
            ice_servers: vec![RTCIceServer {
                urls: if self.media_info.ice_servers.is_empty() {
                    self.config.ice_servers.clone()
                } else {
                    self.media_info.ice_servers.clone()
                },
                ..Default::default()
            }],
            ..Default::default()
        };

        let peer_connection = Arc::new(api.new_peer_connection(config).await?);
        peer_connection
            .add_transceiver_from_kind(RTPCodecType::Audio, None)
            .await?;
        peer_connection
            .add_transceiver_from_kind(RTPCodecType::Video, None)
            .await?;

        let notify_tx = Arc::new(Notify::new());
        let notify_rx = notify_tx.clone();
        let video_sender = self.video_sender.clone();
        let audio_sender = self.audio_sender.clone();
        let pc = Arc::downgrade(&peer_connection);
        let media_info = self.media_info.clone();

        peer_connection.on_track(Box::new(move |track, _, _| {
            // Send a PLI on an interval so that the publisher is pushing a keyframe every rtcpPLIInterval
            let media_ssrc = track.ssrc();
            let pc2 = pc.clone();
            tokio::spawn(async move {
                let mut result = ClientResult::<usize>::Ok(0);
                while result.is_ok() {
                    let timeout = tokio::time::sleep(Duration::from_secs(3));
                    tokio::pin!(timeout);

                    tokio::select! {
                        _ = timeout.as_mut() =>{
                            if let Some(pc) = pc2.upgrade(){
                                result = pc.write_rtcp(&[Box::new(PictureLossIndication{
                                    sender_ssrc: 0,
                                    media_ssrc,
                                })]).await.map_err(Into::into);
                            }else {
                                break;
                            }
                        }
                    };
                }
            });

            let notify_rx = Arc::clone(&notify_rx);
            let mut video_sender = video_sender.clone();
            let mut audio_sender = audio_sender.clone();

            Box::pin(async move {
                let codec = track.codec();
                let mime_type = codec.capability.mime_type.to_lowercase();
                if mime_type == MIME_TYPE_OPUS.to_lowercase() {
                    info!("Got Opus track, processing audio");

                    if let Some(sender) = audio_sender.take() {
                        tokio::spawn(async move {
                            let _ = process_audio_track(
                                track,
                                sender,
                                notify_rx,
                                media_info.audio.sample_rate,
                                media_info.audio.channels,
                            )
                            .await;
                        });
                    }
                } else if mime_type == MIME_TYPE_H264.to_lowercase() {
                    info!("Got H264 track, processing video");
                    if let Some(sender) = video_sender.take() {
                        tokio::spawn(async move {
                            let _ = process_video_track(
                                track,
                                sender,
                                notify_rx,
                                media_info.video.width as u32,
                                media_info.video.height as u32,
                            )
                            .await;
                        });
                    }
                }
            })
        }));

        let notify_tx2 = notify_tx.clone();
        let (done_tx, mut done_rx) = tokio::sync::mpsc::channel::<()>(1);

        peer_connection.on_ice_connection_state_change(Box::new(
            move |connection_state: RTCIceConnectionState| {
                debug!("Connection State has changed {connection_state}");

                if connection_state == RTCIceConnectionState::Connected {
                    info!("WHEP client connected to server");
                } else if connection_state == RTCIceConnectionState::Failed {
                    warn!("WHEP client connection failed");
                    notify_tx2.notify_waiters();
                    _ = done_tx.try_send(());
                }
                Box::pin(async {})
            },
        ));

        info!("WHEP client connecting to: {}", self.config.server_url);

        let offer = peer_connection.create_offer(None).await?;
        let mut gather_complete = peer_connection.gathering_complete_promise().await;
        peer_connection.set_local_description(offer).await?;
        let _ = gather_complete.recv().await;

        if let Some(local_desc) = peer_connection.local_description().await {
            let client = reqwest::Client::new();
            let offer_sdp = local_desc.sdp;

            info!("Sending WHEP request to: {}/whep", self.config.server_url);
            debug!("SDP Offer length: {} bytes", offer_sdp.len());

            let mut headers = HeaderMap::new();
            headers.insert(
                CONTENT_TYPE,
                HeaderValue::from_str("application/sdp").unwrap(),
            );
            if let Some(token) = &self.config.auth_token {
                headers.insert(
                    AUTHORIZATION,
                    HeaderValue::from_str(&format!("Bearer {token}")).unwrap(),
                );
            };

            let response = client
                .post(&format!("{}/whep", self.config.server_url))
                .headers(headers)
                .body(offer_sdp)
                .send()
                .await?;

            info!("WHEP response status: {}", response.status());

            if response.status().is_success() {
                let answer_sdp = response.text().await?;
                debug!("SDP Answer received, length: {} bytes", answer_sdp.len());

                let answer = RTCSessionDescription::answer(answer_sdp)?;
                peer_connection.set_remote_description(answer).await?;
                info!("WHEP connection established successfully");
            } else {
                let status = response.status();
                let error_text = response.text().await.unwrap_or_default();
                return Err(ClientError::ConnectionError(format!(
                    "WHEP server returned error: {} - {}",
                    status, error_text
                )));
            }
        } else {
            return Err(ClientError::MissingLocalDescription);
        }

        _ = done_rx.recv().await;

        info!("WHEP client connection ended");

        peer_connection.close().await?;
        Ok(())
    }
}

async fn process_video_track(
    track: Arc<TrackRemote>,
    video_sender: Sender<RGBFrame>,
    notify: Arc<Notify>,
    width: u32,
    height: u32,
) -> ClientResult<()> {
    let mut h264_decoder = H264Decoder::new(width, height)?;

    info!("H264 video decoder initialized");

    loop {
        tokio::select! {
            result = track.read_rtp() => {
                if let Ok((rtp_packet, _)) = result {
                    let payload = rtp_packet.payload;
                    trace!("Received H264 RTP packet: {} bytes payload", payload.len());

                    let h264_data = parse_h264_rtp_payload(&payload);
                    match h264_decoder.decode(&h264_data) {
                        Ok(rgb_frame) => {
                            if let Err(e) = video_sender.try_send(rgb_frame) {
                                warn!("video_sender try send failed: {e}");
                            }
                        }
                        Err(e) => trace!("H264 decoding failed: {e}"),
                    }
                } else {
                    info!("Video track ended");
                    break;
                }
            }
            _ = notify.notified() => {
                info!("Video processing stopped by notification");
                break;
            }
        }
    }
    Ok(())
}

async fn process_audio_track(
    track: Arc<TrackRemote>,
    audio_sender: Sender<AudioSamples>,
    notify: Arc<Notify>,
    sample_rate: u32,
    channels: u16,
) -> ClientResult<()> {
    let mut opus_decoder = OpusCoder::new(
        sample_rate,
        if channels == 1 {
            Channels::Mono
        } else {
            Channels::Stereo
        },
    )?;

    info!("Opus audio decoder initialized");

    loop {
        tokio::select! {
            result = track.read_rtp() => {
                if let Ok((rtp_packet, _)) = result {
                    let payload = rtp_packet.payload;

                    match opus_decoder.decode(&payload) {
                        Ok(audio_samples) => {
                            if let Err(e) = audio_sender.send(audio_samples) {
                                warn!("audio_sender try send failed: {e}");
                            }
                        }
                        Err(e) =>  warn!("Opus decoding failed: {e}"),
                    }
                } else {
                    info!("Audio track ended");
                    break;
                }
            }
            _ = notify.notified() => {
                info!("Audio processing stopped by notification");
                break;
            }
        }
    }
    Ok(())
}

pub struct H264Decoder {
    decoder: Decoder,
    width: u32,
    height: u32,
    frame_count: u32,
}

impl H264Decoder {
    pub fn new(width: u32, height: u32) -> ClientResult<Self> {
        let decoder = Decoder::new().map_err(|e| {
            ClientError::H264DecoderError(format!("Failed to create OpenH264 decoder: {:?}", e))
        })?;

        Ok(Self {
            decoder,
            width,
            height,
            frame_count: 0,
        })
    }

    pub fn decode(&mut self, h264_data: &[u8]) -> ClientResult<RGBFrame> {
        if h264_data.is_empty() {
            return Ok((
                self.width,
                self.height,
                vec![0u8; (self.width * self.height * 3) as usize],
            ));
        }

        trace!(
            "Processing pre-processed H264 data: {} bytes",
            h264_data.len()
        );

        let nal_type = if h264_data.len() >= 5 {
            h264_data[4] & 0x1F
        } else {
            debug!("H264 data too short for NAL header");
            return Err(ClientError::H264DataTooShort);
        };

        trace!("NAL unit type {}, size: {}", nal_type, h264_data.len());

        match self.decoder.decode(h264_data) {
            Ok(Some(yuv_frame)) => {
                match mp4_player::yuv420_to_rgb(&yuv_frame, self.width, self.height) {
                    Ok(rgb_data) => {
                        self.frame_count += 1;
                        trace!(
                            "Successfully converted H264 frame to RGB: {}x{}",
                            self.width, self.height
                        );
                        return Ok((self.width, self.height, rgb_data));
                    }
                    Err(e) => {
                        warn!("YUV to RGB conversion error: {:?}", e);
                        return Err(ClientError::YuvToRgbError(format!(
                            "YUV to RGB conversion failed: {e:?}",
                        )));
                    }
                }
            }
            Ok(None) => trace!("No frame produced from NAL type {}, continuing", nal_type),
            Err(e) => trace!("Decode error on NAL type {}: {:?}", nal_type, e),
        }

        Err(ClientError::H264DecodeFailed)
    }
}

async fn fetch_media_info(server_url: &str) -> ClientResult<MediaInfo> {
    let client = reqwest::Client::new();
    let response = client
        .get(&format!("{}/mediainfo", server_url))
        .send()
        .await
        .map_err(|e| ClientError::ConnectionError(format!("Failed to fetch media info: {}", e)))?;

    if response.status().is_success() {
        let media_info: MediaInfo = response.json().await.map_err(|e| {
            ClientError::ConnectionError(format!("Failed to parse media info: {}", e))
        })?;
        Ok(media_info)
    } else {
        Err(ClientError::ConnectionError(format!(
            "Media info endpoint returned status: {}",
            response.status()
        )))
    }
}

fn parse_h264_rtp_payload(payload: &[u8]) -> Vec<u8> {
    if payload.is_empty() {
        return Vec::new();
    }

    let nal_header = payload[0];
    let f_bit = (nal_header >> 7) & 0x1;
    let nal_type = nal_header & 0x1F;

    trace!(
        "Parsing RTP payload: NAL type {}({}), F bit {}",
        match nal_type {
            1 => "Non-IDR",
            5 => "IDR",
            7 => "SPS",
            8 => "PPS",
            24 => "STAP-A",
            _ => "Unknown",
        },
        nal_type,
        f_bit
    );

    match nal_type {
        1 | 5 | 7 | 8 => {
            let mut result = Vec::with_capacity(payload.len() + 4);
            result.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]);
            result.extend_from_slice(payload);
            result
        }
        24 => {
            // STAP-A contains multiple NAL units including SPS/PPS
            trace!("STAP-A packet, extracting NAL units");
            parse_stap_a(payload)
        }
        _ => {
            trace!("Skipping NAL unit type: {}", nal_type);
            Vec::new()
        }
    }
}

fn parse_stap_a(payload: &[u8]) -> Vec<u8> {
    let mut result = Vec::new();
    let mut pos = 1; // Skip STAP-A header

    trace!("STAP-A: Processing {} bytes", payload.len());

    while pos < payload.len() {
        if pos + 2 > payload.len() {
            break;
        }

        // Read NAL unit size (16 bits)
        let nal_size = u16::from_be_bytes([payload[pos], payload[pos + 1]]) as usize;
        pos += 2;

        if pos + nal_size > payload.len() {
            warn!("Invalid STAP-A NAL unit size: {}", nal_size);
            break;
        }

        // Add start code and NAL unit
        result.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]);
        result.extend_from_slice(&payload[pos..pos + nal_size]);

        pos += nal_size;
    }

    trace!("STAP-A: Extracted {} bytes total", result.len());
    result
}
