use recorder::AudioRecorder;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let recorder = AudioRecorder::new(None)?;

    println!("\nAvailable Audio Devices:");
    println!("------------------------");

    let devices = recorder.get_available_devices()?;
    for (i, device) in devices.iter().enumerate() {
        println!(
            "{}. {} ({}) {:?}",
            i + 1,
            device.name,
            "Input",
            device.default_config
        );
    }

    println!("\nDefault Input Device:");
    if let Some(default_input) = recorder.get_default_input_device()? {
        println!(
            "  {} {:?}",
            default_input.name, default_input.default_config
        );
    } else {
        println!("  No default input device found");
    }

    Ok(())
}
