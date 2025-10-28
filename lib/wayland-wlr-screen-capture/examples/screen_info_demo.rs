use wayland_wlr_screen_capture as capture;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = capture::available_screens()?;
    println!("output: {:#?}", output);

    Ok(())
}
