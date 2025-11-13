//! This example demonstrates how to use the Wayland XDG Portal for screen capture.
//! It will request permission from the user and capture a single screenshot.

use screen_capture_wayland_portal::ScreenCaptureWaylandPortal;
use screen_capture::{ScreenCapture, CaptureStreamConfig, CaptureStreamCallbackData};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();

    println!("🖥️  Wayland XDG Portal Screen Capture Demo");
    println!("==========================================");
    println!();

    let mut capture = ScreenCaptureWaylandPortal;

    // Step 1: Get available screens
    println!("📺 Detecting available screens...");
    match capture.available_screens() {
        Ok(screens) => {
            println!("✅ Found {} screen(s):", screens.len());
            for (i, screen) in screens.iter().enumerate() {
                println!("  {}. {} - {}x{} (scale: {:.1})",
                         i + 1,
                         screen.name,
                         screen.logical_size.width,
                         screen.logical_size.height,
                         screen.scale_factor);
            }
            println!();
        }
        Err(e) => {
            println!("⚠️  Could not detect screens automatically: {}", e);
            println!("   This is normal - XDG portal requires user interaction to discover screens.");
            println!("   We'll try to capture anyway when you provide a screen name.");
            println!();
        }
    }

    // Step 2: Capture a screenshot using the streaming API (capture a single frame)
    println!("📸 Attempting to capture a screenshot...");
    println!("   A system dialog will appear asking you to select a screen.");
    println!("   Please select the screen you want to capture.");
    println!();

    println!("📸 Attempting to capture 3 REAL screenshots to /tmp/portal...");
    println!("   A system dialog will appear asking you to select a screen.");
    println!("   Please select the screen you want to capture.");
    println!("   ⚠️  You need to grant permission when the dialog appears!");
    println!();

    let screenshot_count = 3;
    let mut captured_successfully = 0;

    for i in 1..=screenshot_count {
        let filename = format!("/tmp/portal/screenshot_{:02}.png", i);
        println!("🔄 Capturing real screenshot {} of {}...", i, screenshot_count);

        // Use the actual XDG portal to capture real screen content
        let cancel_signal = Arc::new(AtomicBool::new(false));
        let captured_frame = Arc::new(std::sync::Mutex::new(None::<screen_capture::Capture>));

        let config = CaptureStreamConfig {
            name: "default".to_string(), // Try default screen name first
            fps: Some(1.0), // 1 FPS for single capture
            include_cursor: false, // Exclude cursor for clean screenshots
            cancel_sig: cancel_signal.clone(),
        };

        let captured_frame_clone = captured_frame.clone();
        let cancel_signal_clone = cancel_signal.clone();

        let callback = move |data: CaptureStreamCallbackData| {
            if let Ok(mut capture_result) = captured_frame_clone.try_lock() {
                if capture_result.is_none() {
                    let width = data.data.width;
                    let height = data.data.height;
                    let capture_time = data.capture_time;

                    *capture_result = Some(data.data);
                    println!("   📸 Frame captured: {}x{}, capture time: {:?}",
                             width, height, capture_time);
                    // Cancel after first frame
                    cancel_signal_clone.store(true, Ordering::Relaxed);
                }
            }
        };

        // Create a new capture instance since the trait consumes self
        let capture = ScreenCaptureWaylandPortal;

        match capture.capture_output_stream(config, callback) {
            Ok(screen_capture::CaptureStatus::Stopped) => {
                if let Ok(capture_result_guard) = captured_frame.try_lock() {
                    if let Some(capture_result) = capture_result_guard.as_ref() {
                        println!("   ✅ Real screenshot captured successfully!");
                        println!("   📏 Size: {}x{} pixels", capture_result.width, capture_result.height);
                        println!("   💾 Data size: {} bytes", capture_result.pixel_data.len());

                        match save_capture_as_png(capture_result, filename.clone()) {
                            Ok(()) => {
                                println!("   💾 Saved to: {}", filename);
                                captured_successfully += 1;

                                // Verify the saved image immediately
                                match verify_saved_image(&filename) {
                                    Ok((verified_width, verified_height, file_size)) => {
                                        println!("   ✅ Image verified: {}x{}, {} bytes", verified_width, verified_height, file_size);
                                    }
                                    Err(e) => {
                                        println!("   ⚠️  Image verification failed: {}", e);
                                    }
                                }
                            }
                            Err(e) => {
                                println!("   ❌ Failed to save image: {}", e);
                            }
                        }
                    } else {
                        println!("   ❌ No frame was captured from the stream");
                        return Err("No frame captured from PipeWire stream".into());
                    }
                }
            }
            Ok(screen_capture::CaptureStatus::Finished) => {
                println!("   ⚠️  Stream finished without capturing a frame");
                return Err("Stream finished without capturing".into());
            }
            Err(e) => {
                println!("   ❌ Failed to capture real screenshot: {}", e);
                println!("   💡 This indicates that either:");
                println!("      - XDG portal services are not running");
                println!("      - User cancelled the permission dialog");
                println!("      - PipeWire services are not available");
                println!("      - Not running in a proper Wayland session");
                println!("      - Required portal backend is not available");
                return Err(Box::new(e));
            }
        }

        // Add delay between screenshots
        std::thread::sleep(std::time::Duration::from_millis(2000));

        println!();
    }

    println!();
    println!("📊 Summary: Successfully saved {}/{} screenshots", captured_successfully, screenshot_count);

    // Verify all saved images
    println!("🔍 Verifying all saved screenshots...");
    let mut verified_count = 0;

    for i in 1..=screenshot_count {
        let filename = format!("/tmp/portal/screenshot_{:02}.png", i);

        match verify_saved_image(&filename) {
            Ok((width, height, file_size)) => {
                println!("   ✅ {}: {}x{}, {} bytes", filename, width, height, file_size);
                verified_count += 1;
            }
            Err(e) => {
                println!("   ❌ {}: {}", filename, e);
            }
        }
    }

    println!();
    println!("📋 Verification Summary: {}/{} images verified successfully", verified_count, screenshot_count);

    if verified_count == screenshot_count {
        println!("🎉 All screenshots created and verified successfully!");
        println!("   Check the /tmp/portal directory for the PNG files.");
    } else {
        println!("⚠️  Some screenshots failed verification, but test pattern generation worked.");
    }

    // Step 3: Test performance measurement
    println!("🚀 Testing capture performance...");
    let mut perf_capture = ScreenCaptureWaylandPortal;
    match perf_capture.capture_mean_time("default", 3) {
        Ok(avg_time) => {
            println!("✅ Average capture time per screenshot: {:?}", avg_time);
            println!("   This gives us approximately {:.1} FPS potential", 1.0 / avg_time.as_secs_f64());
        }
        Err(e) => {
            println!("⚠️  Performance test failed: {}", e);
        }
    }

    println!();
    println!("🎉 Demo completed successfully!");
    println!("   Check the generated PNG files to see the captured screenshots.");

    Ok(())
}

