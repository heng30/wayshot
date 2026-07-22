// Input handling module
//
// This module handles keyboard and mouse input from Slint
// and converts it to terminal input sequences

use crate::{Result, terminal::Terminal};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

/// Input handler for converting UI events to terminal input sequences
pub struct InputHandler {
    /// Current selection state
    pub selection: Option<SelectionState>,
    /// Whether selection is in progress
    selecting: Arc<AtomicBool>,
    /// Last mouse position for drag
    last_mouse_pos: Option<(usize, usize)>,
}

/// Selection state for mouse-based text selection
#[derive(Debug, Clone)]
pub struct SelectionState {
    pub start_col: usize,
    pub start_row: usize,
    pub end_col: usize,
    pub end_row: usize,
}

impl SelectionState {
    pub fn new(start_col: usize, start_row: usize, end_col: usize, end_row: usize) -> Self {
        Self {
            start_col,
            start_row,
            end_col,
            end_row,
        }
    }

    /// Normalize selection coordinates (ensure start <= end)
    pub fn normalized(&self) -> (usize, usize, usize, usize) {
        let start = (self.start_row, self.start_col);
        let end = (self.end_row, self.end_col);

        if start <= end {
            (self.start_row, self.start_col, self.end_row, self.end_col)
        } else {
            (self.end_row, self.end_col, self.start_row, self.start_col)
        }
    }
}

impl InputHandler {
    pub fn new() -> Self {
        Self {
            selection: None,
            selecting: Arc::new(AtomicBool::new(false)),
            last_mouse_pos: None,
        }
    }

    /// Handle a key event from Slint and send to terminal
    pub fn handle_key(
        &self,
        terminal: &Terminal,
        text: &str,
        key_code: &str,
        modifiers: KeyModifiers,
    ) -> Result<()> {
        // Convert key_code to the format expected by handle_key_event
        let key = if text.is_empty() {
            // No text, use key_code (e.g., "KeyA" -> "a" for special key handling)
            key_code.replace("Key", "").to_lowercase()
        } else {
            // Text is provided, use it directly (preserves shift+letter case)
            text.to_string()
        };

        // Get the input bytes
        let bytes = self.handle_key_event(&key, modifiers)?;

        // Send to terminal
        if !bytes.is_empty() {
            terminal.write(&bytes)?;
        }

        Ok(())
    }

    /// Handle text input (for regular character input)
    pub fn handle_text_input(&self, terminal: &Terminal, text: &str) -> Result<()> {
        if text.is_empty() {
            return Ok(());
        }

        // Send text directly to terminal
        terminal.write_str(text)?;
        Ok(())
    }

    /// Handle a mouse press
    pub fn handle_mouse_press(
        &mut self,
        col: usize,
        row: usize,
        button: MouseButton,
    ) -> Result<()> {
        match button {
            MouseButton::Left => {
                // Start selection
                self.selecting.store(true, Ordering::SeqCst);
                self.last_mouse_pos = Some((col, row));
                self.selection = Some(SelectionState {
                    start_col: col,
                    start_row: row,
                    end_col: col,
                    end_row: row,
                });
                log::debug!("Selection started at col={}, row={}", col, row);
            }
            MouseButton::Middle => {
                // Middle click - paste (handled at higher level)
                log::debug!("Middle click at col={}, row={}", col, row);
            }
            MouseButton::Right => {
                // Right click - context menu (handled at higher level)
                log::debug!("Right click at col={}, row={}", col, row);
            }
        }
        Ok(())
    }

    /// Handle mouse release
    pub fn handle_mouse_release(
        &mut self,
        col: usize,
        row: usize,
        button: MouseButton,
    ) -> Result<()> {
        match button {
            MouseButton::Left if self.selecting.load(Ordering::SeqCst) => {
                // Update selection end
                if let Some(ref mut sel) = self.selection {
                    sel.end_col = col;
                    sel.end_row = row;
                }
                self.selecting.store(false, Ordering::SeqCst);
                log::debug!("Selection ended at col={}, row={}", col, row);
            }
            _ => {}
        }
        self.last_mouse_pos = None;
        Ok(())
    }

    /// Handle mouse move (for selection drag)
    pub fn handle_mouse_move(&mut self, col: usize, row: usize) -> Result<()> {
        if self.selecting.load(Ordering::SeqCst) {
            // Update selection end while dragging
            if let Some(ref mut sel) = self.selection {
                sel.end_col = col;
                sel.end_row = row;
            }
            self.last_mouse_pos = Some((col, row));
        }
        Ok(())
    }

