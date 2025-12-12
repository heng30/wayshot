use anyhow::{Result, bail};
use hound::WavReader;
use image::{ImageBuffer, Rgb};
use once_cell::sync::Lazy;
use rustls::crypto::{CryptoProvider, ring};
use std::{
    collections::HashSet,
    fs::File,
    path::Path,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::sync::{
    Notify,
    broadcast::{self, Sender},
};
use video_encoder::{EncodedFrame, VideoEncoderConfig};
use wrtc::{
    Event, PacketData, WebRTCServer, WebRTCServerConfig,
    opus::OpusCoder,
    session::{MediaInfo, VideoInfo, WebRTCServerSessionConfig},
};

// const IMG_WIDTH: u32 = 1920;
// const IMG_HEIGHT: u32 = 1080;

const IMG_WIDTH: u32 = 3072;
const IMG_HEIGHT: u32 = 1920;

static CONNECTIONS: Lazy<Mutex<HashSet<String>>> = Lazy::new(|| Mutex::new(HashSet::default()));

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    CryptoProvider::install_default(ring::default_provider().into())
        .expect("failed to set crypto provider");

    let audio_path = "./data/test-44100.wav".to_string();
    let medio_info = MediaInfo::default().with_video(
        VideoInfo::default()
            .with_width(IMG_WIDTH as i32)
            .with_height(IMG_HEIGHT as i32),
    );
    let config = WebRTCServerConfig::new("0.0.0.0:9090".to_string(), Some("123".to_string()));
    let session_config = WebRTCServerSessionConfig::default().with_media_info(medio_info);
    let (packet_sender, _) = broadcast::channel(128);
    let (event_sender, mut event_receiver) = broadcast::channel(16);
    let exit_notify = Arc::new(Notify::new());

    if !Path::new(&audio_path).exists() {
        bail!("audio file: '{audio_path}' not exist");
    }

    let exit_notify_clone = exit_notify.clone();
    let packet_sender_clone = packet_sender.clone();
    tokio::spawn(async move {
        loop {
            tokio::select! {
                ev = event_receiver.recv() => {
                    match ev {
                        Ok(Event::PeerConnected(addr)) => {
                            let mut connections = CONNECTIONS.lock().unwrap();
                            if connections.is_empty(){
                                h264_streaming_thread(packet_sender_clone.clone());
                                wav_stream_thread(packet_sender_clone.clone(), audio_path.clone());
                            }

                            connections.insert(addr);
                            log::info!("connections count: {}", connections.len());
                        }
                        Ok(Event::LocalClosed(addr)) => {
                            log::info!("LocalClosed({addr})");

                            let mut connections = CONNECTIONS.lock().unwrap();
                            connections.remove(&addr);
                            log::info!("connections count: {}", connections.len());
                        }
                        Ok(Event::PeerClosed(addr)) => {
                            log::info!("PeerClosed({addr})");
                            let mut connections = CONNECTIONS.lock().unwrap();
                            connections.remove(&addr);
                            log::info!("connections count: {}", connections.len());
                        }
                        _ => (),
                    }
                }
                _ = tokio::signal::ctrl_c() => {
                    log::info!("receive ctrl-c, exit...");
                    exit_notify_clone.notify_waiters();
                }
            }
        }
    });

    let mut server = WebRTCServer::new(
        config,
        session_config,
        packet_sender,
        event_sender,
        exit_notify,
    );

    server.run().await?;

    Ok(())
}

