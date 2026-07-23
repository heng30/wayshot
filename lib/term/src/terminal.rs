//! Terminal core module
//!
//! PTY management and terminal emulation using alacritty_terminal

use crate::{
    error::{Error, Result},
    shell_integration::ShellIntegration,
    theme::ColorPalette,
};
use alacritty_terminal::{
    event::{Event, EventListener},
    grid::{Dimensions, Scroll},
    index::{Column, Line, Point},
    sync::FairMutex,
    term::{Config, RenderableCursor, Term, cell::Flags},
    vte::ansi::{Color, CursorShape, NamedColor},
};
use portable_pty::{CommandBuilder, MasterPty, PtySize, native_pty_system};
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver, Sender, TryRecvError},
    },
    thread,
};
use vte::ansi;

/// Terminal size in columns and rows
#[derive(Debug, Clone, Copy, PartialEq, Eq, derivative::Derivative)]
#[derivative(Default)]
pub struct TerminalSize {
    #[derivative(Default(value = "80"))]
    pub cols: u16,
    #[derivative(Default(value = "24"))]
    pub rows: u16,
}

/// Terminal content for rendering
#[derive(Clone, derivative::Derivative)]
#[derivative(Default)]
pub struct TerminalContent {
    pub cells: Vec<TerminalCell>,

    #[derivative(Default(value = "RenderableCursor {
        shape: CursorShape::Block,
        point: Point::new(Line(0),Column(0)),
    }"))]
    pub cursor: RenderableCursor,

    #[derivative(Default(value = "80"))]
    pub cols: usize,
    #[derivative(Default(value = "24"))]
    pub rows: usize,
    #[derivative(Default(value = "0"))]
    pub display_offset: usize,
    #[derivative(Default(value = "0"))]
    pub display_start: usize, // First visible line in grid history
}

/// A terminal cell with position and content
#[derive(Clone, Debug, derivative::Derivative)]
#[derivative(Default)]
pub struct TerminalCell {
    #[derivative(Default(value = "0"))]
    pub col: usize,
    #[derivative(Default(value = "0"))]
    pub row: usize,
    #[derivative(Default(value = "' '"))]
    pub c: char,
    pub fg: Option<[u8; 3]>,
    pub bg: Option<[u8; 3]>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    /// True if this cell is the first half of a wide (CJK) character
    pub wide: bool,
    /// True if this cell is the spacer (second half) of a wide character
    pub wide_spacer: bool,
}

/// Event listener for alacritty terminal
#[derive(Clone)]
pub struct TerminalEventListener {
    event_tx: Sender<TerminalEvent>,
}

impl EventListener for TerminalEventListener {
    fn send_event(&self, event: Event) {
        _ = self.event_tx.send(TerminalEvent::from(event));
    }
}

/// Terminal events
#[derive(Debug, Clone)]
pub enum TerminalEvent {
    TitleChange(String),
    Wakeup,
    Bell,
    Exit,
    ChildExit(i32),
}

impl From<Event> for TerminalEvent {
    fn from(event: Event) -> Self {
        match event {
            Event::Title(title) => TerminalEvent::TitleChange(title),
            Event::Wakeup => TerminalEvent::Wakeup,
            Event::Bell => TerminalEvent::Bell,
            Event::Exit => TerminalEvent::Exit,
            Event::ChildExit(status) => TerminalEvent::ChildExit(status.code().unwrap_or(-1)),
            _ => TerminalEvent::Wakeup,
        }
    }
}

/// Shared PTY writer that can be cloned
#[derive(Clone)]
pub struct PtyWriter {
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
}

impl PtyWriter {
    pub fn write(&self, data: &[u8]) -> Result<()> {
        let mut writer = self.writer.lock().unwrap();
        writer
            .write_all(data)
            .map_err(|e| Error::Pty(format!("Failed to write to PTY: {}", e)))?;
        writer
            .flush()
            .map_err(|e| Error::Pty(format!("Failed to flush PTY: {}", e)))?;
        Ok(())
    }

    pub fn write_str(&self, s: &str) -> Result<()> {
        self.write(s.as_bytes())
    }
}

