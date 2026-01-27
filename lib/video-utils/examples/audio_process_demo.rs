use std::path::PathBuf;
use std::process::Command;
use video_utils::audio_process::{AudioProcessConfig, LoudnormConfig, process_audio};

fn main() {
    env_logger::init();

    // Get the data and tmp directory paths
    let data_dir = PathBuf::from("data");
    let tmp_dir = PathBuf::from("tmp");

    // Create tmp directory if it doesn't exist
    std::fs::create_dir_all(&tmp_dir).expect("Failed to create tmp directory");

    let test_video = data_dir.join("test.mp4");

    // Check if test video exists
    if !test_video.exists() {
        eprintln!("Test video not found: {:?}", test_video);
        eprintln!("Please place a test video file at data/test.mp4");
        std::process::exit(1);
    }

    println!("Audio processing example");
    println!("=======================");
    println!("Test video: {:?}", test_video);
    println!();

    // Test 1: With volume adjustment
    let output1 = test_with_volume(&test_video, &tmp_dir);
    verify_with_ffprobe(&output1);

    // Test 2: Without volume adjustment (loudness normalization only)
    let output2 = test_without_volume(&test_video, &tmp_dir);
    verify_with_ffprobe(&output2);

    // Test 3: Custom loudness settings (louder)
    let output3 = test_custom_loudness(&test_video, &tmp_dir);
    verify_with_ffprobe(&output3);

    // Test 4: Higher audio bitrate
    let output4 = test_high_bitrate(&test_video, &tmp_dir);
    verify_with_ffprobe(&output4);
}

/// Test 1: With volume adjustment (1.3x boost)
fn test_with_volume(video: &PathBuf, output_dir: &PathBuf) -> PathBuf {
    println!("\n=== Test 1: With Volume Adjustment (1.3x) ===");

    let output = output_dir.join("output_audio_volume.mp4");

    let config = AudioProcessConfig::new()
        .with_input(video.to_str().unwrap().to_string())
        .with_output(output.to_str().unwrap().to_string())
        .with_volume(Some(1.3));

    match process_audio(&config) {
        Ok(_) => println!("✓ Test 1 output saved to: {:?}", output),
        Err(e) => eprintln!("✗ Test 1 failed: {}", e),
    }

    output
}

/// Test 2: Without volume adjustment (loudness normalization only)
fn test_without_volume(video: &PathBuf, output_dir: &PathBuf) -> PathBuf {
    println!("\n=== Test 2: Loudness Normalization Only ===");

    let output = output_dir.join("output_audio_norm_only.mp4");

    let config = AudioProcessConfig::new()
        .with_input(video.to_str().unwrap().to_string())
        .with_output(output.to_str().unwrap().to_string())
        .with_volume(None);

    match process_audio(&config) {
        Ok(_) => println!("✓ Test 2 output saved to: {:?}", output),
        Err(e) => eprintln!("✗ Test 2 failed: {}", e),
    }

    output
}

/// Test 3: Custom loudness settings (louder target)
fn test_custom_loudness(video: &PathBuf, output_dir: &PathBuf) -> PathBuf {
    println!("\n=== Test 3: Custom Loudness (I=-14 LUFS) ===");

    let output = output_dir.join("output_audio_custom.mp4");

    let loudnorm = LoudnormConfig::new().with_target_i(-14.0); // Louder than default

    let config = AudioProcessConfig::new()
        .with_input(video.to_str().unwrap().to_string())
        .with_output(output.to_str().unwrap().to_string())
        .with_loudnorm(loudnorm);

    match process_audio(&config) {
        Ok(_) => println!("✓ Test 3 output saved to: {:?}", output),
        Err(e) => eprintln!("✗ Test 3 failed: {}", e),
    }

    output
}

/// Test 4: Higher audio bitrate (256 kbps)
fn test_high_bitrate(video: &PathBuf, output_dir: &PathBuf) -> PathBuf {
    println!("\n=== Test 4: High Audio Bitrate (256 kbps) ===");

    let output = output_dir.join("output_audio_hq.mp4");

    let config = AudioProcessConfig::new()
        .with_input(video.to_str().unwrap().to_string())
        .with_output(output.to_str().unwrap().to_string())
        .with_audio_bitrate(256000);

    match process_audio(&config) {
        Ok(_) => println!("✓ Test 4 output saved to: {:?}", output),
        Err(e) => eprintln!("✗ Test 4 failed: {}", e),
    }

    output
}

/// Verify output file using ffprobe
fn verify_with_ffprobe(output_path: &PathBuf) {
    println!("\n--- Verifying output with ffprobe ---");
    println!("File: {:?}", output_path);

    // Check if file exists
    if !output_path.exists() {
        eprintln!("✗ Output file does not exist!");
        return;
    }

    // Run ffprobe to get file information
    let output = Command::new("ffprobe")
        .arg("-i")
        .arg(output_path)
        .arg("-hide_banner")
        .output();

    match output {
        Ok(result) => {
            let stderr = String::from_utf8_lossy(&result.stderr);
            println!("File info:\n{}", stderr);
        }
        Err(e) => {
            eprintln!("✗ Failed to run ffprobe: {}", e);
            eprintln!("  (ffprobe may not be installed)");
        }
    }
}
