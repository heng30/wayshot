//! ttfx-rs: terminal text effects rendered to images.
//!
//! A Rust port of [TerminalTextEffects](https://github.com/ChrisBuilds/terminaltexteffects)
//! (TTE). The engine animates input text character by character; instead of
//! emitting ANSI escape sequences to a terminal, this crate rasterizes each
//! frame into an image (PNG sequence, GIF, or raw pixel buffers) via
//! [`render`].

// Ported code deliberately mirrors the Python original's structure (indexed
// loops for `range(len(...))`, `min/max` nests for Python's `min(max(...))`,
// pre-declared variables assigned in branches). Keep these allowances when
// touching ported code.
#![allow(
    clippy::needless_range_loop,
    clippy::explicit_counter_loop,
    clippy::needless_late_init,
    clippy::manual_clamp,
    // OrderedMap shrinks its hot struct with a boxed index (deliberate).
    clippy::box_collection,
)]

pub mod effects;
pub mod engine;
pub mod render;
pub mod utils;