/// Terminal instance
pub struct Terminal {
    /// Alacritty terminal emulator
    term: Arc<FairMutex<Term<TerminalEventListener>>>,

    /// PTY writer for sending input
    pty_writer: Option<PtyWriter>,

    /// Current size
    size: TerminalSize,

    /// Event receiver
    event_rx: Receiver<TerminalEvent>,

    /// Event sender (cloned into reader threads for exit notification)
    event_tx: Sender<TerminalEvent>,

    /// Last rendered content
    last_content: TerminalContent,

    /// Working directory
    working_directory: PathBuf,

    /// Shell program
    shell: String,

    /// Terminal title
    title: String,

    /// Whether process has exited
    exited: bool,

    /// Shell integration (OSC sequences, command zones)
    shell_integration: Arc<Mutex<ShellIntegration>>,

    /// Master PTY for resizing
    master: Option<Box<dyn MasterPty + Send>>,

    /// Child process handle - kept alive to prevent zombie processes
    _child: Option<Box<dyn portable_pty::Child + Send + Sync>>,

    /// Color palette for ANSI color resolution
    color_palette: ColorPalette,
}

impl Terminal {
    pub fn new() -> Result<Self> {
        Self::with_shell(None, None, None)
    }

    /// Create a new terminal with a specific shell and optional size
    pub fn with_shell(
        shell: Option<String>,
        working_directory: Option<PathBuf>,
        size: Option<TerminalSize>,
    ) -> Result<Self> {
        Self::with_shell_and_palette(shell, working_directory, size, ColorPalette::default())
    }

    /// Create a new terminal with a specific shell, size, and color palette
    pub fn with_shell_and_palette(
        shell: Option<String>,
        working_directory: Option<PathBuf>,
        size: Option<TerminalSize>,
        color_palette: ColorPalette,
    ) -> Result<Self> {
        let working_directory = working_directory
            .or_else(|| std::env::var("HOME").ok().map(PathBuf::from))
            .unwrap_or_else(|| std::path::PathBuf::from("."));

        // Create event channel
        let (event_tx, event_rx) = mpsc::channel();
        let listener = TerminalEventListener {
            event_tx: event_tx.clone(),
        };

        // Terminal size - use provided size or larger default
        let size = size.unwrap_or(TerminalSize {
            cols: 120,
            rows: 40,
        });

        // Create terminal config
        let config = Config {
            scrolling_history: 10000,
            ..Config::default()
        };

        // Create terminal dimensions (include scrollback history)
        let dims = TerminalDimensions::new(size.cols, size.rows, config.scrolling_history);

        // Create alacritty terminal
        let term = Term::new(config, &dims, listener.clone());
        let term = Arc::new(FairMutex::new(term));

        // Create PTY
        let shell_program = shell.unwrap_or_else(Self::detect_shell);
        let default_shell_args = Self::default_shell_args();
        log::info!(
            "Using shell: {} (args: {:?})",
            shell_program,
            default_shell_args
        );
        let (pty_writer, master, child) = Self::create_pty(
            &shell_program,
            &default_shell_args,
            &working_directory,
            size,
        )?;

        Ok(Self {
            term,
            pty_writer: Some(pty_writer),
            size,
            event_rx,
            event_tx,
            last_content: TerminalContent::default(),
            working_directory,
            shell: shell_program,
            title: "Terminal".to_string(),
            exited: false,
            shell_integration: Arc::new(Mutex::new(ShellIntegration::default())),
            master: Some(master),
            _child: Some(child),
            color_palette,
        })
    }

    /// Detect the default shell
    fn detect_shell() -> String {
        #[cfg(unix)]
        {
            let shell = std::env::var("SHELL")
                .ok()
                .unwrap_or_else(|| "/bin/bash".to_string());
            log::info!("Detected shell from $SHELL: {}", shell);
            shell
        }

        #[cfg(windows)]
        {
            let shell = std::env::var("COMSPEC")
                .ok()
                .unwrap_or_else(|| "cmd.exe".to_string());
            log::info!("Detected shell from %COMSPEC%: {}", shell);
            shell
        }
    }

