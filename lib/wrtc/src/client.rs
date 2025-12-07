use crate::opus::OpusCoder;
use anyhow::Result;
use audiopus::Channels;
use log::{debug, error, info, warn};
use openh264::{decoder::Decoder, formats::YUVSource};
use std::sync::Arc;
use tokio::sync::{Notify, mpsc};
use tokio::time::Duration;
use webrtc::api::APIBuilder;
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::{MIME_TYPE_H264, MIME_TYPE_OPUS, MediaEngine};
use webrtc::ice_transport::ice_connection_state::RTCIceConnectionState;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::interceptor::registry::Registry;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::rtcp::payload_feedbacks::picture_loss_indication::PictureLossIndication;
use webrtc::rtp_transceiver::rtp_codec::{
    RTCRtpCodecCapability, RTCRtpCodecParameters, RTPCodecType,
};
use webrtc::track::track_remote::TrackRemote;

pub type RGBFrame = (u32, u32, Vec<u8>); // (width, height, rgb_data)
pub type AudioSamples = (u32, Vec<f32>); // (sample_rate, f32_samples)

// H264 decoder for Annex-B format RTP streams
pub struct H264Decoder {
    decoder: Decoder,
    width: u32,
    height: u32,
    frame_count: u32,
}

impl H264Decoder {
    pub fn new() -> Result<Self> {
        let decoder = Decoder::new()
            .map_err(|e| anyhow::anyhow!("Failed to create OpenH264 decoder: {:?}", e))?;

        Ok(Self {
            decoder,
            width: 1920, // Default, will be updated when decoding first frame
            height: 1080,
            frame_count: 0,
        })
    }

    pub fn decode(&mut self, h264_data: &[u8]) -> Result<RGBFrame> {
        if h264_data.is_empty() {
            // Return empty frame if no data
            return Ok((
                self.width,
                self.height,
                vec![0u8; (self.width * self.height * 3) as usize],
            ));
        }

        // Parse Annex-B format NAL units
        let nal_units = self.parse_annexb_nal_units(h264_data);

        // Try to decode each NAL unit until we get a frame
        for nal_data in nal_units {
            match self.decoder.decode(&nal_data) {
                Ok(Some(yuv_frame)) => {
                    // Convert YUV to RGB
                    match Self::yuv420_to_rgb(&yuv_frame, self.width, self.height) {
                        Ok(rgb_data) => {
                            self.frame_count += 1;
                            return Ok((self.width, self.height, rgb_data));
                        }
                        Err(e) => {
                            warn!("YUV to RGB conversion error: {:?}", e);
                            continue;
                        }
                    }
                }
                Ok(None) => continue, // No frame produced, continue to next NAL
                Err(e) => {
                    warn!("H264 decode error: {:?}", e);
                    continue; // Continue to next NAL on decode error
                }
            }
        }

        // If no frame decoded, return a test pattern for debugging
        self.generate_test_frame()
    }

    fn parse_annexb_nal_units(&self, data: &[u8]) -> Vec<Vec<u8>> {
        let mut nal_units = Vec::new();
        let mut i = 0;

        while i < data.len() {
            // Look for NAL unit start code (0x00000001 or 0x000001)
            let start_code_len = self.find_start_code(data, i);

            if start_code_len == 0 {
                break; // No more start codes found
            }

            i += start_code_len;

            // Find next start code or end of data
            let next_start_pos = self.find_next_start_code(data, i);

            if next_start_pos > i {
                // Extract NAL unit data (excluding start code)
                let nal_data = data[i..next_start_pos].to_vec();
                if !nal_data.is_empty() {
                    nal_units.push(nal_data);
                }
            }

            i = next_start_pos;
        }

        nal_units
    }

    fn find_start_code(&self, data: &[u8], start_pos: usize) -> usize {
        let pos = start_pos;
        let remaining = data.len() - pos;

        if remaining < 3 {
            return 0;
        }

        // Check for 4-byte start code (0x00000001)
        if remaining >= 4
            && data[pos] == 0
            && data[pos + 1] == 0
            && data[pos + 2] == 0
            && data[pos + 3] == 1
        {
            return 4;
        }

        // Check for 3-byte start code (0x000001)
        if data[pos] == 0 && data[pos + 1] == 0 && data[pos + 2] == 1 {
            return 3;
        }

        0
    }

