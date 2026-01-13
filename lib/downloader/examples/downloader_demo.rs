use anyhow::Result;
use downloader::{DownloadError, DownloadState, Downloader};
use std::io::Write;

#[tokio::main]
async fn main() -> Result<(), DownloadError> {
    let save_path = "./test_video.mp4";
    let download_url =
        "https://freetestdata.com/wp-content/uploads/2022/02/Free_Test_Data_1MB_MP4.mp4";

    println!("Starting download from: {}", download_url);
    println!("Saving to: {}", save_path);
    println!("Press Ctrl+C to cancel...\n");

    let downloader = Downloader::new(download_url.to_string(), save_path.into());

    match downloader
        .start(|downloaded: u64, total: u64, progress: f32| {
            let percent = progress * 100.0;
            let mb_downloaded = downloaded as f64 / 1024.0 / 1024.0;
            let mb_total = total as f64 / 1024.0 / 1024.0;
            print!(
                "\rProgress: {:.2}% ({:.2} MB / {:.2} MB)",
                percent, mb_downloaded, mb_total
            );
            std::io::stdout().flush().unwrap();
        })
        .await
    {
        Ok(DownloadState::Finsished) => {
            println!("\n✓ Download completed successfully!");
            println!("File saved to: {}", save_path);
        }
        Ok(DownloadState::Cancelled) => {
            println!("\n✗ Download was cancelled!");
        }
        Ok(DownloadState::Incompleted) => {
            println!("\n✗ Download as incompleted!");
        }
        Err(e) => {
            println!("\n✗ Download failed: {}", e);
            return Err(e);
        }
    }

    Ok(())
}