    /// Get default shell arguments for the platform
    fn default_shell_args() -> Vec<String> {
        #[cfg(unix)]
        {
            vec!["-l".to_string()]
        }

        #[cfg(windows)]
        {
            vec![]
        }
    }

    /// Create a PTY
    fn create_pty(
        shell: &str,
        shell_args: &[String],
        working_directory: &PathBuf,
        size: TerminalSize,
    ) -> Result<(
        PtyWriter,
        Box<dyn MasterPty + Send>,
        Box<dyn portable_pty::Child + Send + Sync>,
    )> {
        let pty_system = native_pty_system();

        let pty_size = PtySize {
            rows: size.rows,
            cols: size.cols,
            pixel_width: 0,
            pixel_height: 0,
        };

        let pty_pair = pty_system
            .openpty(pty_size)
            .map_err(|e| Error::Pty(format!("Failed to open PTY: {}", e)))?;

        // Build command with shell arguments
        let mut cmd = CommandBuilder::new(shell);
        cmd.cwd(working_directory);
        if !shell_args.is_empty() {
            cmd.args(shell_args);
        }

        // Set environment variables - important for fish shell compatibility and vi
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");

        #[cfg(unix)]
        {
            // Disable fish greeting
            cmd.env("fish_greeting", "");
            // Disable mouse reporting for vi compatibility (many vi clones don't work well with mouse)
            cmd.env("MOUSE_DISABLE", "1");
            // Ensure proper locale
            cmd.env(
                "LANG",
                std::env::var("LANG").unwrap_or_else(|_| "en_US.UTF-8".to_string()),
            );
            cmd.env(
                "LC_ALL",
                std::env::var("LC_ALL").unwrap_or_else(|_| "en_US.UTF-8".to_string()),
            );
            // Set VISUAL and EDITOR to use vi
            cmd.env("VISUAL", "vi");
            cmd.env("EDITOR", "vi");
        }

        #[cfg(windows)]
        {
            // Set console code page to UTF-8
            cmd.env("LANG", "en_US.UTF-8");
            // ConPTY on Windows 10+ may need explicit UTF-8 code page
            cmd.env("CHCP", "65001");
        }

        // Spawn the shell
        let child = pty_pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| Error::Pty(format!("Failed to spawn shell: {}", e)))?;

        // Get writer
        let writer = pty_pair
            .master
            .take_writer()
            .map_err(|e| Error::Pty(format!("Failed to get PTY writer: {}", e)))?;

        // Clone master for returning
        let master = pty_pair.master;
        let pty_writer = PtyWriter {
            writer: Arc::new(Mutex::new(writer)),
        };

        // Windows ConPTY with PSEUDOCONSOLE_INHERIT_CURSOR (used by portable-pty 0.9+)
        // sends ESC[6n (DSR — Device Status Report / cursor position query) during
        // initialization and blocks ALL child process I/O until the host replies
        // with a Cursor Position Report (ESC[row;colR) on stdin.
        //
        // Without this proactive response, ConPTY deadlocks: the child shell
        // never starts, no output is ever produced, and the terminal appears
        // blank with only a blinking cursor.
        //
        // We write the CPR immediately after PTY creation so ConPTY unblocks
        // before the reader thread even starts.
        #[cfg(windows)]
        {
            log::info!("Sending proactive CPR to ConPTY to unblock DSR deadlock");
            if let Err(e) = pty_writer.write(b"\x1b[1;1R") {
                log::warn!("Failed to send proactive CPR to ConPTY: {}", e);
            }
        }