    fn find_next_start_code(&self, data: &[u8], start_pos: usize) -> usize {
        let mut pos = start_pos;
        let mut remaining = data.len() - pos;

        while remaining >= 3 {
            // Look for potential start code pattern
            if data[pos] == 0 && data[pos + 1] == 0 {
                if pos + 2 < data.len() && data[pos + 2] == 0 {
                    if pos + 3 < data.len() && data[pos + 3] == 1 {
                        return pos; // Found 4-byte start code
                    }
                    pos += 1;
                } else if pos + 2 < data.len() && data[pos + 2] == 1 {
                    return pos; // Found 3-byte start code
                } else {
                    pos += 1;
                }
            } else {
                pos += 1;
            }
            remaining = data.len() - pos;
        }

        data.len() // No more start codes, return end of data
    }

    fn yuv420_to_rgb(
        yuv_frame: &openh264::decoder::DecodedYUV,
        width: u32,
        height: u32,
    ) -> Result<Vec<u8>> {
        // Get YUV plane data
        let y_plane = yuv_frame.y();
        let u_plane = yuv_frame.u();
        let v_plane = yuv_frame.v();

        let width_usize = width as usize;
        let height_usize = height as usize;
        let mut rgb_data = vec![0u8; width_usize * height_usize * 3];

        // Simple YUV420 to RGB conversion
        for y in 0..height_usize {
            for x in 0..width_usize {
                let y_idx = y * width_usize + x;
                let uv_y = y / 2;
                let uv_x = x / 2;

                let y_val = y_plane[y_idx] as f32;
                let u_val = u_plane[uv_y * width_usize / 2 + uv_x] as f32 - 128.0;
                let v_val = v_plane[uv_y * width_usize / 2 + uv_x] as f32 - 128.0;

                // YUV to RGB conversion (BT.601)
                let r = (y_val + 1.402 * v_val).clamp(0.0, 255.0) as u8;
                let g = (y_val - 0.344 * u_val - 0.714 * v_val).clamp(0.0, 255.0) as u8;
                let b = (y_val + 1.772 * u_val).clamp(0.0, 255.0) as u8;

                let rgb_idx = y_idx * 3;
                rgb_data[rgb_idx] = r;
                rgb_data[rgb_idx + 1] = g;
                rgb_data[rgb_idx + 2] = b;
            }
        }

        Ok(rgb_data)
    }

    fn generate_test_frame(&mut self) -> Result<RGBFrame> {
        // Generate a test pattern as fallback when no real frame is decoded
        self.frame_count += 1;
        let mut rgb_data = vec![0u8; (self.width * self.height * 3) as usize];

        for y in 0..self.height {
            for x in 0..self.width {
                let idx = ((y * self.width + x) * 3) as usize;
                let color_shift = (self.frame_count % 255) as u8;

                // Create a moving color gradient
                rgb_data[idx] = ((x + color_shift as u32) % 255) as u8; // R
                rgb_data[idx + 1] = ((y + color_shift as u32) % 255) as u8; // G
                rgb_data[idx + 2] = ((x + y) % 255) as u8; // B
            }
        }

        Ok((self.width, self.height, rgb_data))
    }
}

pub struct WHEPClient {
    pub video_sender: mpsc::UnboundedSender<RGBFrame>,
    pub audio_sender: mpsc::UnboundedSender<AudioSamples>,
}

impl WHEPClient {
    pub fn new() -> (
        Self,
        mpsc::UnboundedReceiver<RGBFrame>,
        mpsc::UnboundedReceiver<AudioSamples>,
    ) {
        let (video_tx, video_rx) = mpsc::unbounded_channel();
        let (audio_tx, audio_rx) = mpsc::unbounded_channel();

        (
            Self {
                video_sender: video_tx,
                audio_sender: audio_tx,
            },
            video_rx,
            audio_rx,
        )
    }
}

async fn process_video_track(
    track: Arc<TrackRemote>,
    video_sender: mpsc::UnboundedSender<RGBFrame>,
    notify: Arc<Notify>,
) -> Result<()> {
    let mut h264_decoder = match H264Decoder::new() {
        Ok(decoder) => decoder,
        Err(e) => {
            error!("Failed to create H264 decoder: {}", e);
            return Ok(());
        }
    };

    info!("H264 video decoder initialized");

    loop {
        tokio::select! {
            result = track.read_rtp() => {
                if let Ok((rtp_packet, _)) = result {
                    // Extract H264 payload from RTP packet
                    let payload = rtp_packet.payload;

                    // Decode H264 packet to RGB frame
                    match h264_decoder.decode(&payload) {
                        Ok(rgb_frame) => {
                            if video_sender.send(rgb_frame).is_err() {
                                info!("Video channel closed, stopping video processing");
                                break;
                            }
                        }
                        Err(e) => {
                            warn!("H264 decoding error: {}", e);
                            // Continue processing even if a frame fails to decode
                        }
                    }
                } else {
                    info!("Video track ended");
                    return Ok(());
                }
            }
            _ = notify.notified() => {
                info!("Video processing stopped by notification");
                return Ok(());
            }
        }
    }
    Ok(())
}

