use crate::{
    EventSender, PacketDataSender, WebRTCError,
    common::{auth::Auth, http::http_method_name},
    session::{HttpStream, SessionsMap, WebRTCServerSession, WebRTCServerSessionConfig},
};
use derive_setters::Setters;
use rustls_pemfile::{certs, pkcs8_private_keys};
use std::{collections::HashMap, fs::File, io::BufReader, net::SocketAddr, sync::Arc};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{Mutex, Notify},
};
use tokio_rustls::{
    TlsAcceptor,
    rustls::{
        ServerConfig,
        pki_types::{CertificateDer, PrivateKeyDer},
    },
};

#[non_exhaustive]
#[derive(Debug, Setters, Clone)]
#[setters[prefix = "with_"]]
pub struct WebRTCServerConfig {
    pub address: String,
    pub auth_token: Option<String>,
    pub cert_file: Option<String>,
    pub key_file: Option<String>,
    pub enable_https: bool,
}

impl WebRTCServerConfig {
    pub fn new(address: String, auth_token: Option<String>) -> Self {
        Self {
            address,
            auth_token,
            cert_file: None,
            key_file: None,
            enable_https: false,
        }
    }
}

pub struct WebRTCServer {
    config: WebRTCServerConfig,
    session_config: WebRTCServerSessionConfig,
    packet_sender: PacketDataSender,
    event_sender: EventSender,
    exit_notify: Arc<Notify>,
    sessions: SessionsMap,
}

impl WebRTCServer {
    pub fn new(
        config: WebRTCServerConfig,
        session_config: WebRTCServerSessionConfig,
        packet_sender: PacketDataSender,
        event_sender: EventSender,
        exit_notify: Arc<Notify>,
    ) -> Self {
        Self {
            config,
            session_config,
            packet_sender,
            event_sender,
            exit_notify,
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    async fn handle_tls_handshake(
        tcp_stream: TcpStream,
        acceptor: TlsAcceptor,
        socket_addr: SocketAddr,
        config: WebRTCServerSessionConfig,
        sessions: SessionsMap,
        auth: Option<Auth>,
        packet_sender: PacketDataSender,
        event_sender: EventSender,
    ) {
        match acceptor.accept(tcp_stream).await {
            Ok(tls_stream) => {
                log::info!("TLS handshake successful for {socket_addr}");
                let stream = HttpStream::Tls(tls_stream);
                Self::handle_connection(
                    stream,
                    socket_addr,
                    config,
                    sessions,
                    auth,
                    packet_sender,
                    event_sender,
                )
                .await;
            }
            Err(e) => {
                log::warn!("TLS handshake failed for {socket_addr}: {e}");
                if let Err(e) =
                    event_sender.send(crate::Event::LocalClosed(socket_addr.to_string()))
                {
                    log::warn!("event_sender send LocalClosed {socket_addr} failed: {e}");
                }
            }
        }
    }

    async fn handle_connection(
        stream: HttpStream,
        socket_addr: SocketAddr,
        session_config: WebRTCServerSessionConfig,
        sessions: SessionsMap,
        auth: Option<Auth>,
        packet_sender: PacketDataSender,
        event_sender: EventSender,
    ) {
        let session = Arc::new(Mutex::new(WebRTCServerSession::new(
            session_config,
            sessions.clone(),
            auth,
            stream,
            socket_addr,
            packet_sender,
            event_sender.clone(),
        )));

        let event_sender_clone = event_sender.clone();
        tokio::spawn(async move {
            let mut session_unlock = session.lock().await;
            if let Err(e) = session_unlock.run().await {
                log::warn!("session run failed: {e}");

                if let Err(e) =
                    event_sender_clone.send(crate::Event::LocalClosed(socket_addr.to_string()))
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

    pub async fn run(&mut self) -> Result<(), WebRTCError> {
        let socket_addr: &SocketAddr = &self.config.address.parse()?;
        let listener = TcpListener::bind(socket_addr).await?;

        let tls_acceptor = if self.config.enable_https {
            let cert_file = self.config.cert_file.as_ref().ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "Certificate file required when HTTPS is enabled",
                )
            })?;
            let key_file = self.config.key_file.as_ref().ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "Private key file required when HTTPS is enabled",
                )
            })?;

            Some(
                create_tls_acceptor(cert_file, key_file)
                    .await
                    .map_err(|e| {
                        std::io::Error::new(std::io::ErrorKind::InvalidInput, e.to_string())
                    })?,
            )
        } else {
            None
        };

        log::info!(
            "WebRTC server listening on {}://{}",
            if self.config.enable_https {
                "https"
            } else {
                "http"
            },
            socket_addr
        );

        loop {
            tokio::select! {
                item = listener.accept() => {
                    match item {
                        Ok((tcp_stream, socket_addr)) => {
                            if let Err(e) = self
                                .event_sender
                                    .send(crate::Event::PeerConnecting(socket_addr.to_string()))
                            {
                                log::warn!("event_sender send PeerConnecting {socket_addr} failed: {e}");
                            }

                            let sessions = self.sessions.clone();
                            let session_config = self.session_config.clone();
                            let packet_sender = self.packet_sender.clone();
                            let event_sender = self.event_sender.clone();
                            let tls_acceptor = tls_acceptor.clone();

                            let auth = if let Some(token) = self.config.auth_token.clone() {
                                Some( Auth::new(token))
                            } else {
                                None
                            };

                            if let Some(acceptor) = tls_acceptor {
                                tokio::spawn(Self::handle_tls_handshake(
                                    tcp_stream,
                                    acceptor,
                                    socket_addr,
                                    session_config,
                                    sessions,
                                    auth,
                                    packet_sender,
                                    event_sender,
                                ));
                            } else {
                                let stream = HttpStream::Tcp(tcp_stream);
                                Self::handle_connection(
                                    stream,
                                    socket_addr,
                                    session_config,
                                    sessions,
                                    auth,
                                    packet_sender,
                                    event_sender,
                                ).await;
                            }
                        }
                        Err(e) =>  {
                            log::warn!("WebRTCServer accept failed: {e}");
                            return Err(WebRTCError::IOError(e));
                        }
                    }
                }
                _ = self.exit_notify.notified() => {
                    log::info!("WebRTCServer receive `exit notify`. exit...");
                    return Ok(());
                }
            }
        }
    }
}

