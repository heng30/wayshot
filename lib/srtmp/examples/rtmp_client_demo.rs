use anyhow::{Result, bail};
use crossbeam::channel::{Sender, bounded};
use image::{ImageBuffer, Rgb};
use spin_sleep::SpinSleeper;
use srtmp::{AacEncoderConfig, AudioData, RtmpClient, RtmpClientConfig, VideoData};
use std::{
    sync::{Arc, atomic::AtomicBool},
    thread,
    time::{Duration, Instant},
};

const IMG_WIDTH: u32 = 1920;
const IMG_HEIGHT: u32 = 1080;
const AUDIO_SAMPLE_RATE: u32 = 44100;
const AUDIO_CHANNELS: u8 = 2;

// Usage: rtmp-client-demo [rtmp_url] [app_name] [stream_key] [query_params]
// Or: rtmp-client-demo [full_url]
//
// Examples:
//   rtmp-client-demo rtmp://localhost:1935 live stream "key=value&token=abc"
//   rtmp-client-demo "rtmp://localhost:1935/live/stream?key=value&token=abc"
fn main() -> Result<()> {
    env_logger::init();

    let args: Vec<String> = std::env::args().collect();

    let config = if args.len() > 1 && args[1].contains("rtmp://") {
        if args.len() == 2 {
            log::info!("Parsing RTMP URL...");
            RtmpClientConfig::from_url(&args[1])?
        } else {
            let rtmp_url = args
                .get(1)
                .cloned()
                .unwrap_or_else(|| "rtmp://localhost:1935".to_string());
            let app_name = args.get(2).cloned().unwrap_or_else(|| "live".to_string());
            let stream_key = args.get(3).cloned().unwrap_or_else(|| "stream".to_string());
            let query_params = args.get(4).cloned().unwrap_or_default();

            let mut config =
                RtmpClientConfig::new(rtmp_url.clone(), app_name.clone(), stream_key.clone());
            if !query_params.is_empty() {
                config = config.with_query_params(query_params.clone());
            }
            config
        }
    } else {
        RtmpClientConfig::new(
            "rtmp://localhost:1935".to_string(),
            "live".to_string(),
            "stream".to_string(),
        )
    };

    log::info!("RTMP Client Demo with AAC Encoding");
    log::info!("===================================");
    log::info!("RTMP URL: {}", config.rtmp_url);
    log::info!("App name: {}", config.app);
    log::info!("Stream Key: {}", config.stream_key);
    if !config.query_params.is_empty() {
        log::info!("Query Params: {}", config.query_params);
    }

    let exit_sig = Arc::new(AtomicBool::new(false));
    let (video_tx, video_rx) = bounded(16);
    let (audio_tx, audio_rx) = bounded(32);

    // let aac_config = None;
    let aac_config = Some(
        AacEncoderConfig::default()
            .with_sample_rate(AUDIO_SAMPLE_RATE)
            .with_channels(2),
    );

    let mut client = RtmpClient::new(config, aac_config, video_rx, audio_rx, exit_sig.clone())?;

    let exit_sig_video = exit_sig.clone();
    let video_handle = thread::spawn(move || {
        spawn_video_generator(video_tx, exit_sig_video).unwrap();
    });

    let exit_sig_audio = exit_sig.clone();
    let audio_handle = thread::spawn(move || {
        spawn_pcm_audio_generator(audio_tx, exit_sig_audio);
    });

    let exit_sig_clone = exit_sig.clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_secs(10 * 3600));
        log::info!("Stopping stream after 10 seconds...");
        exit_sig_clone.store(true, std::sync::atomic::Ordering::Relaxed);
    });

    let start = Instant::now();

    match client.start() {
        Ok(_) => log::info!("Streaming completed successfully"),
        Err(e) => log::error!("Streaming error: {}", e),
    }

    let elapsed = start.elapsed();
    log::info!("Streaming completed in {:.2}s", elapsed.as_secs_f64());

    client.stop();
    _ = video_handle.join();
    _ = audio_handle.join();

    log::info!("Demo finished");
    Ok(())
}

