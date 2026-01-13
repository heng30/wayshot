use crate::{DownloadError, Result};
use futures::StreamExt;
use reqwest::Client;
use std::{
    fs,
    io::Write,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

pub enum DownloadStatus {
    Finsished,
    Cancelled,
    Downloading,
}

#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Downloader {
    url: String,
    save_path: String,
    cancel_sig: Arc<AtomicBool>,
}

impl Downloader {
    pub fn new(url: String, save_path: String) -> Downloader {
        Downloader {
            url,
            save_path,
            cancel_sig: Arc::new(AtomicBool::new(false)),
        }
    }

    pub async fn start(
        &self,
        mut progress_cb: impl FnMut(u64, u64, f32) + 'static,
    ) -> Result<DownloadStatus> {
        let tmp_filepath = format!("{}.tmp", self.save_path);

        let mut save_file =
            fs::File::create(&tmp_filepath).map_err(|e| DownloadError::FileCreateError {
                error: e,
                path: tmp_filepath.clone(),
            })?;

        let response =
            Client::new()
                .get(&self.url)
                .send()
                .await
                .map_err(|e| DownloadError::RequestError {
                    error: e,
                    url: self.url.to_string(),
                })?;

        let total_size = response
            .content_length()
            .ok_or_else(|| DownloadError::ContentLengthError)?;

        let mut downloaded: u64 = 0;
        let mut stream = response.bytes_stream();

        while let Some(chunk) = stream.next().await {
            if self.cancel_sig.load(Ordering::Relaxed) {
                return Ok(DownloadStatus::Cancelled);
            }

            let chunk = chunk.map_err(|e| DownloadError::IncompleteDownload {
                error: e.to_string(),
                downloaded,
                total: total_size,
            })?;
            save_file.write_all(&chunk)?;

            downloaded += chunk.len() as u64;

            let progress = downloaded as f32 / total_size as f32;
            progress_cb(downloaded, total_size, progress);
        }

        if total_size == downloaded {
            _ = fs::rename(&tmp_filepath, &self.save_path);
            Ok(DownloadStatus::Finsished)
        } else {
            Ok(DownloadStatus::Downloading)
        }
    }

    pub fn cancel(&self) {
        self.cancel_sig.store(true, Ordering::Relaxed);
    }

    pub fn cancel_sig(&self) -> Arc<AtomicBool> {
        self.cancel_sig.clone()
    }
}
