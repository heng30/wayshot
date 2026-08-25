use crate::{
    EventSender, PacketData, PacketDataReceiver, WebRTCError, session::WebRTCServerSessionConfig,
};
use derive_setters::Setters;
use rtc::{
    ice::network_type::NetworkType,
    interceptor::Registry,
    media::Sample,
    media_stream::MediaStreamTrack,
    peer_connection::configuration::media_engine::{MIME_TYPE_H264, MIME_TYPE_OPUS, MediaEngine},
    peer_connection::transport::RTCIceCandidateType,
    rtp_transceiver::{
        PayloadType,
        rtp_sender::{RTCRtpCodec, RTCRtpCodingParameters, RTCRtpEncodingParameters, RtpCodecKind},
    },
};
use std::{net::SocketAddr, str::FromStr, sync::Arc, time::Duration};
use tokio::sync::broadcast;
use webrtc::{
    media_stream::track_local::{
        TrackLocal, TrackLocalEvent, static_sample::TrackLocalStaticSample,
    },
    peer_connection::{
        PeerConnection, PeerConnectionBuilder, PeerConnectionEventHandler, RTCConfigurationBuilder,
        RTCIceConnectionState, RTCIceGatheringState, RTCIceServer, RTCPeerConnectionState,
        RTCSessionDescription, SettingEngine, register_default_interceptors,
    },
    rtp_transceiver::RtpSender,
};

pub type Result<T> = std::result::Result<T, WebRTCError>;
pub const ICE_SERVERS: [&str; 1] = ["stun:stun.l.google.com:19302"];

#[non_exhaustive]
#[derive(Debug, Setters, Clone)]
#[setters[prefix = "with_"]]
pub struct WhepConfig {
    pub ice_servers: Vec<RTCIceServer>,
    pub host_ips: Vec<String>,
    pub socket_addr: SocketAddr,
    pub disable_host_ipv6: bool,
}

impl WhepConfig {
    pub fn new(socket_addr: SocketAddr) -> Self {
        Self {
            socket_addr,
            host_ips: vec![],
            disable_host_ipv6: false,
            ice_servers: vec![],
        }
    }
}

impl From<WebRTCServerSessionConfig> for WhepConfig {
    fn from(config: WebRTCServerSessionConfig) -> Self {
        Self {
            host_ips: config.host_ips,
            disable_host_ipv6: config.media_info.disable_host_ipv6,
            ice_servers: config.media_info.ice_servers,
            socket_addr: SocketAddr::from_str("0.0.0.0:9090").unwrap(),
        }
    }
}

#[derive(Clone)]
struct WhepHandler {
    gather_complete_tx: broadcast::Sender<()>,
    state_tx: broadcast::Sender<RTCPeerConnectionState>,
}

#[async_trait::async_trait]
impl PeerConnectionEventHandler for WhepHandler {
    async fn on_ice_gathering_state_change(&self, state: RTCIceGatheringState) {
        if state == RTCIceGatheringState::Complete {
            log::info!("ICE Gathering is complete");
            let _ = self.gather_complete_tx.send(());
        }
    }

    async fn on_ice_connection_state_change(&self, state: RTCIceConnectionState) {
        log::info!("Connection State has changed {state}");
    }

    async fn on_connection_state_change(&self, state: RTCPeerConnectionState) {
        log::info!("Peer Connection State has changed: {state}");

        // Wait until PeerConnection has had no network activity for 30 seconds or another failure. It may be reconnected using an ICE Restart.
        // Use webrtc.PeerConnectionStateDisconnected if you are interested in detecting faster timeout.
        // Note that the PeerConnection may come back from PeerConnectionStateDisconnected.
        if state == RTCPeerConnectionState::Failed {
            log::info!("Peer Connection has gone to failed exiting: Done forwarding");
        }

        if let Err(e) = self.state_tx.send(state) {
            log::warn!("on_connection_state_change send state failed: {e}");
        }
    }
}

