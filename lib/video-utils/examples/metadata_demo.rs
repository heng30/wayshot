use std::path::PathBuf;
use video_utils::metadata::get_metadata;

fn main() {
    env_logger::init();

    // Test with the example video file
    let test_video = PathBuf::from("data/test.mp4");

    if !test_video.exists() {
        eprintln!("Test video not found: {:?}", test_video);
        eprintln!("Please place a test video file at data/test.mp4");
        std::process::exit(1);
    }

    println!("╔════════════════════════════════════════════════════════════════╗");
    println!("║              Video Metadata Example                                ║");
    println!("╚════════════════════════════════════════════════════════════════╝");
    println!();

    match get_metadata(&test_video) {
        Ok(metadata) => {
            println!("Basic Information:");
            println!("==================");
            println!("  File:       {}", metadata.path);
            println!("  Format:     {}", metadata.format_name);
            println!("  Duration:   {:.2} seconds", metadata.duration);
            println!("  Bitrate:    {} bps ({:.2} Mbps)",
                metadata.bitrate,
                metadata.bitrate as f64 / 1_000_000.0);
            println!("  Size:       {} bytes ({:.2} MB)",
                metadata.size,
                metadata.size as f64 / 1_048_576.0);
            println!();

            println!("Streams:");
            println!("========");
            println!("  Video streams: {}", metadata.video_streams_count);
            println!("  Audio streams: {}", metadata.audio_streams_count);
            println!();

            println!("Summary:");
            println!("========");
            if metadata.video_streams_count > 0 {
                println!("  ✓ This file contains video");
            }
            if metadata.audio_streams_count > 0 {
                println!("  ✓ This file contains audio");
            }
            println!();

            println!("Quick Info:");
            println!("===========");
            println!("  Resolution info: Use ffprobe for detailed stream information");
            println!("  Command: ffprobe -v quiet -print_format json -show_streams \"{:?}\"", test_video);
        }
        Err(e) => {
            eprintln!("Error getting metadata: {}", e);
            std::process::exit(1);
        }
    }

    println!();
    println!("╔════════════════════════════════════════════════════════════════╗");
    println!("║                     Test Complete                                  ║");
    println!("╚════════════════════════════════════════════════════════════════╝");
}
