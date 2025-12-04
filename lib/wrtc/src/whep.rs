use crate::{
    EventSender, PacketData, PacketDataReceiver, WebRTCError, session::WebRTCServerSessionConfig,
};
use derive_setters::Setters;
use std::{net::SocketAddr, str::FromStr, sync::Arc};
use tokio::sync::broadcast;
use webrtc::{
    api::{
        APIBuilder,
        interceptor_registry::register_default_interceptors,
        media_engine::{MIME_TYPE_H264, MIME_TYPE_OPUS, MediaEngine},
    },
    ice_transport::{ice_connection_state::RTCIceConnectionState, ice_server::RTCIceServer},
    interceptor::registry::Registry,
    peer_connection::{
        RTCPeerConnection, configuration::RTCConfiguration,
        peer_connection_state::RTCPeerConnectionState,
        sdp::session_description::RTCSessionDescription,
    },
    rtp_transceiver::rtp_codec::RTCRtpCodecCapability,
    track::track_local::{
        TrackLocal, TrackLocalWriter, track_local_static_rtp::TrackLocalStaticRTP,
    },
};

pub type Result<T> = std::result::Result<T, WebRTCError>;

#[derive(Debug, Setters, Clone)]
#[setters[prefix = "with_"]]
pub struct WhepConfig {
    pub ice_servers: Vec<String>,
    pub video_mime_type: String,
    pub audio_mime_type: String,
    pub socket_addr: SocketAddr,
}

impl WhepConfig {
    pub fn new(socket_addr: SocketAddr) -> Self {
        Self {
            ice_servers: vec!["stun:stun.l.google.com:19302".to_owned()],
            video_mime_type: MIME_TYPE_H264.to_owned(),
            audio_mime_type: MIME_TYPE_OPUS.to_owned(),
            socket_addr,
        }
    }
}

impl From<WebRTCServerSessionConfig> for WhepConfig {
    fn from(value: WebRTCServerSessionConfig) -> Self {
        Self {
            ice_servers: value.ice_servers,
            video_mime_type: value.video_mime_type,
            audio_mime_type: value.audio_mime_type,
            socket_addr: SocketAddr::from_str("0.0.0.0").unwrap(),
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

    let api = APIBuilder::new()
        .with_media_engine(m)
        .with_interceptor_registry(registry)
        .build();

    let rtc_peer_config = RTCConfiguration {
        ice_servers: vec![RTCIceServer {
            urls: config.ice_servers,
            ..Default::default()
        }],
        ..Default::default()
    };

    let peer_connection = Arc::new(api.new_peer_connection(rtc_peer_config).await?);

    let video_track = Arc::new(TrackLocalStaticRTP::new(
        RTCRtpCodecCapability {
            mime_type: config.video_mime_type,
            ..Default::default()
        },
        "video".to_owned(),
        "webrtc-rs".to_owned(),
    ));

    let audio_track = Arc::new(TrackLocalStaticRTP::new(
        RTCRtpCodecCapability {
            mime_type: config.audio_mime_type,
            ..Default::default()
        },
        "audio".to_owned(),
        "webrtc-rs".to_owned(),
    ));

    let rtp_sender = peer_connection
        .add_track(Arc::clone(&video_track) as Arc<dyn TrackLocal + Send + Sync>)
        .await?;

    _ = peer_connection
        .add_track(Arc::clone(&audio_track) as Arc<dyn TrackLocal + Send + Sync>)
        .await?;

    tokio::spawn(async move {
        let mut rtcp_buf = vec![0u8; 1500];
        while let Ok((_, _)) = rtp_sender.read(&mut rtcp_buf).await {}
        Result::<()>::Ok(())
    });

    let (state_sender, mut state_receiver) = broadcast::channel(1);

    peer_connection.on_ice_connection_state_change(Box::new(move |s: RTCIceConnectionState| {
        log::info!("Connection State has changed {s}");

        Box::pin(async {})
    }));

    peer_connection.on_peer_connection_state_change(Box::new(move |s: RTCPeerConnectionState| {
        log::info!("Peer Connection State has changed: {s}");

        if s == RTCPeerConnectionState::Failed {
            log::info!("Peer Connection has gone to failed exiting: Done forwarding");
        }

        if let Err(err) = state_sender.send(s) {
            log::warn!("on_peer_connection_state_change send state err: {}", err);
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
                                PacketData::Video { timestamp: _, data } => {
                                    if let Err(err) = video_track.write(&data[..]).await {
                                        log::warn!("send video data error: {}", err);
                                    }
                                }
                                PacketData::Audio { timestamp: _, data } => {
                                    if let Err(err) = audio_track.write(&data[..]).await {
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
                    if pc_state == Ok(RTCPeerConnectionState::Closed) ||
                       pc_state == Ok(RTCPeerConnectionState::Failed) ||
                       pc_state == Ok(RTCPeerConnectionState::Disconnected) {
                        if let Err(e) = event_sender.send(crate::Event::PeerClosed(socket_addr.clone())) {
                            log::warn!("event_sender send PeerClosed {socket_addr} failed: {e}");
                        }
                        break;
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
