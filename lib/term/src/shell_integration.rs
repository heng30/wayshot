// Shell integration module
//
// This module handles OSC sequences and command zone tracking
// Ported from zTerm

use std::path::PathBuf;

/// OSC sequence types we care about
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OscSequence {
    /// OSC 133;A - Prompt start
    PromptStart,
    /// OSC 133;B - Command start
    CommandStart,
    /// OSC 133;C - Command executing
    CommandExecuting,
    /// OSC 133;D;exit_code - Command finished
    CommandFinished { exit_code: i32 },
    /// OSC 633;E;command - Command text (VS Code style)
    CommandText { command: String },
    /// OSC 633;P;Cwd=path - Working directory property
    WorkingDirectory { path: String },
    /// OSC 7;file://host/path - Working directory (standard)
    Osc7WorkingDirectory { path: String },
}

/// Scanner state machine states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    /// Normal state, looking for ESC
    Ground,
    /// Saw ESC (\x1b)
    Escape,
    /// Saw ESC ]  (OSC start)
    OscStart,
    /// Collecting OSC content
    OscCollect,
    /// Saw ESC in OSC (possible ST)
    OscEscape,
}

/// High-performance OSC scanner using state machine
///
/// Scans PTY output for OSC 133/633 sequences with O(n) complexity.
#[derive(Debug, derivative::Derivative)]
#[derivative(Default)]
pub struct OscScanner {
    #[derivative(Default(value = "State::Ground"))]
    state: State,

    /// Buffer for collecting OSC content
    #[derivative(Default(value = "Vec::with_capacity(256)"))]
    osc_buffer: Vec<u8>,

    /// Maximum OSC buffer size (prevent memory issues)
    #[derivative(Default(value = "4096"))]
    max_osc_len: usize,
}

impl OscScanner {
    /// Create a scanner with custom max OSC length
    pub fn with_max_len(max_osc_len: usize) -> Self {
        Self {
            state: State::Ground,
            osc_buffer: Vec::with_capacity(256),
            max_osc_len,
        }
    }

    /// Scan input data and extract OSC sequences
    ///
    /// Returns a list of found OSC sequences.
    pub fn scan(&mut self, data: &[u8]) -> Vec<OscSequence> {
        let mut sequences = Vec::new();

        for &byte in data {
            match self.state {
                State::Ground => {
                    if byte == 0x1b {
                        self.state = State::Escape;
                    }
                }
                State::Escape => {
                    if byte == b']' {
                        self.state = State::OscStart;
                        self.osc_buffer.clear();
                    } else if byte == 0x1b {
                        // Another ESC, stay in Escape state
                    } else {
                        self.state = State::Ground;
                    }
                }
                State::OscStart => {
                    // First byte of OSC content
                    if byte == 0x07 {
                        // BEL terminator with empty content
                        self.state = State::Ground;
                    } else if byte == 0x1b {
                        self.state = State::OscEscape;
                    } else {
                        self.osc_buffer.push(byte);
                        self.state = State::OscCollect;
                    }
                }
                State::OscCollect => {
                    if byte == 0x07 {
                        // BEL terminator
                        if let Some(seq) = self.parse_osc() {
                            sequences.push(seq);
                        }
                        self.osc_buffer.clear();
                        self.state = State::Ground;
                    } else if byte == 0x1b {
                        self.state = State::OscEscape;
                    } else if self.osc_buffer.len() < self.max_osc_len {
                        self.osc_buffer.push(byte);
                    } else {
                        // OSC too long, abort
                        self.osc_buffer.clear();
                        self.state = State::Ground;
                    }
                }
                State::OscEscape => {
                    if byte == b'\\' {
                        // ST terminator (ESC \)
                        if let Some(seq) = self.parse_osc() {
                            sequences.push(seq);
                        }
                        self.osc_buffer.clear();
                        self.state = State::Ground;
                    } else if byte == b']' {
                        // New OSC starting
                        self.osc_buffer.clear();
                        self.state = State::OscStart;
                    } else {
                        // Invalid, back to ground
                        self.osc_buffer.clear();
                        self.state = State::Ground;
                    }
                }
            }
        }

        sequences
    }

    /// Parse the collected OSC buffer
    fn parse_osc(&self) -> Option<OscSequence> {
        let content = std::str::from_utf8(&self.osc_buffer).ok()?;

        // OSC 133 - FinalTerm shell integration
        if let Some(rest) = content.strip_prefix("133;") {
            return self.parse_osc_133(rest);
        }

        // OSC 633 - VS Code shell integration
        if let Some(rest) = content.strip_prefix("633;") {
            return self.parse_osc_633(rest);
        }

        // OSC 7 - Working directory
        if let Some(rest) = content.strip_prefix("7;") {
            return self.parse_osc_7(rest);
        }

        None
    }

    /// Parse OSC 133 sequence
    fn parse_osc_133(&self, data: &str) -> Option<OscSequence> {
        let mut parts = data.splitn(2, ';');
        let cmd = parts.next()?;
        let params = parts.next().unwrap_or("");

        match cmd {
            "A" => Some(OscSequence::PromptStart),
            "B" => Some(OscSequence::CommandStart),
            "C" => Some(OscSequence::CommandExecuting),
            "D" => {
                let exit_code = self.parse_exit_code(params);
                Some(OscSequence::CommandFinished { exit_code })
            }
            _ => None,
        }
    }

