use crate::{
    global_store, global_terminal_state, global_ve_filter,
    slint_generatedAppWindow::{AppWindow, TermSpan, TermTabModel},
    terminal_state_cb,
};
use slint::{ComponentHandle, Model as SlintModel, ModelRc, Timer, VecModel, Weak};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use term::{
    Result,
    input::{InputHandler, KeyModifiers, MouseButton},
    render::SpanBuilder,
    tabs::Tab,
    tabs::TabManager,
    terminal::{Terminal, TerminalSize},
    theme::{ColorPalette, Theme},
};

const DEFAULT_FONT_SIZE: f32 = 16.0;
static BRIDGE: Mutex<Option<BridgeState>> = Mutex::new(None);

#[macro_export]
macro_rules! store_terminal_state_spans {
    ($ui:expr) => {
        crate::global_terminal_state!($ui)
            .get_spans()
            .as_any()
            .downcast_ref::<VecModel<TermSpan>>()
            .expect("We know we set a VecModel<TermSpan> earlier for terminal state")
    };
}

#[macro_export]
macro_rules! store_terminal_state_tabs {
    ($ui:expr) => {
        crate::global_terminal_state!($ui)
            .get_tabs()
            .as_any()
            .downcast_ref::<VecModel<TermTabModel>>()
            .expect("We know we set a VecModel<TermTabModel> earlier for terminal state")
    };
}

/// Shared bridge state that can be accessed from timer callbacks
struct BridgeState {
    span_builder: SpanBuilder,
    tab_manager: Arc<Mutex<TabManager>>,
    input_handler: InputHandler,
    /// Cached cell dimensions (read from Slint probe on each tick)
    cell_w: f32,
    cell_h: f32,
    /// Last PTY size (cols, rows) — used to detect when resize is needed
    pty_cols: u16,
    pty_rows: u16,
    /// Current font size in logical pixels (tracked for zoom)
    font_size: f32,
}

pub fn init(ui: &AppWindow) {
    inner_init(ui);

    terminal_state_cb!(new_tab, ui);
    terminal_state_cb!(close_tab, ui, index);
    terminal_state_cb!(tab_changed, ui, index);
    terminal_state_cb!(key_pressed_event, ui, text, shift, alt, ctrl);
    terminal_state_cb!(key_released_event, ui, text, shift, alt, ctrl);
    terminal_state_cb!(mouse_pressed, ui, button, x, y);
    terminal_state_cb!(mouse_released, ui, button, x, y);
    terminal_state_cb!(mouse_moved, ui, x, y);
    terminal_state_cb!(mouse_wheel, ui, delta);
    terminal_state_cb!(scroll_to, ui, position);
    terminal_state_cb!(font_zoom_in, ui);
    terminal_state_cb!(font_zoom_out, ui);
    terminal_state_cb!(font_zoom_reset, ui);
    terminal_state_cb!(ime_commit, ui, text);
    terminal_state_cb!(copy_request, ui);
    terminal_state_cb!(paste_request, ui);
    terminal_state_cb!(theme_changed, ui, is_dark);
}