        log::info!("PTY created successfully");
        Ok((pty_writer, master, child))
    }

    /// Spawn the PTY reader thread
    pub fn spawn_reader_thread(&mut self) -> Result<()> {
        // We need to recreate the PTY handle for the reader thread
        // This is a bit of a hack, but we need to get a new reader
        let reader = if let Some(ref mut master) = self.master {
            master
                .try_clone_reader()
                .map_err(|e| Error::Pty(format!("Failed to clone PTY reader: {}", e)))?
        } else {
            return Err(Error::Pty("No PTY master available".to_string()));
        };

        let term = self.term.clone();
        let shell_integration = self.shell_integration.clone();
        let event_tx = self.event_tx.clone();
        let pty_writer = self
            .pty_writer
            .clone()
            .ok_or_else(|| Error::Pty("No PTY writer available".to_string()))?;

        thread::spawn(move || {
            Self::pty_reader_loop(reader, term, shell_integration, event_tx, pty_writer);
        });

        log::info!("PTY reader thread spawned");
        Ok(())
    }

    /// PTY reader loop - uses non-blocking reads to avoid freezing
    fn pty_reader_loop(
        mut reader: Box<dyn Read + Send>,
        term: Arc<FairMutex<Term<TerminalEventListener>>>,
        shell_integration: Arc<Mutex<ShellIntegration>>,
        event_tx: mpsc::Sender<TerminalEvent>,
        pty_writer: PtyWriter,
    ) {
        let mut buf = [0u8; 4096]; // Smaller buffer for more responsive reads
        let mut parser = ansi::Processor::<ansi::StdSyncHandler>::new();
        let current_line: usize = 0;
        let mut consecutive_errors = 0;

        loop {
            match reader.read(&mut buf) {
                Ok(0) => {
                    log::debug!("PTY EOF");
                    break;
                }
                Ok(n) => {
                    consecutive_errors = 0; // Reset error counter on success
                    let data = &buf[..n];
                    log::trace!("Read {} bytes from PTY", n);

                    // Detect and respond to Device Attribute (DA) queries.
                    // Fish shell sends `\x1b[c` or `\x1b[0c` to query terminal
                    // capabilities. We respond with VT100 with AVO (Advanced
                    // Video Option): `\x1b[?1;2c`.
                    // Also handle Secondary DA `\x1b[>c` with VT100 response.
                    if let Some(pos) = Self::find_da_query(data) {
                        if data.get(pos + 2) == Some(&b'>') {
                            // Secondary DA: respond as VT100
                            let _ = pty_writer.write(b"\x1b[>0;0;0c");
                        } else {
                            // Primary DA: respond as VT100 with AVO
                            let _ = pty_writer.write(b"\x1b[?1;2c");
                        }
                    }

                    // Detect and respond to DSR (Device Status Report) cursor
                    // position queries.  Windows ConPTY (with
                    // PSEUDOCONSOLE_INHERIT_CURSOR) sends `\x1b[6n` during
                    // initialization and blocks until the host replies with a
                    // Cursor Position Report (`\x1b[row;colR`).  We also handle
                    // the explicit `\x1b[?6n` variant that some terminals emit.
                    if Self::find_dsr_query(data) {
                        log::debug!("DSR cursor position query detected, replying with CPR");
                        let _ = pty_writer.write(b"\x1b[1;1R");
                    }

                    // Scan for OSC sequences (shell integration)
                    if let Ok(mut si) = shell_integration.try_lock() {
                        let _sequences = si.process_pty_data(data, current_line);
                    }

                    // Lock terminal and feed bytes to VTE parser
                    // Keep lock time short - just parse and release
                    let mut term_guard = term.lock();
                    parser.advance(&mut *term_guard, data);
                    // Lock released automatically when term_guard goes out of scope
                }
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::Interrupted {
                        continue; // Interrupted, just retry
                    }
                    if e.kind() == std::io::ErrorKind::WouldBlock {
                        // Non-blocking read would block, sleep briefly
                        thread::sleep(std::time::Duration::from_millis(1));
                        continue;
                    }
                    consecutive_errors += 1;
                    log::error!(
                        "PTY read error: {} (consecutive: {})",
                        e,
                        consecutive_errors
                    );
                    if consecutive_errors > 10 {
                        log::error!("Too many consecutive errors, exiting reader thread");
                        break;
                    }
                }
            }
        }

        // Notify main thread that the shell process has exited
        let _ = event_tx.send(TerminalEvent::Exit);
        log::info!("PTY reader thread exiting");
    }

    /// Find a Device Attribute (DA) query in PTY output data.
    /// Returns the position of the ESC byte if found, None otherwise.
    /// Detects both Primary DA (`\x1b[c` or `\x1b[0c`) and
    /// Secondary DA (`\x1b[>c` or `\x1b[>0c`).
    fn find_da_query(data: &[u8]) -> Option<usize> {
        let mut i = 0;
        while i < data.len() {
            if data[i] == 0x1b {
                // Found ESC, check if this is a CSI DA query
                if i + 1 < data.len() && data[i + 1] == b'[' {
                    // CSI sequence
                    let mut j = i + 2;
                    // Skip optional parameter bytes (digits, semicolons, '>')
                    while j < data.len()
                        && (data[j].is_ascii_digit() || data[j] == b';' || data[j] == b'>')
                    {
                        j += 1;
                    }
                    // Check if the final byte is 'c' (DA query)
                    if j < data.len() && data[j] == b'c' {
                        // Only match simple DA queries: \x1b[c, \x1b[0c, \x1b[>c, \x1b[>0c
                        let param_len = j - (i + 2);
                        if param_len == 0
                            || (param_len == 1 && data[i + 2] == b'0')
                            || (param_len == 1 && data[i + 2] == b'>')
                            || (param_len == 2 && data[i + 2] == b'>' && data[i + 3] == b'0')
                        {
                            return Some(i);
                        }
                    }
                }
            }
            i += 1;
        }
        None
    }

    /// Find a DSR (Device Status Report) cursor position query in PTY output data.
    ///
    /// Windows ConPTY (when created with `PSEUDOCONSOLE_INHERIT_CURSOR`, which
    /// portable-pty 0.9.0 uses) emits `\x1b[6n` during initialization and
    /// blocks all child process I/O until the host replies with a Cursor
    /// Position Report (`\x1b[row;colR`).  This function detects both the
    /// standard form `\x1b[6n` and the `?`-prefixed variant `\x1b[?6n`.
    ///
    /// Returns `true` if a DSR query was found.
    fn find_dsr_query(data: &[u8]) -> bool {
        // Search for ESC[6n or ESC[?6n
        let patterns: &[&[u8]] = &[b"\x1b[6n", b"\x1b[?6n"];
        for pattern in patterns {
            if data.windows(pattern.len()).any(|w| w == *pattern) {
                return true;
            }
        }
        false
    }

    /// Write data to the PTY
    pub fn write(&self, data: &[u8]) -> Result<()> {
        if let Some(ref writer) = self.pty_writer {
            writer.write(data)?;
            log::trace!(
                "Wrote {} bytes to PTY: {:?}",
                data.len(),
                String::from_utf8_lossy(data)
            );
        } else {
            log::warn!("No PTY writer available");
        }
        Ok(())
    }

    /// Write a string to the PTY
    pub fn write_str(&self, s: &str) -> Result<()> {
        self.write(s.as_bytes())
    }

    /// Resize the terminal
    pub fn resize(&mut self, cols: u16, rows: u16) -> Result<()> {
        if self.size.cols == cols && self.size.rows == rows {
            return Ok(());
        }

        log::info!("Resizing terminal to {}x{}", cols, rows);
        self.size = TerminalSize { cols, rows };

        // Resize PTY
        if let Some(ref mut master) = self.master {
            let size = PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            };
            master
                .resize(size)
                .map_err(|e| Error::Pty(format!("Failed to resize PTY: {}", e)))?;
            log::debug!("PTY resized successfully");
        }

        // Resize terminal emulator
        let mut term = self.term.lock();
        // Alacritty's resize takes just the dimensions
        term.resize(TerminalDimensions::new(cols, rows, 10000));
        log::debug!("Terminal emulator resized");

        Ok(())
    }

    /// Sync terminal content for rendering
    pub fn sync_content(&mut self) {
        let term = self.term.lock();
        let content = term.renderable_content();

        // Get the display offset (scroll position)
        let display_offset = content.display_offset;

        // Use the terminal size from Terminal struct
        let cols = self.size.cols as usize;
        let rows = self.size.rows as usize;

        // Get grid info for debugging
        let grid = term.grid();
        let total_lines = grid.total_lines();
        let screen_lines = grid.screen_lines();
        let history_size = total_lines.saturating_sub(screen_lines);

        log::trace!(
            "TERM: total={}, screen={}, history={}, offset={}",
            total_lines,
            screen_lines,
            history_size,
            display_offset
        );

        // display_iter returns cells with grid coordinates:
        //   Line(-display_offset - 1) = top of viewport (first visible line)
        //   Line(-display_offset + screen_lines - 1) = bottom of viewport
        // To convert grid Line to viewport row: row = line.0 + display_offset
        let cells: Vec<TerminalCell> = content
            .display_iter
            .map(|ic| {
                // Extract colors from cell
                let inverse = ic.cell.flags.contains(Flags::INVERSE);
                let fg = if inverse { &ic.cell.bg } else { &ic.cell.fg };
                let bg = if inverse { &ic.cell.fg } else { &ic.cell.bg };

                // Convert grid line to viewport row
                let screen_row = (ic.point.line.0 + display_offset as i32) as usize;

                // For fg/bg: None means "use the default" (foreground/background),
                // Some means an explicit color was set.  This matches the meatshell
                // convention where default bg is transparent in the Slint span.
                let fg_rgb = self.ansi_color_to_rgb(fg, &ic.cell);
                let bg_rgb = self.ansi_color_to_rgb(bg, &ic.cell);
                let fg_opt = if matches!(fg, Color::Named(NamedColor::Foreground)) {
                    None
                } else {
                    Some(fg_rgb)
                };
                let bg_opt = if matches!(bg, Color::Named(NamedColor::Background)) {
                    None
                } else {
                    Some(bg_rgb)
                };

                TerminalCell {
                    col: ic.point.column.0,
                    row: screen_row,
                    c: ic.cell.c,
                    fg: fg_opt,
                    bg: bg_opt,
                    bold: ic.cell.flags.contains(Flags::BOLD),
                    italic: ic.cell.flags.contains(Flags::ITALIC),
                    underline: ic.cell.flags.contains(Flags::UNDERLINE),
                    wide: ic.cell.flags.contains(Flags::WIDE_CHAR),
                    wide_spacer: ic.cell.flags.contains(Flags::WIDE_CHAR_SPACER),
                }
            })
            .filter(|cell| cell.row < rows && cell.col < cols)
            .collect();

        // Convert cursor grid position to viewport position
        let cursor_line = content.cursor.point.line.0;
        let cursor_viewport_row = (cursor_line + display_offset as i32) as usize;
        let cursor_col = content.cursor.point.column.0;

        log::trace!(
            "CURSOR: grid_line={}, viewport_row={}, col={}, display_offset={}",
            cursor_line,
            cursor_viewport_row,
            cursor_col,
            display_offset
        );

        // Create a modified content with viewport-relative cursor position
        let mut viewport_cursor = content.cursor;
        viewport_cursor.point.line = alacritty_terminal::index::Line(cursor_viewport_row as i32);

        let display_start = total_lines.saturating_sub(screen_lines + display_offset);
        self.last_content = TerminalContent {
            cells,
            cursor: viewport_cursor,
            cols,
            rows,
            display_offset,
            display_start,
        };
    }

    /// Convert an ANSI color to RGB
    fn ansi_color_to_rgb(
        &self,
        color: &alacritty_terminal::vte::ansi::Color,
        cell: &alacritty_terminal::term::cell::Cell,
    ) -> [u8; 3] {
        use alacritty_terminal::vte::ansi::Color;

        match color {
            Color::Named(named) => self.get_named_color_rgb(*named, cell),
            Color::Spec(rgb) => [rgb.r, rgb.g, rgb.b],
            Color::Indexed(idx) => {
                if *idx < 16 {
                    // Use palette ANSI colors for indices 0-15
                    self.color_palette.ansi[*idx as usize]
                } else if *idx < 232 {
                    // 6x6x6 color cube (16-231)
                    let idx = *idx - 16;
                    let r = (idx / 36) % 6;
                    let g = (idx / 6) % 6;
                    let b = idx % 6;
                    let r = if r > 0 { r * 40 + 55 } else { 0 };
                    let g = if g > 0 { g * 40 + 55 } else { 0 };
                    let b = if b > 0 { b * 40 + 55 } else { 0 };
                    [r as u8, g as u8, b as u8]
                } else {
                    // Grayscale ramp (232-255)
                    let gray = (*idx - 232) * 10 + 8;
                    [gray, gray, gray]
                }
            }
        }
    }

    /// Get RGB values for named ANSI colors using the theme palette
    fn get_named_color_rgb(
        &self,
        named: NamedColor,
        _cell: &alacritty_terminal::term::cell::Cell,
    ) -> [u8; 3] {
        match named {
            NamedColor::Black => self.color_palette.ansi[0],
            NamedColor::Red => self.color_palette.ansi[1],
            NamedColor::Green => self.color_palette.ansi[2],
            NamedColor::Yellow => self.color_palette.ansi[3],
            NamedColor::Blue => self.color_palette.ansi[4],
            NamedColor::Magenta => self.color_palette.ansi[5],
            NamedColor::Cyan => self.color_palette.ansi[6],
            NamedColor::White => self.color_palette.ansi[7],
            NamedColor::BrightBlack => self.color_palette.ansi[8],
            NamedColor::BrightRed => self.color_palette.ansi[9],
            NamedColor::BrightGreen => self.color_palette.ansi[10],
            NamedColor::BrightYellow => self.color_palette.ansi[11],
            NamedColor::BrightBlue => self.color_palette.ansi[12],
            NamedColor::BrightMagenta => self.color_palette.ansi[13],
            NamedColor::BrightCyan => self.color_palette.ansi[14],
            NamedColor::BrightWhite => self.color_palette.ansi[15],
            NamedColor::Foreground => self.color_palette.foreground,
            NamedColor::Background => self.color_palette.background,
            NamedColor::Cursor => self.color_palette.cursor,
            _ => self.color_palette.foreground,
        }
    }

    pub fn content(&self) -> &TerminalContent {
        &self.last_content
    }

    pub fn size(&self) -> TerminalSize {
        self.size
    }

    pub fn poll_event(&self) -> Option<TerminalEvent> {
        match self.event_rx.try_recv() {
            Ok(event) => Some(event),
            Err(TryRecvError::Disconnected) => None,
            Err(TryRecvError::Empty) => None,
        }
    }

    pub fn has_exited(&self) -> bool {
        if self.exited {
            return true;
        }
        // Check if child process has exited
        // Note: We can't use try_wait here because it requires mutable access
        // The exited flag will be set when we receive ChildExit event
        false
    }

    pub fn working_directory(&self) -> &std::path::PathBuf {
        &self.working_directory
    }

    pub fn shell_integration(&self) -> Arc<Mutex<ShellIntegration>> {
        self.shell_integration.clone()
    }

    pub fn get_title(&self) -> &str {
        &self.title
    }

    /// Process pending events and update terminal state
    pub fn process_events(&mut self) {
        while let Some(event) = self.poll_event() {
            match event {
                TerminalEvent::TitleChange(title) => self.title = title,
                TerminalEvent::Exit => {
                    self.exited = true;
                    log::info!("Terminal process exited");
                }
                TerminalEvent::ChildExit(code) => {
                    self.exited = true;
                    log::info!("Terminal process exited with code: {}", code);
                }
                TerminalEvent::Bell => {
                    // TODO: Visual bell indicator
                    log::trace!("Terminal bell");
                }
                TerminalEvent::Wakeup => {
                    // Just a wakeup signal, no action needed
                }
            }
        }
    }

    /// Get selected text from terminal content
    pub fn get_selected_text(&self, selection: &crate::input::SelectionState) -> String {
        let (start_row, start_col, end_row, end_col) = selection.normalized();

        // Build a per-row map of column -> character
        let mut row_chars: BTreeMap<usize, Vec<(usize, char)>> = BTreeMap::new();

        for cell in &self.last_content.cells {
            // Check if cell is within selection
            let in_selection = if start_row == end_row {
                cell.row == start_row && cell.col >= start_col && cell.col <= end_col
            } else if cell.row == start_row {
                cell.col >= start_col
            } else if cell.row == end_row {
                cell.col <= end_col
            } else {
                cell.row > start_row && cell.row < end_row
            };

            if in_selection {
                row_chars
                    .entry(cell.row)
                    .or_default()
                    .push((cell.col, cell.c));
            }
        }

        // Assemble text row by row, trimming trailing spaces
        let mut text = String::new();
        let mut first = true;
        for (_row, chars) in &mut row_chars {
            if !first {
                text.push('\n');
            }
            first = false;

            // Sort by column
            chars.sort_by_key(|(col, _)| *col);

            // Build the line, then strip trailing spaces
            let line: String = chars.iter().map(|(_, c)| c).collect();
            text.push_str(line.trim_end());
        }

        text
    }

    /// Scroll up by n lines (view older content)
    pub fn scroll_up(&mut self, lines: usize) {
        use alacritty_terminal::grid::Scroll;
        let mut term = self.term.lock();
        let current = term.grid().display_offset();
        let history = term.grid().history_size();
        log::trace!(
            "scroll_up: current_offset={}, history={}, lines={}",
            current,
            history,
            lines
        );
        if current >= history {
            return; // Already at top of history
        }
        term.scroll_display(Scroll::Delta(lines as i32));
        log::trace!(
            "scroll_up result: new_offset={}",
            term.grid().display_offset()
        );
    }

    /// Scroll down by n lines (view newer content)
    pub fn scroll_down(&mut self, lines: usize) {
        use alacritty_terminal::grid::Scroll;
        let mut term = self.term.lock();
        let current = term.grid().display_offset();
        log::trace!("scroll_down: current_offset={}, lines={}", current, lines);
        if current == 0 {
            return; // Already at bottom
        }
        term.scroll_display(Scroll::Delta(-(lines as i32)));
        log::trace!(
            "scroll_down result: new_offset={}",
            term.grid().display_offset()
        );
    }

    pub fn scroll_to_top(&mut self) {
        let mut term = self.term.lock();
        term.scroll_display(Scroll::Top);
    }

    pub fn scroll_to_bottom(&mut self) {
        let mut term = self.term.lock();
        term.scroll_display(Scroll::Bottom);
    }

    /// Scroll to a specific position (for scrollbar)
    pub fn scroll_to_position(&mut self, position: usize) {
        let mut term = self.term.lock();
        let current = term.grid().display_offset();
        let delta = position as i32 - current as i32;
        if delta != 0 {
            term.scroll_display(Scroll::Delta(delta));
        }
    }

    /// Get the total scrollable content size (history + screen)
    pub fn scroll_content_size(&self) -> usize {
        let term = self.term.lock();
        term.grid().total_lines()
    }

    /// Get current scroll position
    pub fn scroll_offset(&self) -> usize {
        let term = self.term.lock();
        term.grid().display_offset()
    }

    /// Get scrollback length (total lines in history)
    pub fn scrollback_length(&self) -> usize {
        let term = self.term.lock();
        // Total lines includes scrollback + screen
        term.grid().total_lines()
    }

    pub fn set_color_palette(&mut self, palette: ColorPalette) {
        self.color_palette = palette;
    }

    pub fn is_at_bottom(&self) -> bool {
        self.last_content.display_offset == 0
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        log::info!("Terminal dropped: {}", self.shell);
    }
}

/// Terminal dimensions for alacritty
struct TerminalDimensions {
    cols: u16,
    rows: u16,
    /// Total lines including scrollback history
    total_lines: usize,
}

impl TerminalDimensions {
    fn new(cols: u16, rows: u16, history: usize) -> Self {
        Self {
            cols,
            rows,
            total_lines: rows as usize + history,
        }
    }
}

impl Dimensions for TerminalDimensions {
    fn total_lines(&self) -> usize {
        self.total_lines
    }

    fn screen_lines(&self) -> usize {
        self.rows as usize
    }

    fn columns(&self) -> usize {
        self.cols as usize
    }
}
