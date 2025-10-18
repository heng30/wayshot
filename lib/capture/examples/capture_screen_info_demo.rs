fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = capture::available_screens()?;
    println!("output: {:#?}", output);

    Ok(())
}