fn inner_init(ui: &AppWindow) {
    let font_size = global_terminal_state!(ui).get_term_font_size() as f32;
    let terminal_size = TerminalSize { cols: 80, rows: 24 };

    let is_dark = global_store!(ui).get_setting_preference().is_dark;
    let theme = Theme::from_name(if is_dark { "dark" } else { "light" });
    let palette = theme.colors.clone();

    let mut tab_manager = match TabManager::with_size_and_palette(terminal_size, palette.clone()) {
        Ok(tm) => tm,
        Err(e) => {
            log::error!("Failed to create tab manager: {e}");
            return;
        }
    };

    // Spawn PTY reader threads for all tabs
    for tab in &mut tab_manager.tabs {
        if let Err(e) = tab.terminal.spawn_reader_thread() {
            log::error!("Failed to spawn PTY reader thread: {e}");
        }
    }

    let tab_manager = Arc::new(Mutex::new(tab_manager));
    let mut span_builder = SpanBuilder::new();
    span_builder.set_default_bg(palette.background);
    span_builder.set_default_fg(palette.foreground);
    span_builder.set_cursor_color(palette.cursor);
    span_builder.set_selection_bg(palette.selection);
    let input_handler = InputHandler::new();

    let state = BridgeState {
        span_builder,
        tab_manager: tab_manager.clone(),
        input_handler,
        cell_w: 8.0_f32,
        cell_h: 17.0_f32,
        pty_cols: 80,
        pty_rows: 24,
        font_size,
    };

    *BRIDGE.lock().unwrap() = Some(state);

    // Set the models on TerminalState
    let spans_model = ModelRc::new(VecModel::<TermSpan>::default());
    let tabs_model = ModelRc::new(VecModel::<TermTabModel>::default());
    global_terminal_state!(ui).set_spans(spans_model);
    global_terminal_state!(ui).set_tabs(tabs_model);

    let ui_weak = ui.as_weak();
    apply_theme_to_ui(ui, &palette);
    sync_tabs_to_ui(&ui_weak);

    // Start the display refresh timer (~25 FPS)
    let timer = Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(40),
        move || {
            if let Some(ui) = ui_weak.upgrade()
                && global_ve_filter!(ui).get_right_panel_selected_index() != 3
            {
                return; //  not in terminal tab
            }

            if let Some(state) = BRIDGE.lock().unwrap().as_mut()
                && let Err(e) = update_display(state, &ui_weak)
            {
                log::trace!("Display update error: {e}");
            }

            if let Some(ui) = ui_weak.upgrade() {
                ui.window().request_redraw();
            }
        },
    );

    // Keep the timer alive
    std::mem::forget(timer);

    log::info!("Terminal backend initialized (font_size={font_size:.1}px)");
}

fn new_tab(ui: &AppWindow) {
    let state = BRIDGE.lock().unwrap();
    let Some(state) = state.as_ref() else { return };

    let Ok(mut tab_manager) = state.tab_manager.try_lock() else {
        return;
    };

    match tab_manager.add_new_tab() {
        Ok(()) => {
            // Spawn reader thread for the new tab
            if let Some(tab) = tab_manager.tabs.last_mut()
                && let Err(e) = tab.terminal.spawn_reader_thread()
            {
                log::error!("Failed to spawn reader thread: {e}");
            }

            let tab_count = tab_manager.tab_count();

            if let Err(e) = tab_manager.set_active_tab(tab_count - 1) {
                log::error!("Failed to switch to new tab: {e}");
            }

            let titles: Vec<TermTabModel> = tab_manager
                .get_tab_titles()
                .into_iter()
                .map(|t| TermTabModel { title: t.into() })
                .collect();
            crate::store_terminal_state_tabs!(ui).set_vec(titles);
            global_terminal_state!(ui).set_active_tab_index((tab_count - 1) as i32);
        }
        Err(e) => log::error!("Failed to create new tab: {e}"),
    }
}

fn close_tab(ui: &AppWindow, index: i32) {
    let state = BRIDGE.lock().unwrap();
    let Some(state) = state.as_ref() else { return };
    let Ok(mut tab_manager) = state.tab_manager.try_lock() else {
        return;
    };

    let index = index as usize;
    if let Err(e) = tab_manager.remove_tab(index) {
        log::error!("Failed to close tab {index}: {e}");
    } else {
        let titles: Vec<TermTabModel> = tab_manager
            .get_tab_titles()
            .into_iter()
            .map(|t| TermTabModel { title: t.into() })
            .collect();
        crate::store_terminal_state_tabs!(ui).set_vec(titles);
    }
}

fn tab_changed(ui: &AppWindow, index: i32) {
    let state = BRIDGE.lock().unwrap();
    let Some(state) = state.as_ref() else { return };
    let Ok(mut tab_manager) = state.tab_manager.try_lock() else {
        return;
    };

    let index = index as usize;
    if let Err(e) = tab_manager.set_active_tab(index) {
        log::error!("Failed to switch to tab {index}: {e}");
    } else {
        global_terminal_state!(ui).set_active_tab_index(index as i32);
    }
}

