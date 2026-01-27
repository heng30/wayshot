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

    println!("Video Metadata Example");
    println!("======================\n");

    match get_metadata(&test_video) {
        Ok(metadata) => {
            println!("File: {}", metadata.path);
            println!("Format: {} ({})", metadata.format.format_name, metadata.format.format_long_name);
            println!("Duration: {:.2} seconds", metadata.duration);
            println!("Bitrate: {} bps ({:.2} Mbps)", metadata.bitrate, metadata.bitrate as f64 / 1_000_000.0);
            println!("Size: {} bytes ({:.2} MB)", metadata.size, metadata.size as f64 / 1_048_576.0);
            println!("Streams: {}", metadata.format.nb_streams);
            println!();

            // Video streams
            if !metadata.video_streams.is_empty() {
                println!("Video Streams:");
                for (i, video) in metadata.video_streams.iter().enumerate() {
                    println!("  [Stream #{}]", i);
                    println!("    Codec: {} ({})", video.codec, video.codec_long_name);
                    println!("    Resolution: {}x{}", video.width, video.height);
                    println!("    Frame Rate: {}", video.r_frame_rate);
                    println!("    Pixel Format: {}", video.pix_fmt);
                    println!("    Aspect Ratio: {}", video.display_aspect_ratio);
                    if let Some(bitrate) = video.bitrate {
                        println!("    Bitrate: {} bps ({:.2} Mbps)", bitrate, bitrate as f64 / 1_000_000.0);
                    }
                    if let Some(nb_frames) = video.nb_frames {
                        println!("    Frames: {}", nb_frames);
                    }
                    println!();
                }
            }

            // Audio streams
            if !metadata.audio_streams.is_empty() {
                println!("Audio Streams:");
                for (i, audio) in metadata.audio_streams.iter().enumerate() {
                    println!("  [Stream #{}]", i);
                    println!("    Codec: {} ({})", audio.codec, audio.codec_long_name);
                    println!("    Sample Rate: {} Hz", audio.sample_rate);
                    println!("    Channels: {}", audio.channels);
                    println!("    Channel Layout: {}", audio.channel_layout);
                    println!("    Sample Format: {}", audio.sample_fmt);
                    if let Some(bitrate) = audio.bitrate {
                        println!("    Bitrate: {} bps ({:.2} kbps)", bitrate, bitrate as f64 / 1_000.0);
                    }
                    if let Some(nb_frames) = audio.nb_frames {
                        println!("    Frames: {}", nb_frames);
                    }
                    if let Some(bps) = audio.bits_per_sample {
                        println!("    Bits per Sample: {}", bps);
                    }
                    println!();
                }
            }

            // Subtitle streams
            if !metadata.subtitle_streams.is_empty() {
                println!("Subtitle Streams:");
                for (i, subtitle) in metadata.subtitle_streams.iter().enumerate() {
                    println!("  [Stream #{}]", i);
                    println!("    Codec: {} ({})", subtitle.codec, subtitle.codec_long_name);
                    if let Some(language) = &subtitle.language {
                        println!("    Language: {}", language);
                    }
                    println!();
                }
            }

            // Output as JSON
            println!("JSON Output:");
            println!("===========");
            match serde_json::to_string_pretty(&metadata) {
                Ok(json) => println!("{}", json),
                Err(e) => eprintln!("Failed to serialize to JSON: {}", e),
            }
        }
        Err(e) => {
            eprintln!("Error getting metadata: {}", e);
            std::process::exit(1);
        }
    }
}