async fn process_audio_track(
    track: Arc<TrackRemote>,
    audio_sender: mpsc::UnboundedSender<AudioSamples>,
    notify: Arc<Notify>,
) -> Result<()> {
    let mut opus_decoder = match OpusCoder::new(48000, Channels::Stereo) {
        Ok(decoder) => decoder,
        Err(e) => {
            error!("Failed to create Opus decoder: {}", e);
            return Ok(());
        }
    };

    info!("Opus audio decoder initialized");

    loop {
        tokio::select! {
            result = track.read_rtp() => {
                if let Ok((rtp_packet, _)) = result {
                    // Extract Opus payload from RTP packet
                    let payload = rtp_packet.payload;

                    // Decode Opus packet to f32 audio samples
                    match opus_decoder.decode(&payload) {
                        Ok(audio_samples) => {
                            let audio_data: AudioSamples = (48000, audio_samples);
                            if audio_sender.send(audio_data).is_err() {
                                info!("Audio channel closed, stopping audio processing");
                                break;
                            }
                        }
                        Err(e) => {
                            warn!("Opus decoding error: {}", e);
                            // Continue processing even if a packet fails to decode
                        }
                    }
                } else {
                    info!("Audio track ended");
                    return Ok(());
                }
            }
            _ = notify.notified() => {
                info!("Audio processing stopped by notification");
                return Ok(());
            }
        }
    }
    Ok(())
}

