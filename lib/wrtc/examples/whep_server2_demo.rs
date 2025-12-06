use anyhow::{Result, bail};
use image::{ImageBuffer, Rgb};
use std::{
    fs::File,
    io::BufReader,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicI32, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::sync::broadcast::{self, Sender};
use video_encoder::{EncodedFrame, VideoEncoderConfig};
use webrtc::media::io::ogg_reader::OggReader;
use wrtc::{
    Event, OPUS_SAMPLE_RATE, PacketData, session::WebRTCServerSessionConfig, webrtc::WebRTCServer,
};

const IMG_WIDTH: u32 = 1920;
const IMG_HEIGHT: u32 = 1080;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let audio_path = "./data/test.ogg".to_string();
    let config = WebRTCServerSessionConfig::default();
    let (packet_sender, _) = broadcast::channel(128);
    let (event_sender, mut event_receiver) = broadcast::channel(16);

    if !Path::new(&audio_path).exists() {
        bail!("audio file: '{audio_path}' not exist");
    }

    let packet_sender_clone = packet_sender.clone();
    tokio::spawn(async move {
        let connections = Arc::new(AtomicI32::new(0));
        loop {
            tokio::select! {
                ev = event_receiver.recv() => {
                    match ev {
                        Ok(Event::PeerConnected(_)) => {
                            if connections.load(Ordering::Relaxed) == 0 {
                                h264_streaming_thread(packet_sender_clone.clone(), connections.clone());
                                ogg_stream_thread(packet_sender_clone.clone(), audio_path.clone(), connections.clone());
                            }

                            let count = connections.fetch_add(1, Ordering::Relaxed);
                            log::info!("connections count: {}", count + 1);
                        }
                        Ok(Event::LocalClosed(addr)) => {
                            if connections.fetch_sub(1, Ordering::Relaxed) == 0 {
                                connections.store(0, Ordering::Relaxed);
                            }

                            log::info!("LocalClosed({addr})");
                            log::info!("connections count: {}", connections.load(Ordering::Relaxed));
                        }
                        Ok(Event::PeerClosed(addr)) => {
                            if connections.fetch_sub(1, Ordering::Relaxed) == 0 {
                                connections.store(0, Ordering::Relaxed);
                            }

                            log::info!("PeerClosed({addr})");
                            log::info!("connections count: {}", connections.load(Ordering::Relaxed));
                        }
                        _ => (),
                    }
                }
                _ = tokio::signal::ctrl_c() => {
                    log::info!("receive ctrl-c, exit...");
                    std::process::exit(0);
                }
            }
        }
    });

    let mut server = WebRTCServer::new(
        config,
        "0.0.0.0:9090".to_string(),
        None,
        packet_sender,
        event_sender,
    );

    server.run().await?;

    Ok(())
}

fn h264_streaming_thread(packet_sender: Sender<PacketData>, connections: Arc<AtomicI32>) {
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

                if connections.load(Ordering::Relaxed) <= 0 {
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

            if connections.load(Ordering::Relaxed) <= 0 {
                break;
            }
        }

        log::info!("h264_streaming_thread exit...");
    });
}

fn ogg_stream_thread(
    packet_sender: Sender<PacketData>,
    audio_file: String,
    connections: Arc<AtomicI32>,
) {
    tokio::spawn(async move {
        'out: loop {
            let file = File::open(&audio_file).unwrap();
            let reader = BufReader::new(file);
            let (mut ogg, _) = OggReader::new(reader, true).unwrap();
            let page_duration = Duration::from_millis(20);

            let mut last_granule = 0;
            let mut ticker = tokio::time::interval(page_duration);

            while let Ok((page_data, page_header)) = ogg.parse_next_page() {
                let sample_count = page_header.granule_position - last_granule;
                last_granule = page_header.granule_position;
                let sample_duration = Duration::from_millis(sample_count * 1000 / OPUS_SAMPLE_RATE);

                if let Err(e) = packet_sender.send(PacketData::Audio {
                    timestamp: Instant::now(),
                    duration: sample_duration,
                    data: page_data.freeze().into(),
                }) {
                    log::warn!("send audio data failed: {e}");
                }

                _ = ticker.tick().await;

                if connections.load(Ordering::Relaxed) <= 0 {
                    break 'out;
                }
            }

            if connections.load(Ordering::Relaxed) <= 0 {
                break;
            }
        }
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
