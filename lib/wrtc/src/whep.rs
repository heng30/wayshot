use crate::{
    EventSender, PacketData, PacketDataReceiver, WebRTCError, session::WebRTCServerSessionConfig,
};
use derive_setters::Setters;
use std::{net::SocketAddr, str::FromStr, sync::Arc, time::Duration};
use tokio::sync::broadcast;
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
        ice_server::RTCIceServer,
    },
    interceptor::registry::Registry,
    media::Sample,
    peer_connection::{
        RTCPeerConnection, configuration::RTCConfiguration,
        peer_connection_state::RTCPeerConnectionState,
        sdp::session_description::RTCSessionDescription,
    },
    rtp_transceiver::rtp_codec::RTCRtpCodecCapability,
    track::track_local::{TrackLocal, track_local_static_sample::TrackLocalStaticSample},
};

pub type Result<T> = std::result::Result<T, WebRTCError>;
pub const ICE_SERVERS: [&str; 2] = [
    "stun:stun.nextcloud.com:443",
    "stun:stun.l.google.com:19302",
];

#[non_exhaustive]
#[derive(Debug, Setters, Clone)]
#[setters[prefix = "with_"]]
pub struct WhepConfig {
    pub ice_servers: Vec<String>,
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
            ice_servers: ICE_SERVERS
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>(),
        }
    }
}

impl From<WebRTCServerSessionConfig> for WhepConfig {
    fn from(value: WebRTCServerSessionConfig) -> Self {
        Self {
            host_ips: value.host_ips,
            disable_host_ipv6: value.disable_host_ipv6,
            ice_servers: value.media_info.ice_servers,
            socket_addr: SocketAddr::from_str("0.0.0.0:9090").unwrap(),
        }
    }
}

pub async fn handle_whep(
    config: WhepConfig,
    offer: RTCSessionDescription,
    mut receiver: PacketDataReceiver,
    event_sender: EventSender,
) -> Result<(RTCSessionDescription, Arc<RTCPeerConnection>)> {
    let mut m = MediaEngine::default();
    m.register_default_codecs()?;

    let mut registry = Registry::new();
    registry = register_default_interceptors(registry, &mut m)?;

    log::info!("host_ips: {:?}", config.host_ips);

    let mut api = APIBuilder::new()
        .with_media_engine(m)
        .with_interceptor_registry(registry);

    if !config.host_ips.is_empty() {
        let mut setting_engine = SettingEngine::default();

        if config.disable_host_ipv6 {
            setting_engine.set_network_types(vec![NetworkType::Tcp4, NetworkType::Udp4]);
        }

        setting_engine.set_nat_1to1_ips(config.host_ips, RTCIceCandidateType::Host);
        api = api.with_setting_engine(setting_engine);
    }
    let api = api.build();

    let rtc_peer_config = RTCConfiguration {
        ice_servers: vec![RTCIceServer {
            urls: config.ice_servers,
            ..Default::default()
        }],
        ..Default::default()
    };

    let peer_connection = Arc::new(api.new_peer_connection(rtc_peer_config).await?);

    let video_track = Arc::new(TrackLocalStaticSample::new(
        RTCRtpCodecCapability {
            mime_type: MIME_TYPE_H264.to_string(),
            ..Default::default()
        },
        "video".to_owned(),
        "webrtc-rs".to_owned(),
    ));

    let audio_track = Arc::new(TrackLocalStaticSample::new(
        RTCRtpCodecCapability {
            mime_type: MIME_TYPE_OPUS.to_string(),
            ..Default::default()
        },
        "audio".to_owned(),
        "webrtc-rs".to_owned(),
    ));

    let video_rtp_sender = peer_connection
        .add_track(Arc::clone(&video_track) as Arc<dyn TrackLocal + Send + Sync>)
        .await?;

    let audio_rtp_sender = peer_connection
        .add_track(Arc::clone(&audio_track) as Arc<dyn TrackLocal + Send + Sync>)
        .await?;

    tokio::spawn(async move {
        let mut rtcp_buf = vec![0u8; 1500];
        while let Ok((_, _)) = video_rtp_sender.read(&mut rtcp_buf).await {}
        Result::<()>::Ok(())
    });

    tokio::spawn(async move {
        let mut rtcp_buf = vec![0u8; 1500];
        while let Ok((_, _)) = audio_rtp_sender.read(&mut rtcp_buf).await {}
        Result::<()>::Ok(())
    });

    peer_connection.on_ice_connection_state_change(Box::new(move |s: RTCIceConnectionState| {
        log::info!("Connection State has changed {s}");

        Box::pin(async {})
    }));

    let (state_sender, mut state_receiver) = broadcast::channel(1);
    peer_connection.on_peer_connection_state_change(Box::new(move |s: RTCPeerConnectionState| {
        log::info!("Peer Connection State has changed: {s}");

        // Wait until PeerConnection has had no network activity for 30 seconds or another failure. It may be reconnected using an ICE Restart.
        // Use webrtc.PeerConnectionStateDisconnected if you are interested in detecting faster timeout.
        // Note that the PeerConnection may come back from PeerConnectionStateDisconnected.
        if s == RTCPeerConnectionState::Failed {
            log::info!("Peer Connection has gone to failed exiting: Done forwarding");
        }

        if let Err(e) = state_sender.send(s) {
            log::warn!("on_peer_connection_state_change send state failed: {e}");
        }

        Box::pin(async {})
    }));

    peer_connection.set_remote_description(offer).await?;
    let answer = peer_connection.create_answer(None).await?;

    let mut gather_complete = peer_connection.gathering_complete_promise().await;
    peer_connection.set_local_description(answer).await?;
    _ = gather_complete.recv().await;

    let socket_addr = config.socket_addr.to_string();
    tokio::spawn(async move {
        loop {
            tokio::select! {
                av_data = receiver.recv() =>{
                    match av_data {
                        Ok(data) =>{
                            match data {
                                PacketData::Video { timestamp: _timestamp, data } => {
                                    log::trace!("{:?}: sending video data ({}) bytes", _timestamp.elapsed(), data.len());

                                    if let Err(err) = video_track
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

                                    if let Err(err) = audio_track.write_sample(&Sample {
                                        data,
                                        duration,
                                        ..Default::default()
                                    }).await {
                                        log::warn!("send audio data error: {}", err);
                                    }
                                }
                            }
                        }
                        _ => {
                            if let Err(e) = event_sender.send(crate::Event::LocalClosed(socket_addr.clone())) {
                                log::warn!("event_sender send LocalClosed {socket_addr} failed: {e}");
                            }
                            break;
                        }
                    }
                }
                pc_state = state_receiver.recv() => {
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