fn spawn_video_generator(
    packet_sender: Sender<VideoData>,
    exit_sig: Arc<AtomicBool>,
) -> Result<()> {
    let fps = 25;
    let mut next_frame_timestamp = 0;
    let frame_interval = 1000 / fps as u32;
    let sleeper = SpinSleeper::default();
    let start_time = Instant::now();

    loop {
        if exit_sig.load(std::sync::atomic::Ordering::Relaxed) {
            log::info!("video thread received `exit signal`");
            break;
        }

        let config = video_encoder::VideoEncoderConfig::new(IMG_WIDTH, IMG_HEIGHT)
            .with_fps(fps)
            .with_annexb(true); // Use annexb format (start codes)
        let mut h264_encoder = video_encoder::new(config).unwrap();
        let headers_data = h264_encoder.headers().unwrap();

        log::info!(
            "H.264 headers (annexb) data: {} bytes, first 40 bytes: {:02x?}",
            headers_data.len(),
            &headers_data[..headers_data.len().min(40)]
        );

        // must be sent
        let packet = VideoData::new_with_sequence_header(headers_data)?;
        if let Err(e) = packet_sender.send(packet) {
            bail!("send h264 sequence header failed: {}", e.to_string());
        }

        for frame_num in 0.. {
            if exit_sig.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }

            let c = ((frame_num % fps) * (255 / fps)) as u8;
            let img = match (frame_num / fps) % 3 {
                0 => &create_color_frame(c, 0, 0),
                1 => &create_color_frame(0, c, 0),
                2 => &create_color_frame(0, 0, c),
                _ => &create_color_frame(c, c, c),
            };
            let encoded_frame = h264_encoder.encode_frame(img.clone()).unwrap();

            match encoded_frame {
                video_encoder::EncodedFrame::Frame((_, data)) => {
                    if let Err(e) = packet_sender.send(VideoData::new(next_frame_timestamp, data)) {
                        log::warn!("send h264 nal data failed: {e}");
                    }
                }
                _ => break,
            }

            next_frame_timestamp = frame_interval * (frame_num + 1);

            let sleep_time = start_time + Duration::from_millis(next_frame_timestamp as u64);
            if sleep_time > Instant::now() {
                sleeper.sleep_until(sleep_time);
            }
        }

        let remained_datas = Arc::new(std::sync::Mutex::new(vec![]));
        let remained_datas_copy = remained_datas.clone();

        if let Err(e) = h264_encoder.flush(Box::new(move |data| {
            remained_datas_copy.lock().unwrap().push(data);
        })) {
            log::warn!("Failed to flush encoder frame: {:?}", e);
        }

        let datas_to_send = remained_datas.lock().unwrap().clone();
        for data in datas_to_send {
            if let Err(e) = packet_sender.send(VideoData::new(next_frame_timestamp, data)) {
                log::warn!("send h264 nal data failed: {e}");
            };
        }
    }

    log::info!("h264_streaming_thread exit...");
    Ok(())
}

fn spawn_pcm_audio_generator(packet_sender: Sender<AudioData>, exit_sig: Arc<AtomicBool>) {
    let sample_rate = AUDIO_SAMPLE_RATE;
    let channels = AUDIO_CHANNELS;
    let frame_samples = 1024usize; // AAC frame size (samples per channel)
    let total_samples = frame_samples * channels as usize;
    let frame_duration_ms = (frame_samples * 1000 / sample_rate as usize) as u64;

    let mut frame_count = 0u32;
    let mut next_frame_timestamp = 0;
    let sleeper = SpinSleeper::default();
    let start_time = Instant::now();

    loop {
        if exit_sig.load(std::sync::atomic::Ordering::Relaxed) {
            log::info!("audio thread received `exit signal`");
            break;
        }

        // Generate a simple test tone (sine wave at 440Hz = A4 note)
        let frequency = 440.0;
        let mut pcm_data = Vec::with_capacity(total_samples);

        for i in 0..frame_samples {
            let t = (frame_count as usize * frame_samples + i) as f32 / sample_rate as f32;
            let sample_value = (2.0 * std::f32::consts::PI * frequency * t).sin() * 0.05; // 5% volume

            // Stereo: same value for both channels
            pcm_data.push(sample_value);
            pcm_data.push(sample_value);
        }

        if let Err(e) = packet_sender.send(AudioData {
            timestamp: next_frame_timestamp,
            data: pcm_data,
        }) {
            log::warn!("send pcm audio data failed: {e}");
        }

        frame_count += 1;
        next_frame_timestamp += frame_duration_ms as u32;

        let sleep_time = start_time + Duration::from_millis(next_frame_timestamp as u64);
        if sleep_time > Instant::now() {
            sleeper.sleep_until(sleep_time);
        }
    }

    log::info!("pcm_audio_streaming_thread exit...");
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
