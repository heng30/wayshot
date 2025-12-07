use crate::{
    EventSender, PacketDataSender, SessionError,
    common::{
        self,
        auth::{Auth, SecretCarrier},
        http::{
            HttpRequest, HttpResponse, Marshal as HttpMarshal, Unmarshal as HttpUnmarshal,
            http_method_name,
        },
        uuid::{RandomDigitCount, Uuid},
    },
    whep::{ICE_SERVERS, WhepConfig, handle_whep},
};
use bytes::BytesMut;
use bytesio::{
    bytes_reader::BytesReader, bytes_writer::AsyncBytesWriter, bytesio::TNetIO, bytesio::TcpIO,
};
use derive_setters::Setters;
use http::StatusCode;
use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use tokio::{net::TcpStream, sync::Mutex};
use webrtc::peer_connection::{RTCPeerConnection, sdp::session_description::RTCSessionDescription};

static WEB_WHEP_JS: &str = include_str!("../web-whep-client/whep.js");
static WEB_WHEP_INDEX: &str = include_str!("../web-whep-client/index.html");
static WEB_FAVICON: &[u8] = include_bytes!("../../../wayshot/windows/icon.ico");

#[derive(Debug, Setters, Clone)]
#[setters[prefix = "with_"]]
pub struct WebRTCServerSessionConfig {
    pub ice_servers: Vec<String>,
}

impl Default for WebRTCServerSessionConfig {
    fn default() -> Self {
        Self {
            ice_servers: ICE_SERVERS
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>(),
        }
    }
}

pub type SessionsMap = Arc<Mutex<HashMap<Uuid, Arc<Mutex<WebRTCServerSession>>>>>;

pub struct WebRTCServerSession {
    config: WebRTCServerSessionConfig,

    io: Arc<Mutex<Box<dyn TNetIO + Send + Sync>>>,
    socket_addr: SocketAddr,
    reader: BytesReader,
    writer: AsyncBytesWriter,
    packet_sender: PacketDataSender,
    event_sender: EventSender,

    sessions: SessionsMap,
    auth: Option<Auth>,

    pub session_id: Option<Uuid>,
    pub http_request_data: Option<HttpRequest>,
    pub peer_connection: Option<Arc<RTCPeerConnection>>,
}

impl WebRTCServerSession {
    pub fn new(
        config: WebRTCServerSessionConfig,
        sessions: SessionsMap,
        auth: Option<Auth>,
        stream: TcpStream,
        socket_addr: SocketAddr,
        packet_sender: PacketDataSender,
        event_sender: EventSender,
    ) -> Self {
        let net_io: Box<dyn TNetIO + Send + Sync> = Box::new(TcpIO::new(stream));
        let io = Arc::new(Mutex::new(net_io));

        Self {
            config,
            io: io.clone(),
            socket_addr,
            reader: BytesReader::new(BytesMut::default()),
            writer: AsyncBytesWriter::new(io),
            packet_sender,
            event_sender,

            auth,
            sessions,
            session_id: None,
            http_request_data: None,
            peer_connection: None,
        }
    }

