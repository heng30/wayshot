use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    pub app_name: String,
    pub terminal: TerminalConfig,
    pub ui: UiConfig,
    pub keybindings: KeybindingsConfig,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TerminalConfig {
    pub shell: String,
    pub shell_args: Vec<String>,
    pub scrollback_lines: usize,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct UiConfig {
    pub theme: String,
    pub tab_bar_position: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct KeybindingsConfig {
    pub new_tab: String,
    pub close_tab: String,
    pub copy: String,
    pub paste: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            app_name: "slint-term".to_string(),
            terminal: TerminalConfig {
                #[cfg(unix)]
                shell: "/bin/bash".to_string(),
                #[cfg(windows)]
                shell: "cmd.exe".to_string(),
                #[cfg(unix)]
                shell_args: vec!["-l".to_string()],
                #[cfg(windows)]
                shell_args: vec![],
                scrollback_lines: 10000,
            },
            ui: UiConfig {
                theme: "dark".to_string(),
                tab_bar_position: "top".to_string(),
            },
            keybindings: KeybindingsConfig {
                new_tab: "ctrl+t".to_string(),
                close_tab: "ctrl+w".to_string(),
                copy: "ctrl+shift+c".to_string(),
                paste: "ctrl+shift+v".to_string(),
            },
        }
    }
}

impl Config {
    pub fn app_name(&self) -> String {
        self.app_name.clone()
    }

    pub fn with_app_name(mut self, app_name: &str) -> Self {
        self.app_name = app_name.to_string();
        self
    }

    /// Get shell program based on config or auto-detect
    pub fn get_shell(&self) -> String {
        if !self.terminal.shell.is_empty() {
            #[cfg(unix)]
            if self.terminal.shell == "/bin/bash" {
                return Self::detect_shell();
            }

            #[cfg(windows)]
            if self.terminal.shell == "cmd.exe" {
                return Self::detect_shell();
            }
            return self.terminal.shell.clone();
        }
        Self::detect_shell()
    }

    /// Detect default shell for the platform
    fn detect_shell() -> String {
        #[cfg(unix)]
        {
            std::env::var("SHELL")
                .ok()
                .unwrap_or_else(|| "/bin/bash".to_string())
        }

        #[cfg(windows)]
        {
            "cmd.exe".to_string()
        }
    }
}