fn h264_streaming_thread(packet_sender: Sender<PacketData>) {
    std::thread::spawn(move || {
        let fps = 25;

        'out: loop {
            let config = VideoEncoderConfig::new(IMG_WIDTH, IMG_HEIGHT)
                .with_fps(fps)
                .with_annexb(true);

            let mut h264_encoder = video_encoder::new(config).unwrap();
            let headers_data = h264_encoder.headers().unwrap();
            let packet_sender = packet_sender.clone();

            if let Err(e) = packet_sender.send(PacketData::Video {
                timestamp: Instant::now(),
                data: headers_data.into(),
            }) {
                log::warn!("send h264 nal data failed: {e}");
            };

            for frame_num in 0.. {
                let c = ((frame_num % fps) * (255 / fps)) as u8;
                let img = match (frame_num / fps) % 3 {
                    0 => &create_color_frame(c, 0, 0),
                    1 => &create_color_frame(0, c, 0),
                    2 => &create_color_frame(0, 0, c),
                    _ => &create_color_frame(c, c, c),
                };
                let encoded_frame = h264_encoder.encode_frame(img.clone()).unwrap();

                match encoded_frame {
                    EncodedFrame::Frame((_, data)) => {
                        if let Err(e) = packet_sender.send(PacketData::Video {
                            timestamp: Instant::now(),
                            data: data.into(),
                        }) {
                            log::warn!("send h264 nal data failed: {e}");
                        };
                    }
                    _ => break,
                }

                std::thread::sleep(Duration::from_secs_f64(1 as f64 / fps as f64));

                if CONNECTIONS.lock().unwrap().is_empty() {
                    break 'out;
                }
            }

            if let Err(e) = h264_encoder.flush(Box::new(move |data| {
                if let Err(e) = packet_sender.send(PacketData::Video {
                    timestamp: Instant::now(),
                    data: data.into(),
                }) {
                    log::warn!("send h264 nal data failed: {e}");
                };
            })) {
                log::warn!("Failed to flush encoder frame: {:?}", e);
            }

            if CONNECTIONS.lock().unwrap().is_empty() {
                break;
            }
        }

        log::info!("h264_streaming_thread exit...");
    });
}

fn wav_stream_thread(packet_sender: Sender<PacketData>, audio_file: String) {
    tokio::spawn(async move {
        'out: loop {
            let file = File::open(&audio_file).unwrap();
            let mut reader = WavReader::new(file).unwrap();
            let spec = reader.spec();

            let channels = if spec.channels == 1 {
                audiopus::Channels::Mono
            } else if spec.channels == 2 {
                audiopus::Channels::Stereo
            } else {
                log::error!(
                    "Only mono and stereo audio are supported, got {} channels",
                    spec.channels
                );
                break;
            };

            let mut opus_coder = OpusCoder::new(spec.sample_rate, channels)
                .expect("Failed to initialize Opus coder");

            let samples: Vec<f32> = reader
                .samples::<i16>()
                .map(|s| match s {
                    Ok(sample) => sample as f32 / 32768.0,
                    Err(e) => {
                        log::warn!("Failed to read sample: {}", e);
                        0.0
                    }
                })
                .collect();

            log::trace!(
                "Loaded WAV: {}Hz, {} channels, {} samples, {:.2}s",
                spec.sample_rate,
                spec.channels,
                samples.len(),
                samples.len() as f32 / (spec.sample_rate as f32 * spec.channels as f32)
            );

            let samples_per_frame = opus_coder.input_samples_per_frame();
            let frame_duration_ms = 20;
            let frame_duration = Duration::from_millis(frame_duration_ms);
            let mut ticker = tokio::time::interval(frame_duration);

            for (frame_idx, chunk) in samples.chunks(samples_per_frame).enumerate() {
                let mut frame = vec![0.0f32; samples_per_frame];
                frame[..chunk.len()].copy_from_slice(chunk);

                match opus_coder.encode(&frame) {
                    Ok(opus_data) => {
                        if let Err(e) = packet_sender.send(PacketData::Audio {
                            timestamp: Instant::now(),
                            duration: frame_duration,
                            data: opus_data.into(),
                        }) {
                            log::warn!("send audio data failed: {e}");
                        }
                    }
                    Err(e) => {
                        log::warn!("Encoding frame {} failed: {}", frame_idx + 1, e);
                        if let Err(e) = packet_sender.send(PacketData::Audio {
                            timestamp: Instant::now(),
                            duration: frame_duration,
                            data: vec![].into(),
                        }) {
                            log::warn!("send empty audio data failed: {e}");
                        }
                    }
                }

                ticker.tick().await;

                if CONNECTIONS.lock().unwrap().is_empty() {
                    break 'out;
                }
            }

            if CONNECTIONS.lock().unwrap().is_empty() {
                break;
            }
        }

        log::info!("wav_streaming_thread exit...");
    });
}

fn create_color_frame(r: u8, g: u8, b: u8) -> ImageBuffer<Rgb<u8>, Vec<u8>> {
    let mut frame: Vec<u8> = Vec::with_capacity((IMG_WIDTH * IMG_HEIGHT * 3) as usize);

    for _ in 0..(IMG_WIDTH * IMG_HEIGHT) {
        frame.push(r);
        frame.push(g);
        frame.push(b);
    }
    ImageBuffer::from_vec(IMG_WIDTH, IMG_HEIGHT, frame).unwrap()
}