    /// Parse OSC 633 sequence
    fn parse_osc_633(&self, data: &str) -> Option<OscSequence> {
        let mut parts = data.splitn(2, ';');
        let cmd = parts.next()?;
        let params = parts.next().unwrap_or("");

        match cmd {
            "A" => Some(OscSequence::PromptStart),
            "B" => Some(OscSequence::CommandStart),
            "C" => Some(OscSequence::CommandExecuting),
            "D" => {
                let exit_code = self.parse_exit_code(params);
                Some(OscSequence::CommandFinished { exit_code })
            }
            "E" => {
                let command = Self::decode_percent(params);
                Some(OscSequence::CommandText { command })
            }
            "P" => {
                if let Some(path) = params.strip_prefix("Cwd=") {
                    Some(OscSequence::WorkingDirectory {
                        path: path.to_string(),
                    })
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Parse OSC 7 sequence (working directory)
    fn parse_osc_7(&self, data: &str) -> Option<OscSequence> {
        // Format: file://host/path or just path
        let path = if let Some(rest) = data.strip_prefix("file://") {
            // Skip host part
            if let Some(slash_idx) = rest.find('/') {
                &rest[slash_idx..]
            } else {
                rest
            }
        } else {
            data
        };

        Some(OscSequence::Osc7WorkingDirectory {
            path: Self::decode_percent(path),
        })
    }

    /// Parse exit code from OSC 133 D parameters
    fn parse_exit_code(&self, params: &str) -> i32 {
        if params.is_empty() {
            return 0;
        }

        // Try direct number first
        if let Ok(code) = params.trim().parse::<i32>() {
            return code;
        }

        // Try to find exit code in key=value format (e.g., "err=1")
        for part in params.split(';') {
            if let Some(value) = part.strip_prefix("err=")
                && let Ok(code) = value.parse::<i32>()
            {
                return code;
            }
        }

        0
    }

    /// Decode percent-encoded string
    fn decode_percent(s: &str) -> String {
        let mut result = String::with_capacity(s.len());
        let mut chars = s.chars().peekable();

        while let Some(c) = chars.next() {
            if c == '%' {
                let hex: String = chars.by_ref().take(2).collect();
                if hex.len() == 2
                    && let Ok(byte) = u8::from_str_radix(&hex, 16)
                {
                    result.push(byte as char);
                    continue;
                }
                result.push('%');
                result.push_str(&hex);
            } else {
                result.push(c);
            }
        }

        result
    }

    /// Reset scanner state (e.g., after terminal reset)
    pub fn reset(&mut self) {
        self.state = State::Ground;
        self.osc_buffer.clear();
    }
}

/// Command zone tracks a single command execution
#[derive(Debug, Clone)]
pub struct CommandZone {
    pub prompt_start: usize,
    pub command_start: usize,
    pub output_start: usize,
    pub command_end: usize,
    pub exit_code: i32,
    pub cwd: PathBuf,
    pub command: String,
}

/// Zone manager for tracking command zones
#[derive(derivative::Derivative)]
#[derivative(Default)]
pub struct ZoneManager {
    commands: Vec<CommandZone>,
    current_zone: Option<CommandZone>,
}

impl ZoneManager {
    /// Handle an OSC sequence
    pub fn handle_osc(&mut self, seq: &OscSequence, line: usize) {
        match seq {
            OscSequence::PromptStart => {
                self.current_zone = Some(CommandZone {
                    prompt_start: line,
                    command_start: line,
                    output_start: line,
                    command_end: line,
                    exit_code: 0,
                    cwd: PathBuf::from("/"),
                    command: String::new(),
                });
            }
            OscSequence::CommandStart => {
                if let Some(ref mut zone) = self.current_zone {
                    zone.command_start = line;
                }
            }
            OscSequence::CommandExecuting => {
                if let Some(ref mut zone) = self.current_zone {
                    zone.output_start = line;
                }
            }
            OscSequence::CommandFinished { exit_code } => {
                if let Some(mut zone) = self.current_zone.take() {
                    zone.command_end = line;
                    zone.exit_code = *exit_code;
                    self.commands.push(zone);
                }
            }
            OscSequence::CommandText { command } => {
                if let Some(ref mut zone) = self.current_zone {
                    zone.command = command.clone();
                }
            }
            OscSequence::WorkingDirectory { path } | OscSequence::Osc7WorkingDirectory { path } => {
                if let Some(ref mut zone) = self.current_zone {
                    zone.cwd = PathBuf::from(path);
                }
            }
        }
    }

    /// Add a command zone
    pub fn add_command(&mut self, zone: CommandZone) {
        self.commands.push(zone);
    }

    /// Get command at line
    pub fn get_command_at_line(&self, line: usize) -> Option<&CommandZone> {
        self.commands
            .iter()
            .find(|z| line >= z.prompt_start && line <= z.command_end)
    }

    /// Get all commands
    pub fn commands(&self) -> &[CommandZone] {
        &self.commands
    }
}

#[derive(derivative::Derivative)]
#[derivative(Default)]
pub struct ShellIntegration {
    #[derivative(Default(value = "true"))]
    pub enabled: bool,
    osc_scanner: OscScanner,
    zone_manager: ZoneManager,
}

impl ShellIntegration {
    /// Process PTY data and extract OSC sequences
    pub fn process_pty_data(&mut self, data: &[u8], line: usize) -> Vec<OscSequence> {
        if !self.enabled {
            return Vec::new();
        }

        let sequences = self.osc_scanner.scan(data);
        for seq in &sequences {
            self.zone_manager.handle_osc(seq, line);
        }

        sequences
    }

    /// Get zone manager
    pub fn zone_manager(&self) -> &ZoneManager {
        &self.zone_manager
    }

    /// Get zone manager mutably
    pub fn zone_manager_mut(&mut self) -> &mut ZoneManager {
        &mut self.zone_manager
    }
}
