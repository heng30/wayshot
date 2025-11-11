use mp4_player::{Result, metadata};

fn main() -> Result<()> {
    env_logger::init();
    let path = "/tmp/test.mp4";
    let meta = metadata::parse(path)?;
    log::info!("{meta:#?}");
    Ok(())
}