async fn create_tls_acceptor(cert_file: &str, key_file: &str) -> Result<TlsAcceptor, WebRTCError> {
    let cert_file = File::open(cert_file).map_err(|e| {
        WebRTCError::TlsConfigError(format!("Failed to open certificate file: {}", e))
    })?;
    let mut cert_reader = BufReader::new(cert_file);
    let cert_results: Vec<_> = certs(&mut cert_reader)
        .collect::<Result<_, _>>()
        .map_err(|e| WebRTCError::TlsConfigError(format!("Failed to read certificate: {}", e)))?;
    let certs = cert_results.into_iter().map(CertificateDer::from).collect();

    let key_file = File::open(key_file).map_err(|e| {
        WebRTCError::TlsConfigError(format!("Failed to open private key file: {}", e))
    })?;
    let mut key_reader = BufReader::new(key_file);
    let key_results: Vec<_> = pkcs8_private_keys(&mut key_reader)
        .collect::<Result<_, _>>()
        .map_err(|e| WebRTCError::TlsConfigError(format!("Failed to read private key: {}", e)))?;
    let keys: Vec<_> = key_results.into_iter().map(PrivateKeyDer::Pkcs8).collect();

    let key = keys
        .into_iter()
        .next()
        .ok_or_else(|| WebRTCError::TlsConfigError("No valid private key found".to_string()))?;

    let config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|e| WebRTCError::TlsConfigError(format!("Failed to create TLS config: {}", e)))?;

    Ok(TlsAcceptor::from(Arc::new(config)))
}
