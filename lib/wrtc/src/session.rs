use crate::{
    EventSender, PacketDataSender, SessionError,
    common::{
        auth::{Auth, SecretCarrier},
        http::{
            HttpRequest, HttpResponse, Marshal as HttpMarshal, Unmarshal as HttpUnmarshal,
            http_method_name, parse_content_length,
        },
        uuid::{RandomDigitCount, Uuid},
    },
    whep::{WhepConfig, handle_whep},
};
use bytes::BytesMut;
use bytesio::{
    bytes_reader::BytesReader, bytes_writer::AsyncBytesWriter, bytesio::TNetIO, bytesio::TcpIO,
};
use http::StatusCode;
use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use tokio::{net::TcpStream, sync::Mutex};
use webrtc::peer_connection::{RTCPeerConnection, sdp::session_description::RTCSessionDescription};

static WEB_WHEP_INDEX: &str = include_str!("../web-whep-client/index.html");
static WEB_WHEP_JS: &str = include_str!("../web-whep-client/whep.js");

pub struct WebRTCServerSession {
    io: Arc<Mutex<Box<dyn TNetIO + Send + Sync>>>,
    reader: BytesReader,
    writer: AsyncBytesWriter,

    pub session_id: Option<Uuid>,
    pub http_request_data: Option<HttpRequest>,
    pub peer_connection: Option<Arc<RTCPeerConnection>>,

    packet_sender: PacketDataSender,
    event_sender: EventSender,

    auth: Option<Auth>,
    socket_addr: SocketAddr,
}

impl WebRTCServerSession {
    pub fn new(
        stream: TcpStream,
        socket_addr: SocketAddr,
        packet_sender: PacketDataSender,
        event_sender: EventSender,
        auth: Option<Auth>,
    ) -> Self {
        let net_io: Box<dyn TNetIO + Send + Sync> = Box::new(TcpIO::new(stream));
        let io = Arc::new(Mutex::new(net_io));

        Self {
            io: io.clone(),
            socket_addr,
            reader: BytesReader::new(BytesMut::default()),
            writer: AsyncBytesWriter::new(io),

            session_id: None,
            http_request_data: None,
            peer_connection: None,

            packet_sender,
            event_sender,

            auth,
        }
    }

    pub async fn run(
        &mut self,
        uuid_2_sessions: Arc<Mutex<HashMap<Uuid, Arc<Mutex<WebRTCServerSession>>>>>,
    ) -> Result<(), SessionError> {
        while self.reader.len() < 4 {
            let data = self.io.lock().await.read().await?;
            self.reader.extend_from_slice(&data[..]);
        }

        let mut remaining_data = self.reader.get_remaining_bytes();

        if let Some(content_length) = parse_content_length(std::str::from_utf8(&remaining_data)?) {
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
                    "/" => Self::gen_file_response(WEB_WHEP_INDEX),
                    "/whep.js" => Self::gen_file_response(WEB_WHEP_JS),
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

            //POST /whep?app=live&stream=test HTTP/1.1
            let eles: Vec<&str> = http_request.uri.path.splitn(2, '/').collect();
            let pars_map = &http_request.query_pairs;

            if eles.len() < 2 || pars_map.get("app").is_none() || pars_map.get("stream").is_none() {
                log::warn!(
                    "WebRTCServerSession::run the http path is not correct: {}",
                    http_request.uri.path
                );

                return Err(SessionError::HttpRequestPathError);
            }

            let t = eles[1];
            let app_name = pars_map.get("app").unwrap().clone();
            let stream_name = pars_map.get("stream").unwrap().clone();

            log::info!("1:{},2:{},3:{}", t, app_name, stream_name);

            match request_method {
                http_method_name::POST => {
                    let sdp_data = if let Some(body) = http_request.body.as_ref() {
                        body
                    } else {
                        return Err(SessionError::HttpRequestEmptySdp);
                    };

                    let offer = RTCSessionDescription::offer(sdp_data.clone())?;

                    self.session_id = Some(Uuid::new(RandomDigitCount::Zero));

                    let path = format!(
                        "{}?{}&session_id={}",
                        http_request.uri.path,
                        http_request.uri.query.as_ref().unwrap(),
                        self.session_id.unwrap()
                    );

                    let bearer_carrier = http_request
                        .get_header(&"Authorization".to_string())
                        .map(|header| SecretCarrier::Bearer(header.to_string()));

                    let query_carrier = http_request
                        .uri
                        .query
                        .as_ref()
                        .map(|q| SecretCarrier::Query(q.to_string()));

                    let token_carrier = bearer_carrier.or(query_carrier);

                    if let Some(auth) = &self.auth {
                        auth.authenticate(&stream_name, &token_carrier)?;
                    }

                    match t.to_lowercase().as_str() {
                        "whep" => {
                            self.start_streaming(path, offer).await?;
                        }
                        _ => {
                            log::warn!(
                                "current path: {}, method: {}",
                                http_request.uri.path,
                                t.to_lowercase()
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
                            let mut uuid_2_sessions_unlock = uuid_2_sessions.lock().await;

                            if let Some(session) = uuid_2_sessions_unlock.get(&uuid) {
                                match session.lock().await.close_peer_connection().await {
                                    Err(e) => log::warn!("close peer connection failed: {e}"),
                                    _ => log::info!("close peer connection successfully."),
                                }

                                uuid_2_sessions_unlock.remove(&uuid);
                            } else {
                                log::warn!("the session :{}  is not exited.", uuid);
                            }
                        }
                    } else {
                        log::warn!(
                            "the delete path does not contain session id: {}?{}",
                            http_request.uri.path,
                            http_request.uri.query.as_ref().unwrap()
                        );
                    }

                    match t.to_lowercase().as_str() {
                        "whep" => {}
                        _ => {
                            log::warn!(
                                "current path: {}, method: {}",
                                http_request.uri.path,
                                t.to_lowercase()
                            );
                            return Err(SessionError::HttpRequestNotSupported);
                        }
                    }

                    let status_code = http::StatusCode::OK;
                    let response = Self::gen_response(status_code);
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

    async fn start_streaming(
        &mut self,
        path: String,
        offer: RTCSessionDescription,
    ) -> Result<(), SessionError> {
        let config = WhepConfig::new(self.socket_addr.clone());
        let response = match handle_whep(
            config,
            offer,
            self.packet_sender.subscribe(),
            self.event_sender.clone(),
        )
        .await
        {
            Ok((session_description, peer_connection)) => {
                if let Err(e) = self
                    .event_sender
                    .send(crate::Event::PeerConnected(self.socket_addr.to_string()))
                {
                    log::warn!(
                        "event_sender send PeerConnected {} failed: {e}",
                        self.socket_addr.to_string()
                    );
                }

                self.peer_connection = Some(peer_connection);

                let mut response = Self::gen_response(http::StatusCode::CREATED);
                response
                    .headers
                    .insert("Content-Type".to_string(), "application/sdp".to_string());
                response.headers.insert("Location".to_string(), path);
                response.body = Some(session_description.sdp);
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

    fn gen_file_response(contents: &str) -> HttpResponse {
        let mut response = Self::gen_response(http::StatusCode::OK);
        response
            .headers
            .insert("Content-Type".to_string(), "text/html".to_string());
        response.body = Some(contents.to_string());
        response
    }

    async fn send_response(&mut self, response: &HttpResponse) -> Result<(), SessionError> {
        self.writer.write(response.marshal().as_bytes())?;
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