fn key_pressed_event(
    ui: &AppWindow,
    text: slint::SharedString,
    shift: bool,
    alt: bool,
    ctrl: bool,
) {
    // Handle font zoom shortcuts: Ctrl+= zoom in, Ctrl+- zoom out, Ctrl+0 reset
    if ctrl && !alt && !shift {
        let is_zoom_in = text == "=" || text == "+";
        let is_zoom_out = text == "-";
        let is_zoom_reset = text == "0";

        if is_zoom_in || is_zoom_out || is_zoom_reset {
            let mut state = BRIDGE.lock().unwrap();
            let Some(state) = state.as_mut() else { return };
            let new_size = if is_zoom_in {
                (state.font_size + 1.0).min(32.0)
            } else if is_zoom_out {
                (state.font_size - 1.0).max(6.0)
            } else {
                DEFAULT_FONT_SIZE
            };
            state.font_size = new_size;
            global_terminal_state!(ui).set_term_font_size(new_size);
            return; // Don't forward zoom shortcuts to the terminal
        }
    }

    let modifiers = KeyModifiers {
        shift,
        alt,
        ctrl,
        meta: false,
    };

    let state = BRIDGE.lock().unwrap();
    let Some(state) = state.as_ref() else { return };
    if let Ok(mut tab_manager) = state.tab_manager.try_lock()
        && let Some(tab) = tab_manager.active_tab_mut()
    {
        if let Err(e) = state
            .input_handler
            .handle_key(&tab.terminal, &text, &text, modifiers)
        {
            log::error!("Failed to handle key: {e}");
        }

        // Scroll to bottom on Enter key
        if text == "\r" || text == "\n" {
            tab.terminal.scroll_to_bottom();
        }
    }
}

fn key_released_event(
    _ui: &AppWindow,
    _text: slint::SharedString,
    _shift: bool,
    _alt: bool,
    _ctrl: bool,
) {
    // Key release events are typically not forwarded to the terminal
}

fn mouse_pressed(_ui: &AppWindow, button: i32, x: i32, y: i32) {
    let button = match button {
        0 => MouseButton::Left,
        1 => MouseButton::Middle,
        2 => MouseButton::Right,
        _ => MouseButton::Left,
    };

    let mut state = BRIDGE.lock().unwrap();
    let Some(state) = state.as_mut() else { return };
    let (col, row) = screen_to_grid(x as f32, y as f32, state.cell_w, state.cell_h);
    if let Err(e) = state.input_handler.handle_mouse_press(col, row, button) {
        log::error!("Failed to handle mouse press: {e}");
    }

    // Handle middle click paste
    if button == MouseButton::Middle
        && let Ok(mut tab_manager) = state.tab_manager.try_lock()
        && let Some(tab) = tab_manager.active_tab_mut()
    {
        _ = paste_to_terminal(&tab.terminal);
    }
}

fn mouse_released(_ui: &AppWindow, button: i32, x: i32, y: i32) {
    let button = match button {
        0 => MouseButton::Left,
        1 => MouseButton::Middle,
        2 => MouseButton::Right,
        _ => MouseButton::Left,
    };

    let mut state = BRIDGE.lock().unwrap();
    let Some(state) = state.as_mut() else { return };
    let (col, row) = screen_to_grid(x as f32, y as f32, state.cell_w, state.cell_h);
    if let Err(e) = state.input_handler.handle_mouse_release(col, row, button) {
        log::error!("Failed to handle mouse release: {e}");
    }

    // Auto-copy selected text to clipboard on left mouse release
    if button == MouseButton::Left {
        let has_selection = state.input_handler.get_selection().map_or(false, |sel| {
            let (sr, sc, er, ec) = sel.normalized();
            sr != er || sc != ec
        });

        if has_selection {
            if let Ok(tab_manager) = state.tab_manager.try_lock()
                && let Some(tab) = tab_manager.active_tab()
                && let Some(text) = state
                    .input_handler
                    .get_selected_text(&tab.terminal)
                    .filter(|t| !t.is_empty())
            {
                match arboard::Clipboard::new() {
                    Ok(mut ctx) => {
                        if let Err(e) = ctx.set_text(&text) {
                            log::error!("Failed to copy to clipboard: {e}");
                        }
                    }
                    Err(e) => log::error!("Failed to access clipboard: {e}"),
                }
            }
        } else {
            state.input_handler.clear_selection();
        }
    }
}