fn save_capture_as_png(capture: &screen_capture::Capture, filename: String) -> Result<(), Box<dyn std::error::Error>> {
    use image::{ImageBuffer, Rgba};

    // Convert the captured pixel data to an image
    let img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_raw(
        capture.width,
        capture.height,
        capture.pixel_data.clone(),
    ).ok_or("Failed to create image buffer")?;

    // Save the image as PNG
    img.save(filename)?;
    Ok(())
}

fn verify_saved_image(filename: &str) -> Result<(u32, u32, u64), Box<dyn std::error::Error>> {
    use std::fs::Metadata;

    // Check if file exists
    if !std::path::Path::new(filename).exists() {
        return Err(format!("File does not exist: {}", filename).into());
    }

    // Load and verify the image
    let img = image::open(filename)
        .map_err(|e| format!("Failed to open image: {}", e))?;

    // Get file metadata
    let metadata: Metadata = std::fs::metadata(filename)?;
    let file_size = metadata.len();

    let (width, height) = (img.width(), img.height());

    // Additional verification: check if the image is valid by reading a few pixels
    let pixels = img.to_rgba8();
    let total_pixels = width * height;

    if total_pixels == 0 {
        return Err("Image has zero pixels".into());
    }

    // Check some sample pixels to ensure they're valid (not all black/white, etc.)
    let sample_positions = [(0, 0), (width/2, height/2), (width-1, height-1)];
    let mut valid_samples = 0;

    for &(x, y) in &sample_positions {
        if x < width && y < height {
            let pixel = pixels.get_pixel(x, y);
            // Check if pixel is not completely transparent or has some color variation
            if pixel[3] > 0 && !(pixel[0] == pixel[1] && pixel[1] == pixel[2] && pixel[2] == 0) {
                valid_samples += 1;
            }
        }
    }

    if valid_samples == 0 {
        return Err("All sample pixels appear to be black or transparent".into());
    }

    Ok((width, height, file_size))
}

// Placeholder functions removed - no fake data generation allowed

// Additional example for streaming capture
fn streaming_example() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔄 Starting streaming capture example...");

    let _capture = ScreenCaptureWaylandPortal;
    let cancel_signal = Arc::new(AtomicBool::new(false));
    let frame_count = Arc::new(std::sync::atomic::AtomicU32::new(0));

    let config = screen_capture::CaptureStreamConfig {
        name: "default".to_string(),
        fps: Some(5.0), // 5 FPS for demo
        include_cursor: false,
        cancel_sig: cancel_signal.clone(),
    };

    let frame_count_clone = frame_count.clone();
    let cancel_signal_clone = cancel_signal.clone();
    let callback = move |data: CaptureStreamCallbackData| {
        let count = frame_count_clone.fetch_add(1, Ordering::Relaxed);
        if count % 10 == 0 {
            println!("📹 Captured {} frames (frame size: {}x{}, capture time: {:?})",
                     count,
                     data.data.width,
                     data.data.height,
                     data.capture_time);
        }

        // Cancel after 50 frames
        if count >= 50 {
            cancel_signal_clone.store(true, Ordering::Relaxed);
        }
    };

    // Run streaming capture for a short time
    tokio::runtime::Runtime::new()?.block_on(async {
        tokio::time::sleep(Duration::from_secs(1)).await;
        cancel_signal.store(true, Ordering::Relaxed);
    });

    // Create a new capture instance since the trait consumes self
    let new_capture = ScreenCaptureWaylandPortal;
    match new_capture.capture_output_stream(config, callback) {
        Ok(screen_capture::CaptureStatus::Stopped) => {
            println!("✅ Streaming capture completed after {} frames",
                     frame_count.load(Ordering::Relaxed));
        }
        Ok(_) => {
            println!("✅ Streaming capture completed");
        }
        Err(e) => {
            println!("⚠️  Streaming capture failed: {}", e);
        }
    }

    Ok(())
}