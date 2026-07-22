pub mod audio_player;
mod downloader;
mod model;

use crate::slint_generatedAppWindow::AppWindow;
pub use model::save_transcribe_config;

pub fn app_launch_init(ui: &AppWindow) {
    downloader::init(ui);
}

pub fn init(ui: &AppWindow) {
    model::init(ui);
    audio_player::init(ui);
}