    /// Handle mouse wheel
    pub fn handle_mouse_wheel(&mut self, terminal: &mut Terminal, delta: f32) -> Result<()> {
        // Delta is positive for scroll up, negative for scroll down
        let lines = (delta.abs() as usize).max(1);
        if delta > 0.0 {
            terminal.scroll_up(lines);
        } else {
            terminal.scroll_down(lines);
        }
        Ok(())
    }

    /// Get selected text
    pub fn get_selected_text(&self, terminal: &Terminal) -> Option<String> {
        self.selection
            .as_ref()
            .map(|sel| terminal.get_selected_text(sel))
    }

    /// Clear selection
    pub fn clear_selection(&mut self) {
        self.selection = None;
        self.selecting.store(false, Ordering::SeqCst);
    }

    /// Handle a key event and convert it to terminal input bytes
    pub fn handle_key_event(&self, key: &str, mods: KeyModifiers) -> Result<Vec<u8>> {
        let mut output = Vec::new();

        // Ignore pure modifier key presses (Shift, Control, Alt, CapsLock, etc.)
        // Slint uses special Unicode control characters for these keys:
        // \u{0010} = Shift, \u{0011} = Control, \u{0012} = Alt, \u{0013} = AltGr
        // \u{0014} = CapsLock, \u{0015} = ShiftR, \u{0016} = ControlR
        // \u{0017} = Meta, \u{0018} = MetaR
        // These keys only modify other keys and should not produce output on their own
        if let Some(c) = key.chars().next() {
            match c {
                '\u{0010}' | '\u{0011}' | '\u{0012}' | '\u{0013}' |  // Shift, Control, Alt, AltGr
                '\u{0014}' | '\u{0015}' | '\u{0016}' |               // CapsLock, ShiftR, ControlR
                '\u{0017}' | '\u{0018}' => {                          // Meta, MetaR
                    return Ok(output);  // Return empty output for modifier keys
                }
                // Handle Slint's Unicode key codes for basic control keys
                '\u{0008}' => { output.push(0x7F); return Ok(output); }  // Backspace -> DEL
                '\u{0009}' => { output.extend_from_slice(b"\t"); return Ok(output); }  // Tab
                '\u{000a}' => { output.extend_from_slice(b"\r"); return Ok(output); }  // Return -> CR
                '\u{001b}' => { output.extend_from_slice(b"\x1B"); return Ok(output); }  // Escape
                '\u{007f}' => { output.extend_from_slice(b"\x1B[3~"); return Ok(output); }  // Delete
                '\u{0020}' => { output.extend_from_slice(b" "); return Ok(output); }  // Space
                // Handle Slint's Unicode key codes for special keys
                '\u{F700}' => { // UpArrow
                    if mods.shift {
                        output.extend_from_slice(b"\x1B[1;2A");
                    } else if mods.ctrl {
                        output.extend_from_slice(b"\x1B[1;5A");
                    } else if mods.alt {
                        output.extend_from_slice(b"\x1B[1;3A");
                    } else {
                        output.extend_from_slice(b"\x1B[A");
                    }
                    return Ok(output);
                }
                '\u{F701}' => { // DownArrow
                    if mods.shift {
                        output.extend_from_slice(b"\x1B[1;2B");
                    } else if mods.ctrl {
                        output.extend_from_slice(b"\x1B[1;5B");
                    } else if mods.alt {
                        output.extend_from_slice(b"\x1B[1;3B");
                    } else {
                        output.extend_from_slice(b"\x1B[B");
                    }
                    return Ok(output);
                }
                '\u{F702}' => { // LeftArrow
                    if mods.shift {
                        output.extend_from_slice(b"\x1B[1;2D");
                    } else if mods.ctrl {
                        output.extend_from_slice(b"\x1B[1;5D");
                    } else if mods.alt {
                        output.extend_from_slice(b"\x1B[1;3D");
                    } else {
                        output.extend_from_slice(b"\x1B[D");
                    }
                    return Ok(output);
                }
                '\u{F703}' => { // RightArrow
                    if mods.shift {
                        output.extend_from_slice(b"\x1B[1;2C");
                    } else if mods.ctrl {
                        output.extend_from_slice(b"\x1B[1;5C");
                    } else if mods.alt {
                        output.extend_from_slice(b"\x1B[1;3C");
                    } else {
                        output.extend_from_slice(b"\x1B[C");
                    }
                    return Ok(output);
                }
                '\u{F729}' => { // Home
                    if mods.shift {
                        output.extend_from_slice(b"\x1B[1;2H");
                    } else {
                        output.extend_from_slice(b"\x1B[H");
                    }
                    return Ok(output);
                }
                '\u{F72B}' => { // End
                    if mods.shift {
                        output.extend_from_slice(b"\x1B[1;2F");
                    } else {
                        output.extend_from_slice(b"\x1B[F");
                    }
                    return Ok(output);
                }
                '\u{F72C}' => { // PageUp
                    if mods.shift {
                        output.extend_from_slice(b"\x1B[5;2~");
                    } else if mods.ctrl {
                        output.extend_from_slice(b"\x1B[5;5~");
                    } else {
                        output.extend_from_slice(b"\x1B[5~");
                    }
                    return Ok(output);
                }
                '\u{F72D}' => { // PageDown
                    if mods.shift {
                        output.extend_from_slice(b"\x1B[6;2~");
                    } else if mods.ctrl {
                        output.extend_from_slice(b"\x1B[6;5~");
                    } else {
                        output.extend_from_slice(b"\x1B[6~");
                    }
                    return Ok(output);
                }
                '\u{F727}' => { output.extend_from_slice(b"\x1B[2~"); return Ok(output); } // Insert
                '\u{F704}'..='\u{F71B}' => { // F1-F24
                    let f_codes: &[&[u8]] = &[
                        b"\x1BOP", b"\x1BOQ", b"\x1BOR", b"\x1BOS",       // F1-F4
                        b"\x1B[15~", b"\x1B[17~", b"\x1B[18~", b"\x1B[19~", // F5-F8
                        b"\x1B[20~", b"\x1B[21~", b"\x1B[23~", b"\x1B[24~", // F9-F12
                    ];
                    let idx = (c as usize) - 0xF704;
                    if idx < f_codes.len() {
                        output.extend_from_slice(f_codes[idx]);
                    }
                    return Ok(output);
                }
                // Ignore other special Slint keys that shouldn't produce terminal output
                '\u{F72F}' | '\u{F730}' | '\u{F731}' |  // ScrollLock, Pause, SysReq
                '\u{F734}' | '\u{F735}' | '\u{F748}' => {  // Stop, Menu, Back
                    return Ok(output);
                }
                _ => {}
            }
        }

        // Also check for string-based modifier key names (for compatibility)
        match key {
            "Shift" | "ShiftLeft" | "ShiftRight" | "Control" | "ControlLeft" | "ControlRight"
            | "Alt" | "AltLeft" | "AltRight" | "Meta" | "MetaLeft" | "MetaRight" | "CapsLock"
            | "NumLock" | "ScrollLock" | "Fn" | "FnLock" => {
                return Ok(output);
            }
            _ => {}
        }

        // Handle control combinations first
        if mods.ctrl && !mods.alt {
            // Ctrl+key combinations
            match key {
                "c" => output.extend_from_slice(b"\x03"), // ETX (Ctrl+C) - SIGINT
                "d" => output.extend_from_slice(b"\x04"), // EOT (Ctrl+D) - EOF
                "z" => output.extend_from_slice(b"\x1a"), // SUB (Ctrl+Z) - SIGTSTP
                "l" => output.extend_from_slice(b"\x0c"), // FF (Ctrl+L) - form feed (clear screen)
                "a" => output.extend_from_slice(b"\x01"), // SOH (Ctrl+A) - start of line
                "e" => output.extend_from_slice(b"\x05"), // ENQ (Ctrl+E) - end of line
                "u" => output.extend_from_slice(b"\x15"), // NAK (Ctrl+U) - kill line
                "k" => output.extend_from_slice(b"\x0b"), // VT (Ctrl+K) - kill to end of line
                "w" => output.extend_from_slice(b"\x17"), // ETB (Ctrl+W) - delete word
                "b" => output.extend_from_slice(b"\x02"), // STX (Ctrl+B) - backward char
                "f" => output.extend_from_slice(b"\x06"), // ACK (Ctrl+F) - forward char
                "n" => output.extend_from_slice(b"\x0e"), // SO (Ctrl+N) - next line
                "p" => output.extend_from_slice(b"\x10"), // DLE (Ctrl+P) - previous line
                "t" => output.extend_from_slice(b"\x14"), // DC4 (Ctrl+T) - transpose chars
                "r" => output.extend_from_slice(b"\x12"), // DC2 (Ctrl+R) - reverse search
                "s" => output.extend_from_slice(b"\x13"), // DC3 (Ctrl+S) - forward search
                "h" => output.extend_from_slice(b"\x08"), // BS (Ctrl+H) - backspace
                "m" => output.extend_from_slice(b"\x0d"), // CR (Ctrl+M) - carriage return
                "j" => output.extend_from_slice(b"\x0a"), // LF (Ctrl+J) - line feed
                "i" => output.extend_from_slice(b"\x09"), // HT (Ctrl+I) - tab
                "[" => output.extend_from_slice(b"\x1b"), // ESC (Ctrl+[)
                "\\" => output.extend_from_slice(b"\x1c"), // FS (Ctrl+\)
                "]" => output.extend_from_slice(b"\x1d"), // GS (Ctrl+])
                "^" => output.extend_from_slice(b"\x1e"), // RS (Ctrl+^)
                "_" => output.extend_from_slice(b"\x1f"), // US (Ctrl+_)
                " " => output.extend_from_slice(b"\x00"), // NUL (Ctrl+Space)
                "@" => output.extend_from_slice(b"\x00"), // NUL (Ctrl+@)
                "0" => output.extend_from_slice(b"\x00"), // NUL (Ctrl+0)
                "1" => output.extend_from_slice(b"\x01"), // SOH (Ctrl+1)
                "2" => output.extend_from_slice(b"\x00"), // NUL (Ctrl+2)
                "3" => output.extend_from_slice(b"\x1b"), // ESC (Ctrl+3)
                "4" => output.extend_from_slice(b"\x1c"), // FS (Ctrl+4)
                "5" => output.extend_from_slice(b"\x1d"), // GS (Ctrl+5)
                "6" => output.extend_from_slice(b"\x1e"), // RS (Ctrl+6)
                "7" => output.extend_from_slice(b"\x1f"), // US (Ctrl+7)
                "8" => output.extend_from_slice(b"\x7f"), // DEL (Ctrl+8)
                "9" => output.extend_from_slice(b"\x00"), // NUL (Ctrl+9)
                _ => {
                    // For other Ctrl combinations, send the control code
                    if let Some(c) = key.chars().next() {
                        let ctrl_code = (c as u8) & 0x1f;
                        if ctrl_code >= 1 && ctrl_code <= 26 {
                            output.push(ctrl_code);
                        }
                    }
                }
            }
            return Ok(output);
        }

        // Handle special keys
        match key {
            "Enter" | "Return" => output.extend_from_slice(b"\r"),
            "Tab" => output.extend_from_slice(b"\t"),
            "Backspace" => {
                if mods.alt {
                    output.extend_from_slice(b"\x1B\x7F"); // Alt+Backspace
                } else {
                    output.push(0x7F); // DEL (modern backspace)
                }
            }
            "Escape" => output.extend_from_slice(b"\x1B"),
            "Insert" => output.extend_from_slice(b"\x1B[2~"),
            "Delete" => output.extend_from_slice(b"\x1B[3~"),
            "Home" => {
                if mods.shift {
                    output.extend_from_slice(b"\x1B[1;2H"); // Shift+Home
                } else {
                    output.extend_from_slice(b"\x1B[H");
                }
            }
            "End" => {
                if mods.shift {
                    output.extend_from_slice(b"\x1B[1;2F"); // Shift+End
                } else {
                    output.extend_from_slice(b"\x1B[F");
                }
            }
            "PageUp" => {
                if mods.shift {
                    output.extend_from_slice(b"\x1B[5;2~"); // Shift+PageUp
                } else if mods.ctrl {
                    output.extend_from_slice(b"\x1B[5;5~"); // Ctrl+PageUp
                } else {
                    output.extend_from_slice(b"\x1B[5~");
                }
            }
            "PageDown" => {
                if mods.shift {
                    output.extend_from_slice(b"\x1B[6;2~"); // Shift+PageDown
                } else if mods.ctrl {
                    output.extend_from_slice(b"\x1B[6;5~"); // Ctrl+PageDown
                } else {
                    output.extend_from_slice(b"\x1B[6~");
                }
            }
            "ArrowUp" => {
                if mods.shift {
                    output.extend_from_slice(b"\x1B[1;2A"); // Shift+Up
                } else if mods.ctrl {
                    output.extend_from_slice(b"\x1B[1;5A"); // Ctrl+Up
                } else if mods.alt {
                    output.extend_from_slice(b"\x1B[1;3A"); // Alt+Up
                } else {
                    output.extend_from_slice(b"\x1B[A");
                }
            }
            "ArrowDown" => {
                if mods.shift {
                    output.extend_from_slice(b"\x1B[1;2B"); // Shift+Down
                } else if mods.ctrl {
                    output.extend_from_slice(b"\x1B[1;5B"); // Ctrl+Down
                } else if mods.alt {
                    output.extend_from_slice(b"\x1B[1;3B"); // Alt+Down
                } else {
                    output.extend_from_slice(b"\x1B[B");
                }
            }
            "ArrowLeft" => {
                if mods.shift {
                    output.extend_from_slice(b"\x1B[1;2D"); // Shift+Left
                } else if mods.ctrl {
                    output.extend_from_slice(b"\x1B[1;5D"); // Ctrl+Left
                } else if mods.alt {
                    output.extend_from_slice(b"\x1B[1;3D"); // Alt+Left
                } else {
                    output.extend_from_slice(b"\x1B[D");
                }
            }
            "ArrowRight" => {
                if mods.shift {
                    output.extend_from_slice(b"\x1B[1;2C"); // Shift+Right
                } else if mods.ctrl {
                    output.extend_from_slice(b"\x1B[1;5C"); // Ctrl+Right
                } else if mods.alt {
                    output.extend_from_slice(b"\x1B[1;3C"); // Alt+Right
                } else {
                    output.extend_from_slice(b"\x1B[C");
                }
            }
            "F1" => output.extend_from_slice(b"\x1BOP"),
            "F2" => output.extend_from_slice(b"\x1BOQ"),
            "F3" => output.extend_from_slice(b"\x1BOR"),
            "F4" => {
                if mods.shift {
                    output.extend_from_slice(b"\x1B[1;2S"); // Shift+F4
                } else {
                    output.extend_from_slice(b"\x1BOS");
                }
            }
            "F5" => output.extend_from_slice(b"\x1B[15~"),
            "F6" => output.extend_from_slice(b"\x1B[17~"),
            "F7" => output.extend_from_slice(b"\x1B[18~"),
            "F8" => output.extend_from_slice(b"\x1B[19~"),
            "F9" => output.extend_from_slice(b"\x1B[20~"),
            "F10" => output.extend_from_slice(b"\x1B[21~"),
            "F11" => output.extend_from_slice(b"\x1B[23~"),
            "F12" => output.extend_from_slice(b"\x1B[24~"),
            _ => {
                // Regular characters
                for c in key.chars() {
                    if mods.alt {
                        // Alt+key sends ESC prefix
                        output.push(0x1B);
                    }
                    let mut bytes = [0u8; 4];
                    let encoded = c.encode_utf8(&mut bytes);
                    output.extend_from_slice(encoded.as_bytes());
                }
            }
        }

        Ok(output)
    }

