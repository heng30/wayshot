use slint::{ComponentHandle, Model, Timer, VecModel};
use std::{
    rc::Rc,
    sync::{Arc, Mutex},
    time::Duration,
};
use term::{
    input::{InputHandler, KeyModifiers, MouseButton},
    render::SpanBuilder,
    tabs::TabManager,
    terminal::TerminalSize,
    theme, {Error, Result},
};

slint::slint! {
    import { TerminalApp } from "ui/app.slint";
    export { TerminalApp }
}

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    log::info!("Starting Slint Terminal...");

    let theme_name = parse_theme_arg();
    SlintBridge::new(&theme_name)?.run()?;

    Ok(())
}

/// Parse --theme <name> from command-line arguments.
/// Returns "dark" if not specified.
fn parse_theme_arg() -> String {
    let args: Vec<String> = std::env::args().collect();
    let mut iter = args.iter().peekable();
    let _ = iter.next(); // skip program name
    while let Some(arg) = iter.next() {
        if arg == "--theme" {
            if let Some(name) = iter.next() {
                return name.to_string();
            }
            eprintln!("warning: --theme requires a value (dark|light), defaulting to dark");
        } else if let Some(name) = arg.strip_prefix("--theme=") {
            if !name.is_empty() {
                return name.to_string();
            }
            eprintln!("warning: --theme= requires a value (dark|light), defaulting to dark");
        }
    }
    "dark".to_string()
}

/// Shared bridge state that can be accessed from timer callbacks
struct BridgeState {
    span_builder: SpanBuilder,
    tab_manager: Arc<Mutex<TabManager>>,
    input_handler: InputHandler,
    /// Cached cell dimensions (read from Slint probe on each tick)
    cell_w: f32,
    cell_h: f32,
    /// Current font size (tracked for zoom)
    font_size: f32,
    /// Shared spans model (updated in-place each frame)
    spans_model: Rc<VecModel<TermSpan>>,
    /// Shared tabs model (updated in-place when titles change)
    tabs_model: Rc<VecModel<TabModel>>,
}

pub struct SlintBridge {
    app: TerminalApp,
    state: Arc<Mutex<BridgeState>>,
}

