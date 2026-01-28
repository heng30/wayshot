use std::path::PathBuf;
use std::time::Duration;
use video_utils::{
    MP4Encoder, MP4EncoderConfig, H264Config, EncoderAACConfig as AACConfig,
    H264Preset, EncoderFrameData as FrameData,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("╔════════════════════════════════════════════════════════════════╗");
    println!("║              Simple MP4 Encoder Test                             ║");
    println!("╚════════════════════════════════════════════════════════════════╝");
    println!();

    let config = MP4EncoderConfig {
        output_path: PathBuf::from("tmp/test_output.mp4"),
        frame_rate: 30,
        h264: H264Config {
            bitrate: 2_000_000,
            preset: H264Preset::Medium,
            crf: Some(23),
        },
        aac: AACConfig {
            bitrate: 128_000,
            sample_rate: 44_100,
            channels: 2,
        },
    };

    println!("Starting encoder...");
    let (encoder, video_tx, _audio_tx) = MP4Encoder::start(config)?;

    println!("Sending frame 0...");
    let frame = create_test_frame(1280, 720, 0);
    video_tx.send(frame)?;

    println!("Frame 0 sent successfully");

    println!("Sending frame 1...");
    let frame = create_test_frame(1280, 720, 1);
    video_tx.send(frame)?;

    println!("Frame 1 sent successfully");

    println!("Sending frame 2...");
    let frame = create_test_frame(1280, 720, 2);
    video_tx.send(frame)?;

    println!("Frame 2 sent successfully");

    // Drop senders explicitly to close the channels before calling stop()
    drop(video_tx);
    drop(_audio_tx);

    println!("Channels closed, stopping encoder...");
    encoder.stop()?;

    println!("✓ Test completed successfully");

    Ok(())
}

fn create_test_frame(width: u32, height: u32, frame_index: usize) -> FrameData {
    let mut frame_data = vec![0u8; width as usize * height as usize * 3];

    for y in 0..height {
        for x in 0..width {
            let idx = ((y * width + x) * 3) as usize;

            let r = ((x as f32 / width as f32) * 255.0) as u8;
            let g = ((y as f32 / height as f32) * 255.0) as u8;
            let b = ((frame_index as f32 / 10.0) * 255.0) as u8;

            frame_data[idx] = r;
            frame_data[idx + 1] = g;
            frame_data[idx + 2] = b;
        }
    }

    FrameData {
        width,
        height,
        data: frame_data,
        timestamp: Duration::from_millis((frame_index as u64 * 1000) / 30),
    }
}
