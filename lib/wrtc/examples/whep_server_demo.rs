use anyhow::{Result, bail};
use std::{
    fs::File,
    io::BufReader,
    path::Path,
    time::{Duration, Instant},
};
use tokio::sync::broadcast::{self, Sender};
use webrtc::media::io::h264_reader::H264Reader;
use wrtc::{Event, PacketData, session::WebRTCServerSessionConfig, webrtc::WebRTCServer};

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let video_path = "./data/test.h264".to_string();
    let config = WebRTCServerSessionConfig::default();
    let (packet_sender, _) = broadcast::channel(128);
    let (event_sender, mut event_receiver) = broadcast::channel(16);

    if !Path::new(&video_path).exists() {
        bail!("video file: '{video_path}' not exist");
    }

    let packet_sender_clone = packet_sender.clone();
    tokio::spawn(async move {
        let mut connections = 0;
        loop {
            tokio::select! {
                ev = event_receiver.recv() => {
                    match ev {
                        Ok(Event::PeerConnected(_)) => {
                            if connections == 0 {
                                h264_streaming_thread(packet_sender_clone.clone(), video_path.clone());
                            }

                            connections += 1;
                            log::info!("connections count: {connections}");
                        }
                        Ok(Event::LocalClosed(addr)) => {
                            if connections > 0 {
                                connections  -= 1;
                            }
                            log::info!("LocalClosed({addr})");
                            log::info!("connections count: {connections}");
                        }
                        Ok(Event::PeerClosed(addr)) => {
                            if connections > 0 {
                                connections  -= 1;
                            }
                            log::info!("PeerClosed({addr})");
                            log::info!("connections count: {connections}");
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

fn h264_streaming_thread(packet_sender: Sender<PacketData>, video_file: String) {
    tokio::spawn(async move {
        loop {
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
            }
        }
    });
}