fn mouse_moved(_ui: &AppWindow, x: i32, y: i32) {
    let mut state = BRIDGE.lock().unwrap();
    let Some(state) = state.as_mut() else { return };
    let (col, row) = screen_to_grid(x as f32, y as f32, state.cell_w, state.cell_h);
    if let Err(e) = state.input_handler.handle_mouse_move(col, row) {
        log::error!("Failed to handle mouse move: {e}");
    }
}

fn mouse_wheel(_ui: &AppWindow, delta: f32) {
    let state = BRIDGE.lock().unwrap();
    let Some(state) = state.as_ref() else { return };
    let Ok(mut tab_manager) = state.tab_manager.try_lock() else {
        return;
    };

    if let Some(tab) = tab_manager.active_tab_mut() {
        let lines = (delta.abs() / 60.0 * 5.0).ceil() as usize;
        let lines = lines.max(1).min(50);
        if delta > 0.0 {
            tab.terminal.scroll_up(lines);
        } else {
            tab.terminal.scroll_down(lines);
        }
    }
}

fn scroll_to(_ui: &AppWindow, position: i32) {
    let state = BRIDGE.lock().unwrap();
    let Some(state) = state.as_ref() else { return };
    if let Ok(mut tab_manager) = state.tab_manager.try_lock()
        && let Some(tab) = tab_manager.active_tab_mut()
    {
        tab.terminal.scroll_to_position(position as usize);
    }
}

fn font_zoom_in(ui: &AppWindow) {
    let mut state = BRIDGE.lock().unwrap();
    let Some(state) = state.as_mut() else { return };
    let new_size = (state.font_size + 1.0).min(32.0);
    state.font_size = new_size;
    global_terminal_state!(ui).set_term_font_size(new_size);
}

fn font_zoom_out(ui: &AppWindow) {
    let mut state = BRIDGE.lock().unwrap();
    let Some(state) = state.as_mut() else { return };
    let new_size = (state.font_size - 1.0).max(6.0);
    state.font_size = new_size;
    global_terminal_state!(ui).set_term_font_size(new_size);
}

fn font_zoom_reset(ui: &AppWindow) {
    let mut state = BRIDGE.lock().unwrap();
    let Some(state) = state.as_mut() else { return };
    state.font_size = DEFAULT_FONT_SIZE;
    global_terminal_state!(ui).set_term_font_size(DEFAULT_FONT_SIZE);
}

fn ime_commit(_ui: &AppWindow, text: slint::SharedString) {
    if text.is_empty() {
        return;
    }
    let state = BRIDGE.lock().unwrap();
    let Some(state) = state.as_ref() else { return };
    if let Ok(mut tab_manager) = state.tab_manager.try_lock()
        && let Some(tab) = tab_manager.active_tab_mut()
        && let Err(e) = tab.terminal.write_str(&text)
    {
        log::error!("Failed to write IME text to terminal: {e}");
    }
}

fn copy_request(_ui: &AppWindow) {
    let state = BRIDGE.lock().unwrap();
    let Some(state) = state.as_ref() else { return };
    if let Ok(tab_manager) = state.tab_manager.try_lock()
        && let Some(tab) = tab_manager.active_tab()
        && let Some(text) = state
            .input_handler
            .get_selected_text(&tab.terminal)
            .filter(|t| !t.is_empty())
    {
        match arboard::Clipboard::new() {
            Ok(mut ctx) => {
                if let Err(e) = ctx.set_text(&text) {
                    log::error!("Failed to copy to clipboard: {e}");
                }
            }
            Err(e) => log::error!("Failed to access clipboard: {e}"),
        }
    }
}

fn paste_request(_ui: &AppWindow) {
    let state = BRIDGE.lock().unwrap();
    let Some(state) = state.as_ref() else { return };
    if let Ok(mut tab_manager) = state.tab_manager.try_lock()
        && let Some(tab) = tab_manager.active_tab_mut()
    {
        _ = paste_to_terminal(&tab.terminal);
    }
}

