use video_editor::font;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    log::info!("Getting system fonts information...");

    let fonts = font::get_fonts_info()?;

    log::info!("Found {} font families:", fonts.len());

    // Display first 50 fonts
    for (i, (name, path, _family)) in fonts.iter().take(50).enumerate() {
        log::info!("  {}: {} -> {}", i + 1, name, path.display());
    }

    if fonts.len() > 50 {
        log::info!("  ... and {} more fonts", fonts.len() - 50);
    }

    Ok(())
}
