mod audio;
mod conversion;
mod filter;
pub mod global;
pub mod keyframe;
pub mod subtitle;
mod video;

pub use filter::create_filter_command_with_detail;
pub use keyframe::refresh_selected_filter_detail_at_playhead;

pub fn init(ui: &crate::slint_generatedAppWindow::AppWindow) {
    global::init(ui);
    filter::init(ui);
    video::init(ui);
    audio::init(ui);
    subtitle::init(ui);
    keyframe::init(ui);
}
