use screen_capture_wayland_wlr as capture;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = capture::available_screens()?;
    println!("output: {:#?}", output);

    Ok(())
}
