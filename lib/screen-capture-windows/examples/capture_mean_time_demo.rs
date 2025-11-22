use screen_capture_windows::{available_screens, capture_mean_time};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let screen_infos = available_screens()?;
    if let Ok(avg_time) = capture_mean_time(&screen_infos[0].name, 10) {
        println!("Average capture time: {:.2?}", avg_time);
    } else {
        println!("Failed to measure capture time");
    }

    Ok(())
}