impl SlintBridge {
    pub fn new(theme_name: &str) -> Result<Self> {
        log::info!("Creating Slint Terminal...");

        let app = TerminalApp::new().map_err(|e| Error::Slint(e.to_string()))?;
        let mut span_builder = SpanBuilder::new();

        // Load and apply theme
        let config = term::config::Config::default();
        let theme = theme::Theme::from_name(theme_name);
        let color_palette = theme.colors.clone();

        // Apply theme colors to SpanBuilder
        span_builder.set_default_bg(theme.colors.background);
        span_builder.set_default_fg(theme.colors.foreground);
        span_builder.set_cursor_color(theme.colors.cursor);
        span_builder.set_selection_bg(theme.colors.selection);

        // Apply theme colors directly to Slint app
        let bg = theme.colors.background;
        let fg = theme.colors.foreground;
        let cursor = theme.colors.cursor;
        app.set_term_bg(slint::Color::from_rgb_u8(bg[0], bg[1], bg[2]));
        app.set_term_fg(slint::Color::from_rgb_u8(fg[0], fg[1], fg[2]));
        app.set_cursor_color(slint::Color::from_rgb_u8(cursor[0], cursor[1], cursor[2]));

        // Apply font settings
        let font_family: slint::SharedString = "Noto Sans Mono CJK SC".into();
        app.set_term_font_family(font_family);
        app.set_term_font_size(16.0);

        log::info!("Loaded theme: {}", theme.name);

        // Read initial cell dimensions from the Slint probe
        // (The probe needs a layout pass first, so use defaults initially
        // and read the real values on the first timer tick)
        let cell_w = 8.0; // will be updated from Slint probe
        let cell_h = 17.0;
        let font_size = 16.0;

        // Initial terminal size — will be corrected on the first timer tick
        // when Slint computes the actual term-area size. Use conservative
        // defaults so the PTY starts at a reasonable size.
        let terminal_size = TerminalSize {
            cols: 120,
            rows: 35,
        };

        log::info!(
            "Initial terminal size: {}x{} (font=16.0, font_family=Noto Sans Mono CJK SC)",
            terminal_size.cols,
            terminal_size.rows,
        );

        let mut tab_manager = TabManager::with_size_and_palette(terminal_size, color_palette)?;

        // Spawn PTY reader threads for all tabs
        for tab in &mut tab_manager.tabs {
            tab.terminal.spawn_reader_thread()?;
        }
        log::info!(
            "PTY reader threads spawned for {} tab(s)",
            tab_manager.tabs.len()
        );

        let tab_manager = Arc::new(Mutex::new(tab_manager));
        let input_handler = InputHandler::new();

        // Create the shared spans model
        let spans_model: Rc<VecModel<TermSpan>> = Rc::new(VecModel::default());

        // Create the shared tabs model
        let tabs_model: Rc<VecModel<TabModel>> = Rc::new(VecModel::default());

        let state = BridgeState {
            span_builder,
            tab_manager: tab_manager.clone(),
            input_handler,
            cell_w,
            cell_h,
            font_size,
            spans_model: spans_model.clone(),
            tabs_model: tabs_model.clone(),
        };

        let mut bridge = Self {
            app,
            state: Arc::new(Mutex::new(state)),
        };

        // Set the models on the app (once, then update in-place)
        bridge.app.set_spans(spans_model.into());
        bridge.app.set_tabs(tabs_model.into());

        // Set up all callbacks before showing
        bridge.setup_tab_callbacks();
        bridge.setup_keyboard_callbacks();
        bridge.setup_mouse_callbacks();
        bridge.setup_clipboard_callbacks();
        bridge.setup_scrollbar_callbacks();
        bridge.setup_font_zoom_callbacks();
        bridge.setup_ime_callbacks();
        bridge.sync_tabs_to_ui();

        Ok(bridge)
    }

    pub fn run(self) -> Result<()> {
        log::info!("Starting Slint Terminal...");

        // Get weak reference for callbacks
        let app_weak = self.app.as_weak();

        // Clone state for the timer callback
        let state_for_timer = self.state.clone();
        let app_weak_for_timer = app_weak.clone();

        // Set up a timer to update the display periodically (60 FPS)
        let timer = Timer::default();
        timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(16),
            move || {
                if let Ok(mut state) = state_for_timer.try_lock() {
                    match Self::update_display_internal(&mut state, app_weak_for_timer.clone()) {
                        Ok(_) => {}
                        Err(e) => log::trace!("Display update error: {}", e),
                    }
                }

                // Trigger window redraw
                if let Some(app) = app_weak_for_timer.upgrade() {
                    app.window().request_redraw();
                }
            },
        );

        // Show the window and run
        log::info!("Showing window...");
        self.app.show().map_err(|e| Error::Slint(e.to_string()))?;

        log::info!("Window shown");

        self.app.run().map_err(|e| Error::Slint(e.to_string()))?;