fn theme_changed(ui: &AppWindow, is_dark: bool) {
    let theme = Theme::from_name(if is_dark { "dark" } else { "light" });
    let palette = theme.colors.clone();

    // Update BridgeState: SpanBuilder colors + TabManager palette
    let mut state = BRIDGE.lock().unwrap();
    let Some(state) = state.as_mut() else { return };
    state.span_builder.set_default_bg(palette.background);
    state.span_builder.set_default_fg(palette.foreground);
    state.span_builder.set_cursor_color(palette.cursor);
    state.span_builder.set_selection_bg(palette.selection);

    if let Ok(mut tab_manager) = state.tab_manager.try_lock() {
        tab_manager.set_color_palette(palette.clone());
    }

    apply_theme_to_ui(ui, &palette);
}

fn apply_theme_to_ui(ui: &AppWindow, palette: &ColorPalette) {
    let ts = global_terminal_state!(ui);
    let [r, g, b] = palette.background;
    ts.set_term_bg(slint::Color::from_rgb_u8(r, g, b));
    let [r, g, b] = palette.foreground;
    ts.set_term_fg(slint::Color::from_rgb_u8(r, g, b));
    let [r, g, b] = palette.cursor;
    ts.set_term_cursor_color(slint::Color::from_rgb_u8(r, g, b));
}

fn paste_to_terminal(terminal: &Terminal) -> Result<()> {
    match arboard::Clipboard::new() {
        Ok(mut ctx) => match ctx.get_text() {
            Ok(text) => {
                if !text.is_empty() {
                    terminal.write_str(&text)?;
                }
            }
            Err(e) => log::error!("Failed to get clipboard text: {e}"),
        },
        Err(e) => log::error!("Failed to access clipboard: {e}"),
    }
    Ok(())
}

fn sync_tabs_to_ui(ui_weak: &Weak<AppWindow>) {
    let Some(app) = ui_weak.upgrade() else { return };
    let state = BRIDGE.lock().unwrap();
    let Some(state) = state.as_ref() else { return };
    if let Ok(tab_manager) = state.tab_manager.try_lock() {
        let titles: Vec<TermTabModel> = tab_manager
            .get_tab_titles()
            .into_iter()
            .map(|t| TermTabModel { title: t.into() })
            .collect();
        store_terminal_state_tabs!(app).set_vec(titles);
        global_terminal_state!(app).set_active_tab_index(tab_manager.active_tab_index() as i32);
    }
}