    /// Get current selection (if any)
    pub fn get_selection(&self) -> Option<&SelectionState> {
        self.selection.as_ref()
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct KeyModifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub meta: bool,
}

impl KeyModifiers {
    pub fn empty() -> Self {
        Self::default()
    }

    /// Create modifiers with only shift
    pub const SHIFT: Self = Self {
        ctrl: false,
        alt: false,
        shift: true,
        meta: false,
    };

    /// Create modifiers with only alt
    pub const ALT: Self = Self {
        ctrl: false,
        alt: true,
        shift: false,
        meta: false,
    };

    /// Create modifiers with only control
    pub const CONTROL: Self = Self {
        ctrl: true,
        alt: false,
        shift: false,
        meta: false,
    };

    /// Create modifiers with only meta
    pub const META: Self = Self {
        ctrl: false,
        alt: false,
        shift: false,
        meta: true,
    };
}

impl std::ops::BitOr for KeyModifiers {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self {
            ctrl: self.ctrl || rhs.ctrl,
            alt: self.alt || rhs.alt,
            shift: self.shift || rhs.shift,
            meta: self.meta || rhs.meta,
        }
    }
}

impl std::ops::BitOrAssign for KeyModifiers {
    fn bitor_assign(&mut self, rhs: Self) {
        self.ctrl = self.ctrl || rhs.ctrl;
        self.alt = self.alt || rhs.alt;
        self.shift = self.shift || rhs.shift;
        self.meta = self.meta || rhs.meta;
    }
}

#[derive(Debug, Clone)]
pub enum MouseEvent {
    Moved {
        x: f32,
        y: f32,
    },
    Pressed {
        button: MouseButton,
        col: usize,
        row: usize,
    },
    Released {
        button: MouseButton,
        col: usize,
        row: usize,
    },
    Wheel {
        delta: f32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
}
