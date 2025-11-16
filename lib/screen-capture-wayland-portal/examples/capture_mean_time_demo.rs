use screen_capture_wayland_portal::{available_screens, capture_mean_time};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let screen_infos = available_screens()?;
    match capture_mean_time(&screen_infos[0].name, 3) {
        Ok(avg_time) => println!("Average capture time: {avg_time:.2?}"),
        Err(e) => println!("Failed to measure capture time. {e}"),
    }

    Ok(())
}
