//! Slint Terminal Library
//!
//! Terminal emulator core — UI-framework independent.
//! The Slint UI bridge lives in the `slint-term` example binary.

pub mod config;
pub mod error;
pub mod input;
pub mod render;
pub mod shell_integration;
pub mod tabs;
pub mod terminal;
pub mod theme;

pub use error::{Error, Result};
