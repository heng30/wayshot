use crate::{
    ClientError, RTCIceServer,
    opus::{OPUS_SAMPLE_RATE, OpusCoder},
    session::MediaInfo,
    whep::ICE_SERVERS,
};
use derive_setters::Setters;
use http::{
    header::{AUTHORIZATION, CONTENT_TYPE},
    {HeaderMap, HeaderValue},
};
use log::{debug, info, trace, warn};
use openh264::decoder::Decoder;
use opus::Channels;
use std::sync::Arc;
use tokio::{
    sync::{Mutex, Notify, mpsc::Sender},
    time::Duration,
};
use webrtc::{
    api::{
        APIBuilder,
        interceptor_registry::register_default_interceptors,
        media_engine::{MIME_TYPE_H264, MIME_TYPE_OPUS, MediaEngine},
        setting_engine::SettingEngine,
    },
    ice::network_type::NetworkType,
    ice_transport::{
        ice_candidate_type::RTCIceCandidateType, ice_connection_state::RTCIceConnectionState,
    },
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

    pub ice_servers: Vec<RTCIceServer>,
    pub host_ips: Vec<String>,
}

impl WHEPClientConfig {
    pub fn new(server_url: String) -> Self {
        Self {
            server_url,
            auth_token: None,
            host_ips: vec![],
            ice_servers: vec![RTCIceServer {
                urls: ICE_SERVERS
                    .iter()
                    .map(|s| s.to_string())
                    .collect::<Vec<_>>(),
                ..Default::default()
            }],
        }
    }
}

pub struct WHEPClient {
    pub config: WHEPClientConfig,
    pub media_info: MediaInfo,
    pub video_sender: Option<Sender<RGBFrame>>,
    pub audio_sender: Option<Sender<AudioSamples>>,
    pub exit_notify: Arc<Notify>,

    session_url: Arc<Mutex<Option<String>>>,
}

impl WHEPClient {
    pub async fn new(
        config: WHEPClientConfig,
        video_sender: Option<Sender<RGBFrame>>,
        audio_sender: Option<Sender<AudioSamples>>,
        exit_notify: Arc<Notify>,
    ) -> ClientResult<WHEPClient> {
        info!("config: {:#?}", config);

        let media_info =
            match fetch_media_info(&config.server_url, config.auth_token.as_ref()).await {
                Ok(info) => info,
                Err(e) => {
                    warn!("Failed to fetch media info: {e}");
                    return Err(ClientError::ConnectionError(format!(
                        "Failed to fetch media info: {e}"
                    )));
                }
            };

        info!("media_info: {:#?}", media_info);

        Ok(Self {
            config,
            media_info,
            video_sender,
            audio_sender,
            exit_notify,
            session_url: Arc::new(Mutex::new(None)),
        })
    }

    // it should called after `new()` and before `connect()`
    pub fn update_video_sender(&mut self, sender: Sender<RGBFrame>) {
        self.video_sender = Some(sender);
    }

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

        if let Some(ref audio_info) = self.media_info.audio {
            m.register_codec(
                RTCRtpCodecParameters {
                    capability: RTCRtpCodecCapability {
                        mime_type: MIME_TYPE_OPUS.to_owned(),
                        clock_rate: OPUS_SAMPLE_RATE as u32,
                        channels: audio_info.channels,
                        sdp_fmtp_line: "".to_owned(),
                        rtcp_feedback: vec![],
                    },
                    payload_type: 111,
                    ..Default::default()
                },
                RTPCodecType::Audio,
            )?;
        }

        let mut registry = Registry::new();
        registry = register_default_interceptors(registry, &mut m)?;

        let mut setting_engine = SettingEngine::default();
        if self.media_info.disable_host_ipv6 {
            setting_engine.set_network_types(vec![NetworkType::Tcp4, NetworkType::Udp4]);
        }
        if !self.config.host_ips.is_empty() {
            setting_engine
                .set_nat_1to1_ips(self.config.host_ips.clone(), RTCIceCandidateType::Host);
        }

        let api = APIBuilder::new()
            .with_media_engine(m)
            .with_interceptor_registry(registry)
            .with_setting_engine(setting_engine)
            .build();

        let ice_servers = if self.media_info.ice_servers.is_empty() {
            self.config.ice_servers.clone()
        } else {
            self.media_info.ice_servers.clone()
        };

        let config = RTCConfiguration {
            ice_servers,
            ..Default::default()
        };

        let peer_connection = Arc::new(api.new_peer_connection(config).await?);
        peer_connection
            .add_transceiver_from_kind(RTPCodecType::Video, None)
            .await?;

        if self.media_info.audio.is_some() {
            peer_connection
                .add_transceiver_from_kind(RTPCodecType::Audio, None)
                .await?;
        }

        let exit_notify = self.exit_notify.clone();
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

            let exit_notify = exit_notify.clone();
            let media_info = media_info.clone();
            let mut video_sender = video_sender.clone();
            let mut audio_sender = audio_sender.clone();

