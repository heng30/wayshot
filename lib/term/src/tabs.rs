// Multi-tab support module
//
// This module manages multiple terminal tabs

use crate::{
    Error, Result,
    terminal::{Terminal, TerminalSize},
    theme::ColorPalette,
};
use std::path::PathBuf;

/// Tab manager for handling multiple terminal tabs
pub struct TabManager {
    pub tabs: Vec<Tab>,
    active_tab: usize,
    pub default_size: TerminalSize,
    color_palette: ColorPalette,
}

/// A single tab containing a terminal instance
pub struct Tab {
    /// Terminal instance
    pub terminal: Terminal,

    /// Tab title
    pub title: String,

    /// Working directory
    pub working_directory: PathBuf,

    /// Whether the terminal process is still running
    pub is_active: bool,
}

impl Tab {
    pub fn new() -> Result<Self> {
        Self::with_size(None, None, None, ColorPalette::default())
    }

    /// Create a new tab with a specific terminal size
    pub fn with_size(
        shell: Option<String>,
        working_directory: Option<PathBuf>,
        size: Option<TerminalSize>,
        color_palette: ColorPalette,
    ) -> Result<Self> {
        let working_directory = working_directory
            .or_else(|| std::env::var("HOME").ok().map(PathBuf::from))
            .unwrap_or_else(|| PathBuf::from("."));

        let terminal = Terminal::with_shell_and_palette(
            shell,
            Some(working_directory.clone()),
            size,
            color_palette,
        )?;

        Ok(Self {
            terminal,
            title: "Terminal".to_string(),
            working_directory,
            is_active: true,
        })
    }

    /// Create a new tab with a specific shell
    pub fn with_shell(shell: Option<String>, working_directory: Option<PathBuf>) -> Result<Self> {
        Self::with_size(shell, working_directory, None, ColorPalette::default())
    }

    /// Update tab title from terminal
    pub fn update_title(&mut self) {
        let terminal_title = self.terminal.get_title().to_string();
        if !terminal_title.is_empty() && terminal_title != self.title {
            self.title = terminal_title;
        }
    }
}

impl TabManager {
    pub fn new() -> Result<Self> {
        Self::with_size_and_palette(TerminalSize::default(), ColorPalette::default())
    }

    /// Create a new tab manager with a specific terminal size
    pub fn with_size(default_size: TerminalSize) -> Result<Self> {
        Self::with_size_and_palette(default_size, ColorPalette::default())
    }

    /// Create a new tab manager with a specific terminal size and color palette
    pub fn with_size_and_palette(
        default_size: TerminalSize,
        color_palette: ColorPalette,
    ) -> Result<Self> {
        let initial_tab = Tab::with_size(None, None, Some(default_size), color_palette.clone())?;

        Ok(Self {
            tabs: vec![initial_tab],
            active_tab: 0,
            default_size,
            color_palette,
        })
    }

    pub fn add_tab(&mut self, tab: Tab) {
        self.tabs.push(tab);
    }

    /// Create and add a new tab using the stored color palette
    pub fn add_new_tab(&mut self) -> Result<()> {
        let tab = Tab::with_size(
            None,
            None,
            Some(self.default_size),
            self.color_palette.clone(),
        )?;
        self.tabs.push(tab);
        Ok(())
    }

    pub fn remove_tab(&mut self, index: usize) -> Result<()> {
        if self.tabs.is_empty() {
            return Ok(());
        }

        if index >= self.tabs.len() {
            return Ok(());
        }

        // Don't allow removing the last tab
        if self.tabs.len() == 1 {
            log::warn!("Cannot remove the last tab");
            return Ok(());
        }

        // Remove the tab
        self.tabs.remove(index);

        // Adjust active tab if needed
        if self.active_tab >= self.tabs.len() {
            self.active_tab = self.tabs.len().saturating_sub(1);
        }

        Ok(())
    }

    pub fn active_tab(&self) -> Option<&Tab> {
        self.tabs.get(self.active_tab)
    }

    pub fn active_tab_mut(&mut self) -> Option<&mut Tab> {
        self.tabs.get_mut(self.active_tab)
    }

    pub fn set_active_tab(&mut self, index: usize) -> Result<()> {
        if index < self.tabs.len() {
            self.active_tab = index;
            Ok(())
        } else {
            Err(Error::TabOutOfBounds { index })
        }
    }

    pub fn tab(&self, index: usize) -> Option<&Tab> {
        self.tabs.get(index)
    }

    pub fn tab_mut(&mut self, index: usize) -> Option<&mut Tab> {
        self.tabs.get_mut(index)
    }

    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    pub fn default_size(&self) -> TerminalSize {
        self.default_size
    }

    pub fn color_palette(&self) -> &ColorPalette {
        &self.color_palette
    }

    pub fn set_color_palette(&mut self, palette: ColorPalette) {
        self.color_palette = palette.clone();
        for tab in &mut self.tabs {
            tab.terminal.set_color_palette(palette.clone());
        }
    }

    pub fn active_tab_index(&self) -> usize {
        self.active_tab
    }

    pub fn update_tab_titles(&mut self) {
        for tab in &mut self.tabs {
            tab.update_title();
        }
    }

    pub fn get_tab_titles(&self) -> Vec<String> {
        self.tabs.iter().map(|tab| tab.title.clone()).collect()
    }
}
