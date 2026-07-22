//! Built-in music source implementations.
//!
//! Each source implements the `MusicSource` trait and provides
//! its own URL construction, response parsing, and download URL resolution.

pub mod kugou;
pub mod kuwo;
pub mod netease;
pub mod qianqian;

// Re-export concrete source types for convenience
pub use kugou::KugouMusicSource;
pub use kuwo::KuwoMusicSource;
pub use netease::NeteaseMusicSource;
pub use qianqian::QianqianMusicSource;
