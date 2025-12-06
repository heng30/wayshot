use crate::{
    EventSender, PacketDataSender,
    common::{auth::Auth, http::http_method_name},
    session::{SessionsMap, WebRTCServerSession, WebRTCServerSessionConfig},
};
use std::{collections::HashMap, net::SocketAddr, str::FromStr, sync::Arc};
use tokio::{io::Error, net::TcpListener, sync::Mutex};

pub struct WebRTCServer {
    config: WebRTCServerSessionConfig,
    address: String,
    auth: Option<Auth>,
    packet_sender: PacketDataSender,
    event_sender: EventSender,
    sessions: SessionsMap,
}

impl WebRTCServer {
    pub fn new(
        config: WebRTCServerSessionConfig,
        address: String,
        auth: Option<Auth>,
        packet_sender: PacketDataSender,
        event_sender: EventSender,
    ) -> Self {
        Self {
            config,
            address,
            auth,
            packet_sender,
            event_sender,
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn run(&mut self) -> Result<(), Error> {
        let socket_addr: &SocketAddr = &self
            .address
            .parse()
            .unwrap_or_else(|_| SocketAddr::from_str("0.0.0.0:8080").unwrap());
        let listener = TcpListener::bind(socket_addr).await?;

        log::info!("WebRTC server listening on tcp://{}", socket_addr);

        loop {
            let (tcp_stream, socket_addr) = listener.accept().await?;
            if let Err(e) = self
                .event_sender
                .send(crate::Event::PeerConnecting(socket_addr.to_string()))
            {
                log::warn!("event_sender send PeerConnecting {socket_addr} failed: {e}");
            }

            let sessions = self.sessions.clone();
            let session = Arc::new(Mutex::new(WebRTCServerSession::new(
                self.config.clone(),
                sessions.clone(),
                self.auth.clone(),
                tcp_stream,
                socket_addr,
                self.packet_sender.clone(),
                self.event_sender.clone(),
            )));

            let event_sender = self.event_sender.clone();
            tokio::spawn(async move {
                let mut session_unlock = session.lock().await;
                if let Err(e) = session_unlock.run().await {
                    log::warn!("session run failed: {e}");

                    if let Err(e) =
                        event_sender.send(crate::Event::LocalClosed(socket_addr.to_string()))
                    {
                        log::warn!("event_sender send LocalClosed {socket_addr} failed: {e}");
                    }
                    return;
                }

                if let Some(http_request_data) = &session_unlock.http_request_data
                    && matches!(http_request_data.method.as_str(), http_method_name::POST)
                    && let Some(uuid) = session_unlock.session_id
                {
                    sessions.lock().await.insert(uuid, session.clone());
                }
            });
        }
    }
}
