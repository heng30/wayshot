use recorder::{AudioRecorder, StreamingAudioRecorder};
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Input Audio Recording Demo");
    println!("==========================");

    let recorder = AudioRecorder::new(None)?.with_real_time_denoise(true);

    println!("\nAvailable Input Devices:");
    println!("------------------------");

    let input_devices = recorder.get_input_devices()?;
    for (i, device) in input_devices.iter().enumerate() {
        println!("{}. {}", i + 1, device.name);
    }

    if let Some(default_input) = recorder.get_default_input_device()? {
        println!("\nDefault Input Device: {}", default_input.name);
    }

    println!("\nStarting input audio recording to file...");

    let input_filename = "target/input_recording.wav";

    println!(
        "Recording from default input device to '{}'...",
        input_filename
    );

    let streaming_recorder = StreamingAudioRecorder::start(
        recorder,
        "default", // 使用默认输入设备
        input_filename,
        false,
    )?;

    println!("Recording started. Speak into your microphone for 5 seconds...");

    // 录制5秒钟
    for i in (1..=5).rev() {
        println!("Recording... {} seconds remaining", i);
        std::thread::sleep(Duration::from_secs(1));
    }

    println!("Stopping recording...");
    streaming_recorder.stop()?;

    println!("Recording saved to '{}'", input_filename);
    println!("Done!");

    Ok(())
}
