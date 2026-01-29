use video_utils::filters::transform::{rotate_90};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    
    println!("Testing 90 degree rotation...");
    match rotate_90("data/test.mp4", "tmp/test_rotate_90.mp4") {
        Ok(_) => println!("✓ Success"),
        Err(e) => println!("❌ Failed: {}", e),
    }
    
    Ok(())
}