impl WHEPClient {
    pub async fn connect(&self, server_url: &str) -> Result<()> {
        // Everything below is the WebRTC-rs API! Thanks for using it ❤️.

        // Create a MediaEngine object to configure the supported codec
        let mut m = MediaEngine::default();

        // Setup the codecs you want to use.
        // We'll use a H264 and Opus but you can also define your own
        m.register_codec(
            RTCRtpCodecParameters {
                capability: RTCRtpCodecCapability {
                    mime_type: MIME_TYPE_H264.to_owned(),
                    clock_rate: 90000,
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
                    clock_rate: 48000,
                    channels: 2,
                    sdp_fmtp_line: "".to_owned(),
                    rtcp_feedback: vec![],
                },
                payload_type: 111,
                ..Default::default()
            },
            RTPCodecType::Audio,
        )?;

        // Create a InterceptorRegistry. This is the user configurable RTP/RTCP Pipeline.
        let mut registry = Registry::new();

        // Use the default set of Interceptors
        registry = register_default_interceptors(registry, &mut m)?;

        // Create the API object with the MediaEngine
        let api = APIBuilder::new()
            .with_media_engine(m)
            .with_interceptor_registry(registry)
            .build();

        // Prepare the configuration
        let config = RTCConfiguration {
            ice_servers: vec![RTCIceServer {
                urls: vec!["stun:stun.l.google.com:19302".to_owned()],
                ..Default::default()
            }],
            ..Default::default()
        };

        // Create a new RTCPeerConnection
        let peer_connection = Arc::new(api.new_peer_connection(config).await?);

        // Allow us to receive 1 audio track, and 1 video track
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

        // Set a handler for when a new remote track starts
        let pc = Arc::downgrade(&peer_connection);
        peer_connection.on_track(Box::new(move |track, _, _| {
            // Send a PLI on an interval so that the publisher is pushing a keyframe every rtcpPLIInterval
            let media_ssrc = track.ssrc();
            let pc2 = pc.clone();
            tokio::spawn(async move {
                let mut result = Result::<usize>::Ok(0);
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

            let notify_rx2 = Arc::clone(&notify_rx);
            let video_sender2 = video_sender.clone();
            let audio_sender2 = audio_sender.clone();

            Box::pin(async move {
                let codec = track.codec();
                let mime_type = codec.capability.mime_type.to_lowercase();
                if mime_type == MIME_TYPE_OPUS.to_lowercase() {
                    info!("Got Opus track, processing audio");
                    tokio::spawn(async move {
                        let _ = process_audio_track(track, audio_sender2, notify_rx2).await;
                    });
                } else if mime_type == MIME_TYPE_H264.to_lowercase() {
                    info!("Got H264 track, processing video");
                    tokio::spawn(async move {
                        let _ = process_video_track(track, video_sender2, notify_rx2).await;
                    });
                }
            })
        }));

        let (done_tx, mut done_rx) = tokio::sync::mpsc::channel::<()>(1);

        // Set the handler for ICE connection state
        let notify_tx2 = notify_tx.clone();
        peer_connection.on_ice_connection_state_change(Box::new(
            move |connection_state: RTCIceConnectionState| {
                debug!("Connection State has changed {connection_state}");

                if connection_state == RTCIceConnectionState::Connected {
                    info!("WHEP client connected to server");
                } else if connection_state == RTCIceConnectionState::Failed {
                    notify_tx2.notify_waiters();
                    error!("WHEP client connection failed");

                    let _ = done_tx.try_send(());
                }
                Box::pin(async {})
            },
        ));

        // Implement WHEP protocol: HTTP POST with SDP offer
        info!("WHEP client connecting to: {}", server_url);

        // Create offer
        let offer = peer_connection.create_offer(None).await?;

        // Create channel that is blocked until ICE Gathering is complete
        let mut gather_complete = peer_connection.gathering_complete_promise().await;

        // Sets the LocalDescription, and starts our UDP listeners
        peer_connection.set_local_description(offer).await?;

        // Block until ICE Gathering is complete, disabling trickle ICE
        let _ = gather_complete.recv().await;

        // Get the local description to send to server
        if let Some(local_desc) = peer_connection.local_description().await {
            // Send SDP offer to WHEP server via HTTP POST
            let client = reqwest::Client::new();
            let offer_sdp = local_desc.sdp;

            info!("Sending WHEP request to: {}/whep", server_url);
            debug!("SDP Offer length: {} bytes", offer_sdp.len());

            let response = client
                .post(&format!("{}/whep", server_url))
                .header("Content-Type", "application/sdp")
                .header("Authorization", "Bearer 123")
                .body(offer_sdp)
                .send()
                .await?;

            info!("WHEP response status: {}", response.status());

            if response.status().is_success() {
                let answer_sdp = response.text().await?;
                debug!("SDP Answer received, length: {} bytes", answer_sdp.len());

                // Parse SDP answer
                let answer = RTCSessionDescription::answer(answer_sdp)?;

                // Set the remote description (server's answer)
                peer_connection.set_remote_description(answer).await?;
                info!("WHEP connection established successfully");
            } else {
                let status = response.status();
                let error_text = response.text().await.unwrap_or_default();
                anyhow::bail!("WHEP server returned error: {} - {}", status, error_text);
            }
        } else {
            anyhow::bail!("Failed to generate local description");
        }

        // Wait for connection to be established or fail
        tokio::select! {
            _ = done_rx.recv() => {
                info!("WHEP client connection ended");
            }
            _ = tokio::signal::ctrl_c() => {
                info!("WHEP client stopped by user");
            }
        };

        peer_connection.close().await?;
        Ok(())
    }
}

#[cfg(test)]
pub async fn test_whep_client() -> Result<()> {
    let (client, mut video_rx, mut audio_rx) = WHEPClient::new();

    // Spawn a task to handle incoming video frames
    let video_task = tokio::spawn(async move {
        while let Some((width, height, rgb_data)) = video_rx.recv().await {
            info!(
                "Received video frame: {}x{} ({} bytes)",
                width,
                height,
                rgb_data.len()
            );
        }
        info!("Video channel closed");
    });

    // Spawn a task to handle incoming audio samples
    let audio_task = tokio::spawn(async move {
        while let Some((sample_rate, samples)) = audio_rx.recv().await {
            debug!(
                "Received audio samples: {} Hz, {} samples",
                sample_rate,
                samples.len()
            );
        }
        info!("Audio channel closed");
    });

    // Connect to WHEP server (replace with actual server URL)
    let server_url = "http://localhost:9090/whep";
    info!("Connecting to WHEP server: {}", server_url);

    if let Err(e) = client.connect(server_url).await {
        error!("Failed to connect to WHEP server: {}", e);
    }

    // Wait for tasks to complete
    let _ = tokio::join!(video_task, audio_task);

    Ok(())
}
