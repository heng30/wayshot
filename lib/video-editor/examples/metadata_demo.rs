use std::path::PathBuf;
use video_editor::metadata::get_metadata;

fn main() {
    env_logger::init();

    let files = vec![
        "test.mp4", "test.mkv", "test.wav", "test.srt", "test.png", "test.txt",
    ];
    for file in files {
        let file = PathBuf::from("data").join(file);
        match get_metadata(&file) {
            Ok(metadata) => log::info!("{metadata:#?}"),
            Err(e) => log::warn!("Error getting `{}` metadata: {e}", file.display()),
        }
    }
}
