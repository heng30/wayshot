use screen_capture_windows::DXGIManager;

fn main() {
    // List all available screens
    match DXGIManager::list_available_screens() {
        Ok(screens) => {
            println!("Available screens:");
            for (i, screen) in screens.iter().enumerate() {
                println!("  {}: {}", i, screen);
            }

            if !screens.is_empty() {
                // Try to capture the first screen
                let screen_name = screens[0].split(" (").next().unwrap_or(&screens[0]);
                println!("\nTrying to capture screen: {}", screen_name);

                match DXGIManager::new(screen_name.to_string()) {
                    Ok(mut manager) => {
                        println!(
                            "Successfully initialized DXGIManager for screen: {}",
                            screen_name
                        );

                        // Get screen geometry (this returns a tuple directly, not a Result)
                        let (width, height) = manager.geometry();
                        println!("Screen geometry: {}x{}", width, height);

                        // Try to capture a frame
                        match manager.capture_frame_rgba() {
                            Ok((_buffer, (width, height))) => {
                                println!(
                                    "Successfully captured frame: {}x{} ({} bytes)",
                                    width,
                                    height,
                                    _buffer.len()
                                );
                            }
                            Err(e) => {
                                println!("Failed to capture frame: {:?}", e);
                            }
                        }
                    }
                    Err(e) => {
                        println!("Failed to create DXGIManager: {:?}", e);
                    }
                }
            }
        }
        Err(e) => {
            println!("Failed to list screens: {:?}", e);
        }
    }
}

