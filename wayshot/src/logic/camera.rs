use crate::{logic_cb, slint_generatedAppWindow::AppWindow};
use camera::{self, query_available_cameras};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

pub fn init(ui: &AppWindow) {
    // logic_cb!(available_cameras, ui);
}

pub fn available_cameras() -> Vec<SharedString> {
    camera::init();

    query_available_cameras()
        .into_iter()
        .map(|c| c.name.into())
        .collect::<Vec<SharedString>>()
}
