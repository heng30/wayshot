//! Confirmation dialog logic module
//!
//! Handles confirmation dialog callbacks for various application actions.

use crate::{global_logic, global_util, slint_generatedAppWindow::AppWindow};
use slint::ComponentHandle;

/// Initializes confirmation dialog callbacks
///
/// Sets up handlers for different confirmation dialog actions.
///
/// # Parameters
/// - `ui`: Reference to the application window
pub fn init(ui: &AppWindow) {
    let ui_weak = ui.as_weak();
    global_util!(ui).on_handle_confirm_dialog(move |handle_type, user_data| {
        let ui = ui_weak.unwrap();

        match handle_type.as_str() {
            "remove-caches" => {
                global_logic!(ui).invoke_remove_caches();
            }
            "uninstall" => {
                global_logic!(ui).invoke_uninstall();
            }
            "close-window" => {
                global_util!(ui).invoke_close_window();
            }
            "remove-no-found-histories" => {
                global_logic!(ui).invoke_remove_no_found_histories();
            }
            "remove-all-histories" => {
                global_logic!(ui).invoke_remove_all_histories();
            }
            "remove-history" => {
                let index = user_data.parse::<i32>().unwrap_or(-1);
                global_logic!(ui).invoke_remove_history(index);
            }
            _ => (),
        }
    });
}
