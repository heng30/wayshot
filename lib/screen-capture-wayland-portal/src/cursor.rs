use crate::Error;
use screen_capture::{CursorPosition, MonitorCursorPositionConfig};

pub fn monitor_cursor_position(
    _config: MonitorCursorPositionConfig,
    _callback: impl FnMut(CursorPosition) + Send + 'static,
) -> Result<(), Error> {
    Err(Error::Other(
        "Cursor position monitoring is not supported with XDG Portal".to_string(),
    ))
}