mod audio_player;
mod process;

pub fn init(ui: &crate::slint_generatedAppWindow::AppWindow) {
    audio_player::init(ui);
    process::init(ui);
}