fn update_display(state: &mut BridgeState, ui_weak: &Weak<AppWindow>) -> Result<()> {
    let mut need_resize = false;
    if let Some(app) = ui_weak.upgrade() {
        let ts = global_terminal_state!(app);
        let new_cell_w = ts.get_cell_w() as f32;
        let new_cell_h = ts.get_cell_h() as f32;

        // Skip if cell dimensions are still at their uninitialized defaults
        if new_cell_w > 2.0 && new_cell_h > 2.0 {
            if (new_cell_w - state.cell_w).abs() > 0.01 || (new_cell_h - state.cell_h).abs() > 0.01
            {
                need_resize = true;
            }
            state.cell_w = new_cell_w;
            state.cell_h = new_cell_h;

            // Read Slint's computed grid dimensions
            let cols = ts.get_term_cols() as u16;
            let rows = ts.get_term_rows() as u16;
            let cols = cols.max(2);
            let rows = rows.max(2);

            if cols != state.pty_cols || rows != state.pty_rows {
                need_resize = true;
                state.pty_cols = cols;
                state.pty_rows = rows;
            }
        }
    }

    // Update tab titles and process events for all tabs, detect exited tabs
    let mut tab_manager = state.tab_manager.lock().unwrap();
    let titles_before = tab_manager.get_tab_titles();
    tab_manager.update_tab_titles();
    let titles_after = tab_manager.get_tab_titles();
    let titles_changed = titles_before != titles_after;

    let mut exited_tabs: Vec<usize> = Vec::new();
    for (i, tab) in tab_manager.tabs.iter_mut().enumerate() {
        tab.terminal.process_events();
        if tab.terminal.has_exited() && tab.is_active {
            tab.is_active = false;
            exited_tabs.push(i);
            log::info!("Tab {} process exited", i);
        }
    }

    // Handle exited tabs
    let mut tabs_removed = false;
    for idx in exited_tabs.iter().rev() {
        if tab_manager.tab_count() <= 1 {
            // Last tab exited — respawn a new shell instead of closing
            log::info!("Last tab exited, respawning shell");
            let default_size = tab_manager.default_size();
            let palette = tab_manager.color_palette().clone();
            match Tab::with_size(None, None, Some(default_size), palette) {
                Ok(mut new_tab) => {
                    if let Err(e) = new_tab.terminal.spawn_reader_thread() {
                        log::error!("Failed to spawn reader thread: {e}");
                    }
                    // Replace the exited tab with the new one
                    tab_manager.tabs[*idx] = new_tab;
                    tab_manager.tabs[*idx].is_active = true;
                }
                Err(e) => {
                    log::error!("Failed to respawn tab: {e}");
                }
            }
        } else {
            // Remove the exited tab
            if let Err(e) = tab_manager.remove_tab(*idx) {
                log::error!("Failed to remove exited tab {idx}: {e}");
            } else {
                tabs_removed = true;
            }
        }
    }

    // Resize all tabs if needed
    if need_resize {
        let term_size = TerminalSize {
            cols: state.pty_cols,
            rows: state.pty_rows,
        };
        for tab in &mut tab_manager.tabs {
            if let Err(e) = tab.terminal.resize(term_size.cols, term_size.rows) {
                log::error!("Failed to resize terminal: {e}");
            }
        }
        tab_manager.default_size = term_size;
    }

    // If titles changed or tabs were removed, update UI
    if titles_changed || tabs_removed || !exited_tabs.is_empty() {
        let titles: Vec<TermTabModel> = tab_manager
            .get_tab_titles()
            .into_iter()
            .map(|t| TermTabModel { title: t.into() })
            .collect();
        if let Some(app) = ui_weak.upgrade() {
            crate::store_terminal_state_tabs!(app).set_vec(titles);
            global_terminal_state!(app).set_active_tab_index(tab_manager.active_tab_index() as i32);
        }
    }

    // Build spans for active tab only
    if let Some(tab) = tab_manager.active_tab_mut() {
        tab.terminal.sync_content();
        let content = tab.terminal.content();

        // Sync selection state
        state
            .span_builder
            .set_selection(state.input_handler.get_selection().cloned());

        // Build spans from terminal content
        let span_data = state.span_builder.build_spans(content);

        // Convert to Slint TermSpan
        let slint_spans: Vec<TermSpan> = span_data
            .into_iter()
            .map(|s| TermSpan {
                text: s.text.into(),
                fg: slint::Color::from_argb_u8(s.fg.a, s.fg.r, s.fg.g, s.fg.b),
                bg: slint::Color::from_argb_u8(s.bg.a, s.bg.r, s.bg.g, s.bg.b),
                bold: s.bold,
                row: s.row,
                col: s.col,
                cells: s.cells,
                cjk: s.cjk,
                underline: s.underline,
                italic: s.italic,
            })
            .collect();

        // Update the shared model in-place
        if let Some(app) = ui_weak.upgrade() {
            crate::store_terminal_state_spans!(app).set_vec(slint_spans);
        }

        // Update cursor position and scroll state
        if let Some(app) = ui_weak.upgrade() {
            let ts = global_terminal_state!(app);
            let cursor_line = content.cursor.point.line.0;
            let cursor_col = content.cursor.point.column.0;
            ts.set_cursor_row(cursor_line);
            ts.set_cursor_col(cursor_col as i32);

            let scroll_pos = content.display_offset as i32;
            let scroll_max = tab
                .terminal
                .scroll_content_size()
                .saturating_sub(content.rows) as i32;
            ts.set_scroll_position(scroll_pos);
            ts.set_scroll_max(scroll_max.max(1));
        }
    }

    Ok(())
}

/// Convert screen coordinates to grid coordinates
fn screen_to_grid(x: f32, y: f32, cell_w: f32, cell_h: f32) -> (usize, usize) {
    let col = (x / cell_w) as usize;
    let row = (y / cell_h) as usize;
    (col, row)
}