    pub async fn run(&mut self) -> Result<(), SessionError> {
        while self.reader.len() < 4 {
            let data = self.io.lock().await.read().await?;
            self.reader.extend_from_slice(&data[..]);
        }

        let mut remaining_data = self.reader.get_remaining_bytes();

        if let Some(content_length) =
            common::http::parse_content_length(std::str::from_utf8(&remaining_data)?)
        {
            while remaining_data.len() < content_length as usize {
                log::trace!(
                    "content_length: {} {}",
                    content_length,
                    remaining_data.len()
                );

                let data = self.io.lock().await.read().await?;
                self.reader.extend_from_slice(&data[..]);
                remaining_data = self.reader.get_remaining_bytes();
            }
        }

        let request_data = self.reader.extract_remaining_bytes();

        if let Some(http_request) = HttpRequest::unmarshal(std::str::from_utf8(&request_data)?) {
            let request_method = http_request.method.as_str();
            if request_method == http_method_name::GET {
                let response = match http_request.uri.path.as_str() {
                    "/" => Self::gen_file_response(WEB_WHEP_INDEX.as_bytes(), "text/html"),
                    "/favicon.ico" => Self::gen_file_response(WEB_FAVICON, "mage/x-icon"),
                    "/whep.js" => {
                        Self::gen_file_response(WEB_WHEP_JS.as_bytes(), "application/javascript")
                    }
                    _ => {
                        log::warn!(
                            "the http get path: {} is not supported.",
                            http_request.uri.path
                        );
                        return Ok(());
                    }
                };

                self.send_response(&response).await?;
                return Ok(());
            }

            //POST /whep HTTP/1.1
            let eles: Vec<&str> = http_request.uri.path.splitn(2, '/').collect();
            let pars_map = &http_request.query_pairs;
            let ty = eles[1];

            if eles.len() < 2 {
                log::warn!(
                    "WebRTCServerSession::run the http path is not correct: {}",
                    http_request.uri.path
                );

                return Err(SessionError::HttpRequestPathError);
            }

            match request_method {
                http_method_name::POST => {
                    let Some(sdp_data) = http_request.body.as_ref() else {
                        return Err(SessionError::HttpRequestEmptySdp);
                    };

                    self.session_id = Some(Uuid::new(RandomDigitCount::Zero));
                    let offer = RTCSessionDescription::offer(
                        String::from_utf8(sdp_data.clone()).unwrap_or_default(),
                    )?;

                    let path = format!(
                        "{}?{}session_id={}",
                        http_request.uri.path,
                        if http_request.uri.query.is_some() {
                            format!("{}&", http_request.uri.query.as_ref().unwrap())
                        } else {
                            "".to_string()
                        },
                        self.session_id.unwrap()
                    );

                    if let Some(auth) = &self.auth {
                        let token_carrier = http_request
                            .get_header(&"Authorization".to_string())
                            .map(|header| SecretCarrier::Bearer(header.to_string()))
                            .or_else(|| {
                                http_request
                                    .uri
                                    .query
                                    .as_ref()
                                    .map(|q| SecretCarrier::Query(q.to_string()))
                            });
                        auth.authenticate(&token_carrier)?;
                    }

                    match ty.to_lowercase().as_str() {
                        "whep" => {
                            self.start_streaming(path, offer).await?;
                        }
                        _ => {
                            log::warn!(
                                "current path: {}, method: {}",
                                http_request.uri.path,
                                ty.to_lowercase()
                            );
                            return Err(SessionError::HttpRequestNotSupported);
                        }
                    }
                }
                http_method_name::OPTIONS => {
                    self.send_response(&Self::gen_response(http::StatusCode::OK))
                        .await?
                }
                http_method_name::DELETE => {
                    if let Err(e) = self
                        .event_sender
                        .send(crate::Event::PeerClosed(self.socket_addr.to_string()))
                    {
                        log::warn!(
                            "event_sender send PeerClosed {} failed: {e}",
                            self.socket_addr.to_string()
                        );
                    }

                    if let Some(session_id) = pars_map.get("session_id") {
                        if let Some(uuid) = Uuid::from_str2(session_id) {
                            Self::remove_session(self.sessions.clone(), &uuid).await;
                        }
                    } else {
                        log::warn!(
                            "the delete path does not contain session id: {}?{}",
                            http_request.uri.path,
                            http_request.uri.query.as_ref().unwrap()
                        );
                    }

                    if !matches!(ty.to_lowercase().as_str(), "whep") {
                        log::warn!(
                            "current path: {}, method: {}",
                            http_request.uri.path,
                            ty.to_lowercase()
                        );
                        return Err(SessionError::HttpRequestNotSupported);
                    }

                    let response = Self::gen_response(http::StatusCode::OK);
                    self.send_response(&response).await?;
                }
                http_method_name::PATCH => (),
                _ => {
                    log::warn!(
                        "WebRTCServerSession::unsupported method name: {}",
                        http_request.method
                    );
                }
            }

            self.http_request_data = Some(http_request);
        }

        Ok(())
    }

