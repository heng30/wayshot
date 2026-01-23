mod audio_player;
mod downloader;
mod model;

pub fn init(ui: &crate::slint_generatedAppWindow::AppWindow) {
    model::init(ui);
    downloader::init(ui);
    audio_player::init(ui);
}
