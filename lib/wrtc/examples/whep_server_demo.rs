use anyhow::{Result, bail};
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
use webrtc::media::io::h264_reader::H264Reader;
use webrtc::media::io::ogg_reader::OggReader;
use wrtc::{
    Event, OPUS_SAMPLE_RATE, PacketData, session::WebRTCServerSessionConfig, webrtc::WebRTCServer,
};

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let video_path = "./data/test.h264".to_string();
    let audio_path = "./data/test.ogg".to_string();
    let config = WebRTCServerSessionConfig::default();
    let (packet_sender, _) = broadcast::channel(128);
    let (event_sender, mut event_receiver) = broadcast::channel(16);

    if !Path::new(&video_path).exists() {
        bail!("video file: '{video_path}' not exist");
    }

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
                                h264_streaming_thread(packet_sender_clone.clone(), video_path.clone(), connections.clone());
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

fn h264_streaming_thread(
    packet_sender: Sender<PacketData>,
    video_file: String,
    connections: Arc<AtomicI32>,
) {
    tokio::spawn(async move {
        'out: loop {
            let file = File::open(&video_file.clone()).unwrap();
            let reader = BufReader::new(file);
            let mut h264 = H264Reader::new(reader, 1_048_576);

            let mut ticker = tokio::time::interval(Duration::from_millis(40)); // 25fps
            loop {
                let nal = match h264.next_nal() {
                    Ok(nal) => nal,
                    Err(_) => break,
                };

                log::trace!(
                    "PictureOrderCount={}, ForbiddenZeroBit={}, RefIdc={}, UnitType={}, data={}",
                    nal.picture_order_count,
                    nal.forbidden_zero_bit,
                    nal.ref_idc,
                    nal.unit_type,
                    nal.data.len()
                );

                if let Err(e) = packet_sender.send(PacketData::Video {
                    timestamp: Instant::now(),
                    data: nal.data.freeze().into(),
                }) {
                    log::warn!("send h264 nal data failed: {e}");
                };

                _ = ticker.tick().await;

                if connections.load(Ordering::Relaxed) <= 0 {
                    break 'out;
                }
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