    async fn remove_session(sessions: SessionsMap, uuid: &Uuid) {
        let mut sessions_unlock = sessions.lock().await;

        if let Some(session) = sessions_unlock.get(uuid) {
            match session.lock().await.close_peer_connection().await {
                Err(e) => log::warn!("close peer connection failed: {e}"),
                _ => log::info!("close peer connection: [{uuid}] successfully."),
            }

            sessions_unlock.remove(uuid);
        } else {
            log::warn!("the session :{uuid}  is not exited.");
        }
    }

    async fn start_streaming(
        &mut self,
        path: String,
        offer: RTCSessionDescription,
    ) -> Result<(), SessionError> {
        if let Some(session_id) = self.session_id.clone() {
            let mut event_receiver = self.event_sender.subscribe();
            let sessions = self.sessions.clone();
            let socket_addr = self.socket_addr.to_string();

            tokio::spawn(async move {
                while let Ok(ev) = event_receiver.recv().await {
                    match ev {
                        crate::Event::LocalClosed(v) | crate::Event::PeerClosed(v)
                            if v == socket_addr =>
                        {
                            Self::remove_session(sessions, &session_id).await;
                            break;
                        }
                        _ => (),
                    }
                }
            });
        }

        let config = Into::<WhepConfig>::into(self.config.clone())
            .with_socket_addr(self.socket_addr.clone());

        let response = match handle_whep(
            config,
            offer,
            self.packet_sender.subscribe(),
            self.event_sender.clone(),
        )
        .await
        {
            Ok((session_description, peer_connection)) => {
                self.peer_connection = Some(peer_connection);

                let mut response = Self::gen_response(http::StatusCode::CREATED);
                response
                    .headers
                    .insert("Content-Type".to_string(), "application/sdp".to_string());
                response.headers.insert("Location".to_string(), path);
                response.body = Some(session_description.sdp.as_bytes().to_vec());
                response
            }
            Err(err) => {
                log::warn!("handle whep err: {}", err);
                let status_code = http::StatusCode::SERVICE_UNAVAILABLE;
                Self::gen_response(status_code)
            }
        };

        self.send_response(&response).await
    }

    fn gen_response(status_code: StatusCode) -> HttpResponse {
        let reason_phrase = if let Some(reason) = status_code.canonical_reason() {
            reason.to_string()
        } else {
            "".to_string()
        };

        let mut response = HttpResponse {
            version: "HTTP/1.1".to_string(),
            status_code: status_code.as_u16(),
            reason_phrase,
            ..Default::default()
        };

        response
            .headers
            .insert("Access-Control-Allow-Origin".to_owned(), "*".to_owned());
        response.headers.insert(
            "Access-Control-Allow-Headers".to_owned(),
            "content-type".to_owned(),
        );
        response
            .headers
            .insert("Access-Control-Allow-Method".to_owned(), "POST".to_owned());
        response
    }

    fn gen_file_response(contents: &[u8], content_type: &str) -> HttpResponse {
        let mut response = Self::gen_response(http::StatusCode::OK);
        response
            .headers
            .insert("Content-Type".to_string(), content_type.to_string());
        response.body = Some(contents.to_vec());
        response
    }

    async fn send_response(&mut self, response: &HttpResponse) -> Result<(), SessionError> {
        self.writer.write(&response.marshal())?;
        self.writer.flush().await?;
        Ok(())
    }

    pub async fn close_peer_connection(&self) -> Result<(), SessionError> {
        if let Some(pc) = &self.peer_connection {
            pc.close().await?;
        }
        Ok(())
    }
}
