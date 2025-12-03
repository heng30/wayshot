use crate::{
    EventSender, PacketDataSender,
    common::{auth::Auth, http::http_method_name, uuid::Uuid},
    session::WebRTCServerSession,
};
use std::{collections::HashMap, net::SocketAddr, str::FromStr, sync::Arc};
use tokio::{io::Error, net::TcpListener, sync::Mutex};

pub struct WebRTCServer {
    address: String,
    auth: Option<Auth>,
    packet_sender: PacketDataSender,
    event_sender: EventSender,
    uuid_2_sessions: Arc<Mutex<HashMap<Uuid, Arc<Mutex<WebRTCServerSession>>>>>,
}

impl WebRTCServer {
    pub fn new(
        address: String,
        auth: Option<Auth>,
        packet_sender: PacketDataSender,
        event_sender: EventSender,
    ) -> Self {
        Self {
            address,
            auth,
            packet_sender,
            event_sender,
            uuid_2_sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn run(&mut self) -> Result<(), Error> {
        let socket_addr: &SocketAddr = &self
            .address
            .parse()
            .unwrap_or(SocketAddr::from_str("0.0.0.0").unwrap());
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

            let session = Arc::new(Mutex::new(WebRTCServerSession::new(
                tcp_stream,
                socket_addr,
                self.packet_sender.clone(),
                self.event_sender.clone(),
                self.auth.clone(),
            )));

            let uuid_2_sessions = self.uuid_2_sessions.clone();

            tokio::spawn(async move {
                let mut session_unlock = session.lock().await;
                if let Err(e) = session_unlock.run(uuid_2_sessions.clone()).await {
                    log::warn!("session run failed: {e}");
                }

                if let Some(http_request_data) = &session_unlock.http_request_data {
                    let mut uuid_2_session_unlock = uuid_2_sessions.lock().await;

                    match http_request_data.method.as_str() {
                        http_method_name::POST => {
                            if let Some(uuid) = session_unlock.session_id {
                                uuid_2_session_unlock.insert(uuid, session.clone());
                            }
                        }
                        _ => {}
                    }
                }
            });
        }
    }
}