/// 协商完成后从 sender 中解析出指定 codec 的 payload type。
///
/// webrtc 0.20 的 `write_sample` 需要显式指定 payload type 与 SSRC，
/// 而 payload type 只有在 SDP 协商完成后才能确定，因此这里在协商后查询。
async fn negotiated_payload_type(
    sender: &Arc<dyn RtpSender>,
    mime_type: &str,
) -> Option<PayloadType> {
    let params = sender.get_parameters().await.ok()?;
    params
        .rtp_parameters
        .codecs
        .iter()
        .find(|c| c.rtp_codec.mime_type.to_lowercase() == mime_type.to_lowercase())
        .map(|c| c.payload_type)
}

/// 读取本端 track 收到的 RTCP 反馈（PLI/FIR/RR 等），避免反馈队列积压。
async fn drain_local_track_rtcp(track: Arc<TrackLocalStaticSample>) {
    while let Some(evt) = track.poll().await {
        match evt {
            TrackLocalEvent::OnRtcpPacket(packets) => {
                log::trace!("received {} rtcp feedback packet(s)", packets.len());
            }
        }
    }
}

pub async fn handle_whep(
    config: WhepConfig,
    offer: RTCSessionDescription,
    mut receiver: PacketDataReceiver,
    event_sender: EventSender,
) -> Result<(RTCSessionDescription, Arc<dyn PeerConnection>)> {
    let mut m = MediaEngine::default();
    m.register_default_codecs()?;

    let registry = register_default_interceptors(Registry::new(), &mut m)?;

    log::info!("whep config: {:#?}", config);

    let mut setting_engine = SettingEngine::default();
    if config.disable_host_ipv6 {
        setting_engine.set_network_types(vec![NetworkType::Tcp4, NetworkType::Udp4]);
    }
    if !config.host_ips.is_empty() {
        setting_engine.set_nat_1to1_ips(config.host_ips, RTCIceCandidateType::Host);
    }

    let ice_servers = if config.ice_servers.is_empty() {
        vec![RTCIceServer {
            urls: ICE_SERVERS
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>(),
            ..Default::default()
        }]
    } else {
        config.ice_servers.clone()
    };

    let rtc_peer_config = RTCConfigurationBuilder::new()
        .with_ice_servers(ice_servers)
        .build();

    let (gather_complete_tx, mut gather_complete_rx) = broadcast::channel(1);
    let (state_tx, state_rx) = broadcast::channel(1);

    let handler = Arc::new(WhepHandler {
        gather_complete_tx,
        state_tx,
    });

    let peer_connection: Arc<dyn PeerConnection> = Arc::new(
        PeerConnectionBuilder::new()
            .with_configuration(rtc_peer_config)
            .with_media_engine(m)
            .with_interceptor_registry(registry)
            .with_setting_engine(setting_engine)
            .with_handler(handler)
            .with_udp_addrs(vec!["0.0.0.0:0".to_owned()])
            .build()
            .await?,
    );

    let video_ssrc = rand::random::<u32>();
    let audio_ssrc = rand::random::<u32>();

    let video_track: Arc<TrackLocalStaticSample> =
        Arc::new(TrackLocalStaticSample::new(MediaStreamTrack::new(
            "whep-video-stream".to_owned(),
            "whep-video-track".to_owned(),
            "webrtc-rs".to_owned(),
            RtpCodecKind::Video,
            vec![RTCRtpEncodingParameters {
                rtp_coding_parameters: RTCRtpCodingParameters {
                    ssrc: Some(video_ssrc),
                    ..Default::default()
                },
                codec: RTCRtpCodec {
                    mime_type: MIME_TYPE_H264.to_owned(),
                    clock_rate: 90000,
                    channels: 0,
                    sdp_fmtp_line: "".to_owned(),
                    rtcp_feedback: vec![],
                },
                ..Default::default()
            }],
        ))?);

    let audio_track: Arc<TrackLocalStaticSample> =
        Arc::new(TrackLocalStaticSample::new(MediaStreamTrack::new(
            "whep-audio-stream".to_owned(),
            "whep-audio-track".to_owned(),
            "webrtc-rs".to_owned(),
            RtpCodecKind::Audio,
            vec![RTCRtpEncodingParameters {
                rtp_coding_parameters: RTCRtpCodingParameters {
                    ssrc: Some(audio_ssrc),
                    ..Default::default()
                },
                codec: RTCRtpCodec {
                    mime_type: MIME_TYPE_OPUS.to_owned(),
                    clock_rate: 48000,
                    channels: 2,
                    sdp_fmtp_line: "".to_owned(),
                    rtcp_feedback: vec![],
                },
                ..Default::default()
            }],
        ))?);

    let video_rtp_sender = peer_connection
        .add_track(Arc::clone(&video_track) as Arc<dyn TrackLocal>)
        .await?;

    let audio_rtp_sender = peer_connection
        .add_track(Arc::clone(&audio_track) as Arc<dyn TrackLocal>)
        .await?;

    // webrtc 0.20 中 RTCP 反馈经由 `TrackLocal::poll` 交付，替代旧版 `RtpSender::read`
    tokio::spawn(drain_local_track_rtcp(video_track.clone()));
    tokio::spawn(drain_local_track_rtcp(audio_track.clone()));

    peer_connection.set_remote_description(offer).await?;
    let answer = peer_connection.create_answer(None).await?;
    peer_connection.set_local_description(answer).await?;

    _ = gather_complete_rx.recv().await;

    let socket_addr = config.socket_addr.to_string();
    tokio::spawn(async move {
        // 等待协商完成以确定 payload type（协商完成后立即可得）
        let (video_payload_type, audio_payload_type) = loop {
            if let (Some(vpt), Some(apt)) = (
                negotiated_payload_type(&video_rtp_sender, MIME_TYPE_H264).await,
                negotiated_payload_type(&audio_rtp_sender, MIME_TYPE_OPUS).await,
            ) {
                break (vpt, apt);
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        };

        let mut state_rx = state_rx;
        loop {
            tokio::select! {
                av_data = receiver.recv() =>{
                    match av_data {
                        Ok(data) =>{
                            match data {
                                PacketData::Video { timestamp: _timestamp, data } => {
                                    log::trace!("{:?}: sending video data ({}) bytes", _timestamp.elapsed(), data.len());

                                    if let Err(err) = video_track
                                        .sample_writer(video_ssrc, video_payload_type)
                                        .write_sample(&Sample {
                                            data,
                                            duration: Duration::from_secs(1),
                                            ..Default::default()
                                        }).await {
                                        log::warn!("send video data error: {}", err);
                                    }
                                }
                                PacketData::Audio { timestamp: _timestamp, duration, data } => {
                                    log::trace!("{:?}: sending audio data ({}) bytes with {duration:.2?}", _timestamp.elapsed(), data.len());

                                    if let Err(err) = audio_track
                                        .sample_writer(audio_ssrc, audio_payload_type)
                                        .write_sample(&Sample {
                                            data,
                                            duration,
                                            ..Default::default()
                                        }).await {
                                        log::warn!("send audio data error: {}", err);
                                    }
                                }
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(skipped_count)) => {
                            log::info!("Client lagged, skipped {skipped_count} messages. Network might be slow.");
                            continue;
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            log::info!("Broadcast channel source closed.");
                            if let Err(e) = event_sender.send(crate::Event::LocalClosed(socket_addr.clone())) {
                                log::warn!("event_sender send LocalClosed failed: {e}");
                            }
                            break;
                        }
                    }
                }
                pc_state = state_rx.recv() => {
                    match pc_state  {
                        Ok(RTCPeerConnectionState::Failed) | Ok(RTCPeerConnectionState::Closed) => {
                            if let Err(e) = event_sender.send(crate::Event::PeerClosed(socket_addr.clone())) {
                                log::warn!("event_sender send PeerClosed {} failed: {e}", socket_addr.to_string());
                            }
                            break;
                        }
                        Ok(RTCPeerConnectionState::Connected) => {
                            if let Err(e) = event_sender
                                .send(crate::Event::PeerConnected(socket_addr.clone())) {
                                    log::warn!( "event_sender send PeerConnected {} failed: {e}", socket_addr.to_string());
                            }
                        }
                        Err(e) => {
                            log::warn!("state_receiver failed: {e}");
                            break;
                        }
                        _ => (),
                    }

                }
            }
        }
    });

    if let Some(local_desc) = peer_connection.local_description().await {
        Ok((local_desc, peer_connection))
    } else {
        Err(WebRTCError::CanNotGetLocalDescription)
    }
}
