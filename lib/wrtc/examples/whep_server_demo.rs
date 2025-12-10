use anyhow::{Result, bail};
use once_cell::sync::Lazy;
use std::{
    collections::HashSet,
    fs::File,
    io::BufReader,
    path::Path,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::sync::{
    Notify,
    broadcast::{self, Sender},
};
use webrtc::media::io::{h264_reader::H264Reader, ogg_reader::OggReader};
use wrtc::{
    Event, PacketData,
    opus::OPUS_SAMPLE_RATE,
    session::{MediaInfo, WebRTCServerSessionConfig},
    webrtc::WebRTCServer,
};

static CONNECTIONS: Lazy<Mutex<HashSet<String>>> = Lazy::new(|| Mutex::new(HashSet::default()));

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let video_path = "./data/test.h264".to_string();
    let audio_path = "./data/test.ogg".to_string();
    let config = WebRTCServerSessionConfig::default().with_media_info(MediaInfo::default());
    let (packet_sender, _) = broadcast::channel(128);
    let (event_sender, mut event_receiver) = broadcast::channel(16);
    let exit_notify = Arc::new(Notify::new());

    if !Path::new(&video_path).exists() {
        bail!("video file: '{video_path}' not exist");
    }

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
                                h264_streaming_thread(packet_sender_clone.clone(), video_path.clone());
                                ogg_stream_thread(packet_sender_clone.clone(), audio_path.clone());
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
        "0.0.0.0:9090".to_string(),
        None,
        packet_sender,
        event_sender,
        exit_notify,
    );

    server.run().await?;

    Ok(())
}

fn h264_streaming_thread(packet_sender: Sender<PacketData>, video_file: String) {
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

                if CONNECTIONS.lock().unwrap().is_empty() {
                    break 'out;
                }
            }

            if CONNECTIONS.lock().unwrap().is_empty() {
                break;
            }
        }

        log::info!("h264_streaming_thread exit...");
    });
}

fn ogg_stream_thread(packet_sender: Sender<PacketData>, audio_file: String) {
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

                if CONNECTIONS.lock().unwrap().is_empty() {
                    break 'out;
                }
            }

            if CONNECTIONS.lock().unwrap().is_empty() {
                break;
            }
        }
    });
}