        log::info!("Terminal closed");
        Ok(())
    }

    fn setup_tab_callbacks(&mut self) {
        // New tab callback
        let state_for_tabs = self.state.clone();
        let app_weak = self.app.as_weak();
        self.app.on_new_tab(move || {
            log::info!("New tab button clicked");
            if let Ok(state) = state_for_tabs.try_lock()
                && let Ok(mut tab_manager) = state.tab_manager.try_lock()
            {
                match tab_manager.add_new_tab() {
                    Ok(()) => {
                        // Spawn reader thread for the new tab
                        if let Some(tab) = tab_manager.tabs.last_mut() {
                            if let Err(e) = tab.terminal.spawn_reader_thread() {
                                log::error!("Failed to spawn reader thread: {}", e);
                            }
                        }
                        let tab_count = tab_manager.tab_count();
                        log::info!("New tab created, total tabs: {}", tab_count);

                        // Switch to the new tab
                        if let Err(e) = tab_manager.set_active_tab(tab_count - 1) {
                            log::error!("Failed to switch to new tab: {}", e);
                        }

                        // Update UI tabs
                        let titles: Vec<TabModel> = tab_manager
                            .get_tab_titles()
                            .into_iter()
                            .map(|t| TabModel { title: t.into() })
                            .collect();
                        state.tabs_model.set_vec(titles);
                        if let Some(app) = app_weak.upgrade() {
                            app.set_active_tab_index((tab_count - 1) as i32);
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to create new tab: {}", e);
                    }
                }
            }
        });

        // Close tab callback
        let state_for_tabs = self.state.clone();
        self.app.on_close_tab(move |index| {
            log::info!("Close tab button clicked for tab {}", index);
            if let Ok(state) = state_for_tabs.try_lock()
                && let Ok(mut tab_manager) = state.tab_manager.try_lock()
            {
                let index = index as usize;
                if let Err(e) = tab_manager.remove_tab(index) {
                    log::error!("Failed to close tab {}: {}", index, e);
                } else {
                    let tab_count = tab_manager.tab_count();
                    log::info!("Tab {} closed, remaining tabs: {}", index, tab_count);

                    // Update UI tabs
                    let titles: Vec<TabModel> = tab_manager
                        .get_tab_titles()
                        .into_iter()
                        .map(|t| TabModel { title: t.into() })
                        .collect();
                    state.tabs_model.set_vec(titles);
                }
            }
        });

        // Tab changed callback
        let state_for_tabs = self.state.clone();
        let app_weak = self.app.as_weak();

        self.app.on_tab_changed(move |index| {
            log::info!("Tab changed to {}", index);
            if let Ok(state) = state_for_tabs.try_lock()
                && let Ok(mut tab_manager) = state.tab_manager.try_lock()
            {
                let index = index as usize;
                if let Err(e) = tab_manager.set_active_tab(index) {
                    log::error!("Failed to switch to tab {}: {}", index, e);
                } else {
                    log::info!("Switched to tab {}", index);
                    // Update active tab index in UI
                    if let Some(app) = app_weak.upgrade() {
                        app.set_active_tab_index(index as i32);
                    }
                }
            }
        });
    }

    fn setup_keyboard_callbacks(&mut self) {
        // Key pressed event
        let state_for_input = self.state.clone();
        let app_weak = self.app.as_weak();
        self.app
            .on_key_pressed_event(move |text, shift, alt, ctrl| {
                log::trace!(
                    "Key pressed: text='{}', shift={}, alt={}, ctrl={}",
                    text,
                    shift,
                    alt,
                    ctrl
                );

                // Handle font zoom shortcuts: Ctrl+= zoom in, Ctrl+- zoom out, Ctrl+0 reset
                if ctrl && !alt {
                    let is_zoom_in =
                        text == "=" || text == "+" || text.chars().next() == Some('\u{001D}');
                    let is_zoom_out = text == "-" || text.chars().next() == Some('\u{000D}');
                    let is_zoom_reset = text == "0" || text.chars().next() == Some('\u{0010}');

                    let zoom_result = if is_zoom_in {
                        if let Ok(mut state) = state_for_input.try_lock() {
                            let current = state.font_size;
                            let new_size = (current + 1.0).min(32.0);
                            Self::apply_font_zoom(&mut state, new_size, app_weak.clone())
                        } else {
                            None
                        }
                    } else if is_zoom_out {
                        if let Ok(mut state) = state_for_input.try_lock() {
                            let current = state.font_size;
                            let new_size = (current - 1.0).max(6.0);
                            Self::apply_font_zoom(&mut state, new_size, app_weak.clone())
                        } else {
                            None
                        }
                    } else if is_zoom_reset {
                        if let Ok(mut state) = state_for_input.try_lock() {
                            Self::apply_font_zoom(&mut state, 14.0, app_weak.clone())
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                    // If we handled a zoom shortcut, don't send to terminal
                    if zoom_result.is_some() {
                        return;
                    }
                }

                // Handle Ctrl+Shift+C (copy) and Ctrl+Shift+V (paste)
                if ctrl && shift && !alt {
                    if text == "c" || text == "C" {
                        if let Ok(state) = state_for_input.try_lock()
                            && let Ok(tab_manager) = state.tab_manager.try_lock()
                            && let Some(tab) = tab_manager.active_tab()
                            && let Some(text) = state.input_handler.get_selected_text(&tab.terminal)
                            && !text.is_empty()
                        {
                            match arboard::Clipboard::new() {
                                Ok(mut ctx) => {
                                    if let Err(e) = ctx.set_text(&text) {
                                        log::error!("Failed to copy: {}", e);
                                    } else {
                                        log::info!("Ctrl+Shift+C: copied {} chars", text.len());
                                    }
                                }
                                Err(e) => log::error!("Failed to access clipboard: {}", e),
                            }
                        }
                        return;
                    } else if text == "v" || text == "V" {
                        if let Ok(state) = state_for_input.try_lock()
                            && let Ok(mut tab_manager) = state.tab_manager.try_lock()
                            && let Some(tab) = tab_manager.active_tab_mut()
                        {
                            _ = Self::paste_to_terminal(&tab.terminal);
                        }
                        return;
                    }
                }

                let modifiers = KeyModifiers {
                    shift,
                    alt,
                    ctrl,
                    meta: false,
                };

                if let Ok(state) = state_for_input.try_lock()
                    && let Ok(mut tab_manager) = state.tab_manager.try_lock()
                    && let Some(tab) = tab_manager.active_tab_mut()
                    && let Err(e) =
                        state
                            .input_handler
                            .handle_key(&tab.terminal, &text, &text, modifiers)
                {
                    log::error!("Failed to handle key: {}", e);
                }
            });
    }

    fn setup_ime_callbacks(&mut self) {
        // IME commit callback
        let state_for_ime = self.state.clone();
        self.app.on_ime_commit(move |text| {
            log::debug!("IME commit: text='{}'", text);

            if text.is_empty() {
                return;
            }

            if let Ok(state) = state_for_ime.try_lock()
                && let Ok(mut tab_manager) = state.tab_manager.try_lock()
                && let Some(tab) = tab_manager.active_tab_mut()
                && let Err(e) = tab.terminal.write_str(&text)
            {
                log::error!("Failed to write IME text to terminal: {}", e);
            }
        });
    }

    fn setup_mouse_callbacks(&mut self) {
        // Mouse pressed
        let state_for_mouse = self.state.clone();
        self.app.on_mouse_pressed(move |button, x, y| {
            log::trace!("Mouse pressed: button={}, x={}, y={}", button, x, y);

            let button = match button {
                0 => MouseButton::Left,
                1 => MouseButton::Middle,
                2 => MouseButton::Right,
                _ => MouseButton::Left,
            };

            if let Ok(mut state) = state_for_mouse.try_lock() {
                let (col, row) = screen_to_grid(x as f32, y as f32, state.cell_w, state.cell_h);

                if let Err(e) = state.input_handler.handle_mouse_press(col, row, button) {
                    log::error!("Failed to handle mouse press: {}", e);
                }

                // Handle middle click paste
                if button == MouseButton::Middle
                    && let Ok(mut tab_manager) = state.tab_manager.try_lock()
                    && let Some(tab) = tab_manager.active_tab_mut()
                {
                    let _ = Self::paste_to_terminal(&tab.terminal);
                }
            }
        });

        // Mouse released
        let state_for_mouse = self.state.clone();
        self.app.on_mouse_released(move |button, x, y| {
            log::trace!("Mouse released: button={}, x={}, y={}", button, x, y);

            let button = match button {
                0 => MouseButton::Left,
                1 => MouseButton::Middle,
                2 => MouseButton::Right,
                _ => MouseButton::Left,
            };

            if let Ok(mut state) = state_for_mouse.try_lock() {
                let (col, row) = screen_to_grid(x as f32, y as f32, state.cell_w, state.cell_h);

                if let Err(e) = state.input_handler.handle_mouse_release(col, row, button) {
                    log::error!("Failed to handle mouse release: {}", e);
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
                            && let Some(text) = state.input_handler.get_selected_text(&tab.terminal)
                            && !text.is_empty()
                        {
                            match arboard::Clipboard::new() {
                                Ok(mut ctx) => {
                                    if let Err(e) = ctx.set_text(&text) {
                                        log::error!("Failed to copy to clipboard: {}", e);
                                    } else {
                                        log::info!("Auto-copied {} chars to clipboard", text.len());
                                    }
                                }
                                Err(e) => log::error!("Failed to access clipboard: {}", e),
                            }
                        }
                    } else {
                        state.input_handler.clear_selection();
                    }
                }
            }
        });

        // Mouse moved
        let state_for_mouse = self.state.clone();
        self.app.on_mouse_moved(move |x, y| {
            if let Ok(mut state) = state_for_mouse.try_lock() {
                let (col, row) = screen_to_grid(x as f32, y as f32, state.cell_w, state.cell_h);

                if let Err(e) = state.input_handler.handle_mouse_move(col, row) {
                    log::error!("Failed to handle mouse move: {}", e);
                }
            }
        });

        // Mouse wheel
        let state_for_mouse = self.state.clone();
        self.app.on_mouse_wheel(move |delta| {
            if let Ok(state) = state_for_mouse.try_lock()
                && let Ok(mut tab_manager) = state.tab_manager.try_lock()
                && let Some(tab) = tab_manager.active_tab_mut()
            {
                let lines = (delta.abs() / 60.0 * 5.0).ceil() as usize;
                let lines = lines.max(1).min(50);
                if delta > 0.0 {
                    tab.terminal.scroll_up(lines);
                } else {
                    tab.terminal.scroll_down(lines);
                }
            }
        });
    }

    fn setup_scrollbar_callbacks(&mut self) {
        // Scroll to position (from scrollbar drag)
        let state_for_scroll = self.state.clone();
        self.app.on_scroll_to(move |position| {
            if let Ok(state) = state_for_scroll.try_lock()
                && let Ok(mut tab_manager) = state.tab_manager.try_lock()
                && let Some(tab) = tab_manager.active_tab_mut()
            {
                tab.terminal.scroll_to_position(position as usize);
            }
        });
    }

    /// Apply font size change and resize terminal/PTY to match
    fn apply_font_zoom(
        state: &mut BridgeState,
        new_font_size: f32,
        app_weak: slint::Weak<TerminalApp>,
    ) -> Option<f32> {
        state.font_size = new_font_size;

        // Update font size on the Slint app. The probe reactively
        // recalculates cell-w/cell-h, and the next timer tick's
        // update_display_internal() will detect the cell dimension
        // change and resize the PTY accordingly.  We don't resize
        // here because Slint may need a layout pass before the probe
        // reports the new cell dimensions.
        if let Some(app) = app_weak.upgrade() {
            app.set_term_font_size(new_font_size);
        }

        Some(new_font_size)
    }

    fn setup_font_zoom_callbacks(&mut self) {
        // Font zoom in
        let state_for_zoom = self.state.clone();
        let app_weak = self.app.as_weak();
        self.app.on_font_zoom_in(move || {
            if let Ok(mut state) = state_for_zoom.try_lock() {
                let current = state.font_size;
                let new_size = (current + 1.0).min(32.0);
                Self::apply_font_zoom(&mut state, new_size, app_weak.clone());
            }
        });

        // Font zoom out
        let state_for_zoom = self.state.clone();
        let app_weak = self.app.as_weak();
        self.app.on_font_zoom_out(move || {
            if let Ok(mut state) = state_for_zoom.try_lock() {
                let current = state.font_size;
                let new_size = (current - 1.0).max(6.0);
                Self::apply_font_zoom(&mut state, new_size, app_weak.clone());
            }
        });

        // Font zoom reset
        let state_for_zoom = self.state.clone();
        let app_weak = self.app.as_weak();
        self.app.on_font_zoom_reset(move || {
            if let Ok(mut state) = state_for_zoom.try_lock() {
                Self::apply_font_zoom(&mut state, 14.0, app_weak.clone());
            }
        });
    }

    fn setup_clipboard_callbacks(&mut self) {
        let state_for_clipboard = self.state.clone();
        self.app.on_copy_request(move || {
            if let Ok(state) = state_for_clipboard.try_lock()
                && let Ok(tab_manager) = state.tab_manager.try_lock()
                && let Some(tab) = tab_manager.active_tab()
                && let Some(text) = state.input_handler.get_selected_text(&tab.terminal)
                && !text.is_empty()
            {
                match arboard::Clipboard::new() {
                    Ok(mut ctx) => {
                        if let Err(e) = ctx.set_text(&text) {
                            log::error!("Failed to copy to clipboard: {}", e);
                        } else {
                            log::info!("Copied {} chars to clipboard", text.len());
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to access clipboard: {}", e);
                    }
                }
            }
        });

        // Paste request
        let state_for_clipboard = self.state.clone();
        self.app.on_paste_request(move || {
            if let Ok(state) = state_for_clipboard.try_lock()
                && let Ok(mut tab_manager) = state.tab_manager.try_lock()
                && let Some(tab) = tab_manager.active_tab_mut()
            {
                _ = Self::paste_to_terminal(&tab.terminal);
            }
        });
    }

    fn paste_to_terminal(terminal: &term::terminal::Terminal) -> Result<()> {
        match arboard::Clipboard::new() {
            Ok(mut ctx) => match ctx.get_text() {
                Ok(text) => {
                    if !text.is_empty() {
                        log::info!("Pasting {} chars from clipboard", text.len());
                        terminal.write_str(&text)?;
                    }
                }
                Err(e) => log::error!("Failed to get clipboard text: {}", e),
            },
            Err(e) => log::error!("Failed to access clipboard: {}", e),
        }
        Ok(())
    }

    /// Sync tabs from TabManager to UI
    fn sync_tabs_to_ui(&mut self) {
        if let Ok(state) = self.state.try_lock()
            && let Ok(tab_manager) = state.tab_manager.try_lock()
        {
            let titles: Vec<TabModel> = tab_manager
                .get_tab_titles()
                .into_iter()
                .map(|t| TabModel { title: t.into() })
                .collect();
            state.tabs_model.set_vec(titles);
            self.app
                .set_active_tab_index(tab_manager.active_tab_index() as i32);
            log::info!("Synced {} tabs to UI", tab_manager.tab_count());
        }
    }

    /// Internal display update (called from timer callback)
    fn update_display_internal(
        state: &mut BridgeState,
        app_weak: slint::Weak<TerminalApp>,
    ) -> Result<()> {
        // Update cell dimensions from Slint probe (cheap read)
        let mut need_resize = false;
        if let Some(app) = app_weak.upgrade() {
            let new_cell_w = app.get_cell_w() as f32;
            let new_cell_h = app.get_cell_h() as f32;
            // Detect when cell dimensions change (e.g. after first layout pass
            // replaces the placeholder values, or after font zoom)
            if (new_cell_w - state.cell_w).abs() > 0.01 || (new_cell_h - state.cell_h).abs() > 0.01
            {
                log::info!(
                    "Cell dimensions changed: {:.2}x{:.2} → {:.2}x{:.2}",
                    state.cell_w,
                    state.cell_h,
                    new_cell_w,
                    new_cell_h
                );
                need_resize = true;
            }
            state.cell_w = new_cell_w;
            state.cell_h = new_cell_h;
        }

        // Update tab titles and process events
        let mut tab_manager = state.tab_manager.lock().unwrap();
        let titles_before = tab_manager.get_tab_titles();
        tab_manager.update_tab_titles();
        let titles_after = tab_manager.get_tab_titles();
        let titles_changed = titles_before != titles_after;

        // Process events for all tabs, detect exited tabs
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
        for idx in exited_tabs.iter().rev() {
            if tab_manager.tab_count() <= 1 {
                // Last tab exited - close the window
                drop(tab_manager);
                if let Some(app) = app_weak.upgrade() {
                    app.window().hide().ok();
                }
                return Ok(());
            }
            // Remove the exited tab
            tab_manager.remove_tab(*idx)?;
        }

        // If tabs changed or titles changed, update UI
        if !exited_tabs.is_empty() || titles_changed {
            let titles: Vec<TabModel> = tab_manager
                .get_tab_titles()
                .into_iter()
                .map(|t| TabModel { title: t.into() })
                .collect();
            state.tabs_model.set_vec(titles);
            if let Some(app) = app_weak.upgrade() {
                app.set_active_tab_index(tab_manager.active_tab_index() as i32);
            }
        }

        // Resize PTY if cell dimensions changed (e.g. after initial layout pass)
        if need_resize && let Some(app) = app_weak.upgrade() {
            // Use Slint's calculated term-cols/term-rows which account for
            // the actual term-area size (after tab bar, scrollbar, etc.)
            let cols = app.get_term_cols() as u16;
            let rows = app.get_term_rows() as u16;
            let term_size = TerminalSize {
                cols: cols.max(40),
                rows: rows.max(10),
            };

            for tab in &mut tab_manager.tabs {
                if let Err(e) = tab.terminal.resize(term_size.cols, term_size.rows) {
                    log::error!("Failed to resize terminal: {}", e);
                }
            }
            tab_manager.default_size = term_size;

            log::info!(
                "Resized PTY after cell change: {}x{} (cell {:.2}x{:.2})",
                term_size.cols,
                term_size.rows,
                state.cell_w,
                state.cell_h
            );
        }

        // Build spans for active tab
        if let Some(tab) = tab_manager.active_tab_mut() {
            tab.terminal.sync_content();
            let content = tab.terminal.content();

            // Sync selection state from input handler to span builder
            state
                .span_builder
                .set_selection(state.input_handler.get_selection().cloned());

            // Build spans from terminal content
            let span_data = state.span_builder.build_spans(content);

            // Convert to Slint TermSpan and update the shared model in-place
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

            // Update the shared model in-place (more efficient than replacing
            // the entire model, and avoids potential issues with Slint's
            // reactivity not picking up model changes)
            state.spans_model.set_vec(slint_spans);

            log::trace!(
                "Updated spans model: {} spans, cell_w={:.2}, cell_h={:.2}",
                state.spans_model.row_count(),
                state.cell_w,
                state.cell_h
            );

            // Update the Slint UI
            if let Some(app) = app_weak.upgrade() {
                // Update cursor position
                let cursor_line = content.cursor.point.line.0;
                let cursor_col = content.cursor.point.column.0;
                app.set_cursor_row(cursor_line);
                app.set_cursor_col(cursor_col as i32);

                // Update scrollbar position
                let scroll_pos = content.display_offset as i32;
                let scroll_max = tab
                    .terminal
                    .scroll_content_size()
                    .saturating_sub(content.rows) as i32;
                app.set_scroll_position(scroll_pos);
                app.set_scroll_max(scroll_max.max(1));
            }
        }

        Ok(())
    }
}

/// Convert screen coordinates to grid coordinates
fn screen_to_grid(x: f32, y: f32, cell_w: f32, cell_h: f32) -> (usize, usize) {
    let col = (x / cell_w) as usize;
    let row = (y / cell_h) as usize;
    (col, row)
}
