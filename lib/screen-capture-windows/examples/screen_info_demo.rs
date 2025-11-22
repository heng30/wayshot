use screen_capture_windows::available_screens;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("XDG Portal Screen Info Demo");
    println!("==========================");

    // Get available screens
    println!("Querying available screens...");
    match available_screens() {
        Ok(screens) => {
            println!("Found {} screen(s):", screens.len());

            if screens.is_empty() {
                println!("No screens available. This might mean:");
                println!("1. No XDG Portal service is running");
                println!("2. The user denied permission");
                println!("3. No monitors are connected");
                return Ok(());
            }

            for (i, screen) in screens.iter().enumerate() {
                println!("\nScreen {}:", i + 1);
                println!("  Name: {}", screen.name);
                println!(
                    "  Logical Size: {}x{}",
                    screen.logical_size.width, screen.logical_size.height
                );
                println!("  Scale Factor: {:.2}", screen.scale_factor);
                println!("  Position: ({}, {})", screen.position.x, screen.position.y);

                if let Some(ref physical) = screen.physical_size {
                    println!("  Physical Size: {}x{}", physical.width, physical.height);
                }

                println!("  Transform: {:?}", screen.transform);
            }

            println!("\nDemo completed successfully!");
        }
        Err(e) => {
            println!("Failed to get available screens: {}", e);
            return Err(e.into());
        }
    }

    Ok(())
}