            Box::pin(async move {
                let codec = track.codec();
                let mime_type = codec.capability.mime_type.to_lowercase();
                if mime_type == MIME_TYPE_OPUS.to_lowercase() {
                    info!("Got Opus track, processing audio");

                    if let Some(audio_info) = media_info.audio
                        && let Some(sender) = audio_sender.take()
                    {
                        tokio::spawn(async move {
                            _ = process_audio_track(
                                track,
                                sender,
                                exit_notify,
                                audio_info.sample_rate,
                                audio_info.channels,
                            )
                            .await;
                        });
                    }
                } else if mime_type == MIME_TYPE_H264.to_lowercase() {
                    info!("Got H264 track, processing video");
                    if let Some(sender) = video_sender.take() {
                        tokio::spawn(async move {
                            _ = process_video_track(
                                track,
                                sender,
                                exit_notify,
                                media_info.video.width as u32,
                                media_info.video.height as u32,
                            )
                            .await;
                        });
                    }
                }
            })
        }));

        let exit_notify = self.exit_notify.clone();

        peer_connection.on_ice_connection_state_change(Box::new(
            move |connection_state: RTCIceConnectionState| {
                debug!("Connection State has changed {connection_state}");

                if connection_state == RTCIceConnectionState::Connected {
                    info!("WHEP client connected to server");
                } else if connection_state == RTCIceConnectionState::Failed {
                    warn!("WHEP client connection failed");
                    exit_notify.notify_waiters();
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
            let client = reqwest::Client::builder()
                .danger_accept_invalid_certs(true)
                .danger_accept_invalid_hostnames(true)
                .build()
                .map_err(|e| {
                    ClientError::ConnectionError(format!("Failed to build HTTP client: {}", e))
                })?;
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
                if let Some(location) = response.headers().get("location") {
                    if let Ok(location_str) = location.to_str() {
                        let session_url_str = format!("{}{}", self.config.server_url, location_str);
                        info!("Session URL: {}", session_url_str);
                        *self.session_url.lock().await = Some(session_url_str);
                    }
                }

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

        _ = self.exit_notify.notified().await;

        info!("WHEP client connection ended");

        peer_connection.close().await?;
        self.close_session().await?;
        Ok(())
    }

    pub async fn close_session(&self) -> ClientResult<()> {
        let session_url = {
            let url_guard = self.session_url.lock().await;
            match url_guard.as_ref() {
                Some(url) => url.clone(),
                None => {
                    warn!("No session URL available - session may not be established");
                    return Ok(());
                }
            }
        };

        info!("Closing WHEP session: {}", session_url);

        let client = reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .danger_accept_invalid_hostnames(true)
            .build()
            .map_err(|e| {
                ClientError::ConnectionError(format!("Failed to build HTTP client: {}", e))
            })?;

        let mut headers = HeaderMap::new();
        if let Some(token) = &self.config.auth_token {
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {token}")).unwrap(),
            );
        }

        let response = client.delete(&session_url).headers(headers).send().await?;

        if response.status().is_success() {
            info!("WHEP session closed successfully");
        } else {
            warn!(
                "Failed to close WHEP session: {} - {}",
                response.status(),
                response.text().await.unwrap_or_default()
            );
        }

        *self.session_url.lock().await = None;

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

    // FU-A fragment buffer
    let mut fragment_buffer: Vec<u8> = Vec::with_capacity(5 * 1024 * 1024);
    let mut frame_assembling = false;

    loop {
        tokio::select! {
            result = track.read_rtp() => {
                if let Ok((rtp_packet, _)) = result {
                    let payload = rtp_packet.payload;

                    let h264_data_chunks = handle_h264_rtp_payload(&payload, &mut fragment_buffer, &mut frame_assembling);

                    for nal_unit in h264_data_chunks {
                         match h264_decoder.decode(&nal_unit) {
                            Ok(rgb_frame) => {
                                if let Err(e) = video_sender.try_send(rgb_frame) {
                                    warn!("video_sender try send failed: {e}");
                                }
                            }
                            Err(e) =>  trace!("H264 decoding attempt failed: {e:?}"),
                        }
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
                            if let Err(e) = audio_sender.try_send(audio_samples) {
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

fn handle_h264_rtp_payload(
    payload: &[u8],
    fragment_buffer: &mut Vec<u8>,
    assembling: &mut bool,
) -> Vec<Vec<u8>> {
    if payload.is_empty() {
        return Vec::new();
    }

    let nal_header = payload[0];
    let nal_type = nal_header & 0x1F;

    // NAL Header: [F|NRI|Type]
    // F: 1 bit (forbidden_zero_bit)
    // NRI: 2 bits (nal_ref_idc)
    // Type: 5 bits (nal_unit_type)

    match nal_type {
        // Single NAL Unit types
        1 | 5 | 7 | 8 => {
            *assembling = false;
            let mut result = Vec::with_capacity(payload.len() + 4);
            result.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]);
            result.extend_from_slice(payload);
            vec![result]
        }
        // STAP-A (Single-Time Aggregation Packet) - usually contains SPS/PPS
        24 => {
            *assembling = false;
            parse_stap_a_multiple(payload)
        }
        // FU-A (Fragmentation Unit)
        28 => parse_fu_a(payload, fragment_buffer, assembling),
        _ => {
            trace!("Skipping unknown/unsupported NAL unit type: {}", nal_type);
            Vec::new()
        }
    }
}

fn parse_fu_a(payload: &[u8], buffer: &mut Vec<u8>, assembling: &mut bool) -> Vec<Vec<u8>> {
    if payload.len() < 2 {
        return Vec::new();
    }

    // FU Indicator (Byte 0): [F|NRI|28]
    let fu_indicator = payload[0];

    // FU Header (Byte 1): [S|E|R|Type]
    let fu_header = payload[1];
    let is_start = (fu_header & 0x80) != 0;
    let is_end = (fu_header & 0x40) != 0;
    let original_nal_type = fu_header & 0x1F;

    // header packet
    if is_start {
        // Orginal Header = (FU Indicator & 0xE0) | (Original Type & 0x1F)
        let reconstructed_header = (fu_indicator & 0xE0) | original_nal_type;

        // Start Code
        buffer.clear();
        buffer.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]);
        buffer.push(reconstructed_header);

        // skip FU Indicator and FU Header (2bytes)
        buffer.extend_from_slice(&payload[2..]);
        *assembling = true;

        // It'a a  Start and End packet
        if is_end {
            *assembling = false;
            return vec![buffer.clone()];
        }

        Vec::new() // wait for more data
    } else if *assembling {
        // nono header packets
        buffer.extend_from_slice(&payload[2..]);

        if is_end {
            // end packet, return NAL
            *assembling = false;
            vec![buffer.clone()]
        } else {
            Vec::new() // wait for more data
        }
    } else {
        trace!("Received FU-A middle/end packet without start, dropping.");
        Vec::new()
    }
}

fn parse_stap_a_multiple(payload: &[u8]) -> Vec<Vec<u8>> {
    let mut results = Vec::new();
    let mut pos = 1; // Skip STAP-A header

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
        let mut nal_unit = Vec::with_capacity(nal_size + 4);
        nal_unit.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]);
        nal_unit.extend_from_slice(&payload[pos..pos + nal_size]);
        results.push(nal_unit);

        pos += nal_size;
    }
    results
}

/// Convert H.264 NAL units from Annex B format (start codes) to length-prefixes format
/// This is required for MP4 container format, which uses 4-byte length prefixes instead of start codes
pub fn convert_annexb_to_length_prefixes(annexb_data: &[u8]) -> Vec<u8> {
    let mut i = 0;
    let mut length_preference_data = Vec::new();

    if annexb_data.is_empty() {
        return length_preference_data;
    }

    // Find first start code, ignoring leading garbage data
    while i + 2 < annexb_data.len() {
        if annexb_data[i] == 0 && annexb_data[i + 1] == 0 && annexb_data[i + 2] == 1 {
            break; // Found 3-byte start code
        }
        if i + 3 < annexb_data.len()
            && annexb_data[i] == 0
            && annexb_data[i + 1] == 0
            && annexb_data[i + 2] == 0
            && annexb_data[i + 3] == 1
        {
            break; // Found 4-byte start code
        }
        i += 1;
    }

    while i < annexb_data.len() {
        // 1. Determine current start code length
        let start_code_len = if i + 3 < annexb_data.len()
            && annexb_data[i] == 0
            && annexb_data[i + 1] == 0
            && annexb_data[i + 2] == 0
            && annexb_data[i + 3] == 1
        {
            4
        } else if i + 2 < annexb_data.len()
            && annexb_data[i] == 0
            && annexb_data[i + 1] == 0
            && annexb_data[i + 2] == 1
        {
            3
        } else {
            // No valid start code found, we're done
            break;
        };

        let nal_start = i + start_code_len;
        let mut nal_end = annexb_data.len();

        // 2. Find next start code (as end of current NAL)
        // Optimization: search from nal_start
        let mut j = nal_start;
        while j + 2 < annexb_data.len() {
            if annexb_data[j] == 0 && annexb_data[j + 1] == 0 {
                if annexb_data[j + 2] == 1 {
                    // Found next 00 00 01
                    nal_end = j;
                    break;
                } else if j + 3 < annexb_data.len()
                    && annexb_data[j + 2] == 0
                    && annexb_data[j + 3] == 1
                {
                    // Found next 00 00 00 01
                    nal_end = j;
                    break;
                }
            }
            j += 1;
        }

        let nal_data = &annexb_data[nal_start..nal_end];
        let nal_size = nal_data.len();

        if nal_size > 0 {
            length_preference_data.extend_from_slice(&(nal_size as u32).to_be_bytes());
            length_preference_data.extend_from_slice(nal_data);
        }

        i = nal_end;
    }

    length_preference_data
}

async fn fetch_media_info(
    server_url: &str,
    auth_token: Option<&String>,
) -> ClientResult<MediaInfo> {
    let mut client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .danger_accept_invalid_hostnames(true);

    if let Some(token) = &auth_token {
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {token}")).unwrap(),
        );
        client = client.default_headers(headers);
    };

    let client = client
        .build()
        .map_err(|e| ClientError::ConnectionError(format!("Failed to build HTTP client: {}", e)))?;

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
