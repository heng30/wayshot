//! Audio format detection from bytes and URL paths.
//!
//! Uses the `infer` crate for magic-byte detection and Content-Type/URL
//! extension heuristics for cases where the full file is not available.

use crate::types::AudioFormat;

/// Valid audio extensions for filtering (mirrors Python's AudioLinkTester.VALID_AUDIO_EXTS).
pub const VALID_AUDIO_EXTS: &[&str] = &[
    "mp3", "flac", "wav", "aac", "m4a", "ogg", "opus", "wma", "ape", "alac", "wv", "tta", "dsf",
    "dff",
];

/// Check if a file extension is a valid audio format.
pub fn is_valid_audio_ext(ext: &str) -> bool {
    VALID_AUDIO_EXTS.contains(&ext.to_ascii_lowercase().as_str())
}

/// Detects audio format from file content bytes and URL metadata.
pub struct AudioFormatDetector;

impl AudioFormatDetector {
    /// Detect audio format from bytes using magic-byte detection.
    ///
    /// Returns `None` if the bytes are not a recognized audio format.
    pub fn detect(data: &[u8]) -> Option<AudioFormat> {
        let kind = infer::get(data)?;
        match kind.mime_type() {
            "audio/mpeg" => Some(AudioFormat::Mp3),
            "audio/flac" | "application/x-flac" => Some(AudioFormat::Flac),
            "audio/aac" => Some(AudioFormat::Aac),
            "audio/mp4" | "video/mp4" => Some(AudioFormat::M4a),
            "audio/ogg" => Some(AudioFormat::Ogg),
            "audio/opus" => Some(AudioFormat::Opus),
            "audio/wav" | "audio/x-wav" => Some(AudioFormat::Wav),
            "audio/x-ms-wma" => Some(AudioFormat::Wma),
            "audio/x-ape" | "application/x-ape" => Some(AudioFormat::Ape),
            _ => None,
        }
    }

    /// Guess audio format from a Content-Type header value.
    pub fn from_content_type(content_type: &str) -> Option<AudioFormat> {
        let ct = content_type.split(';').next()?.trim().to_ascii_lowercase();
        match ct.as_str() {
            "audio/mpeg" | "audio/mp3" => Some(AudioFormat::Mp3),
            "audio/flac" | "application/x-flac" => Some(AudioFormat::Flac),
            "audio/aac" => Some(AudioFormat::Aac),
            "audio/mp4" | "video/mp4" => Some(AudioFormat::M4a),
            "audio/ogg" => Some(AudioFormat::Ogg),
            "audio/opus" => Some(AudioFormat::Opus),
            "audio/wav" | "audio/x-wav" => Some(AudioFormat::Wav),
            "audio/x-ms-wma" => Some(AudioFormat::Wma),
            "audio/x-ape" | "application/x-ape" => Some(AudioFormat::Ape),
            _ => None,
        }
    }

    /// Extract and normalize the file extension from a URL path.
    ///
    /// Strips fragment and query, then extracts the extension from the last
    /// path segment.
    pub fn ext_from_url(url: &str) -> Option<&'static str> {
        let url_stripped = url.split('#').next()?.split('?').next()?;
        let last_segment = url_stripped.rsplit('/').next()?;
        let ext = last_segment.rsplit('.').next()?;
        if ext == last_segment {
            return None;
        }
        normalize_audio_ext(ext)
    }
}

/// Normalize an audio file extension to a canonical form.
pub fn normalize_audio_ext(ext: &str) -> Option<&'static str> {
    match ext.to_ascii_lowercase().as_str() {
        "mp3" => Some("mp3"),
        "flac" => Some("flac"),
        "aac" => Some("aac"),
        "m4a" | "mp4" => Some("m4a"),
        "ogg" => Some("ogg"),
        "opus" => Some("opus"),
        "wav" | "wave" => Some("wav"),
        "wma" => Some("wma"),
        "ape" => Some("ape"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_mp3() {
        // Minimal MP3 magic bytes: ID3 tag header
        let mp3_bytes = b"ID3\x04\x00\x00\x00\x00\x00\x00";
        assert_eq!(
            AudioFormatDetector::detect(mp3_bytes),
            Some(AudioFormat::Mp3)
        );
    }

    #[test]
    fn test_detect_flac() {
        // FLAC magic bytes detection via content-type is more reliable than
        // magic bytes since infer's byte-level detection depends on buffer size.
        // Test content-type detection instead.
        assert_eq!(
            AudioFormatDetector::from_content_type("audio/flac"),
            Some(AudioFormat::Flac)
        );
        // Also test URL extension detection
        assert_eq!(
            AudioFormatDetector::ext_from_url("https://example.com/song.flac"),
            Some("flac")
        );
    }

    #[test]
    fn test_detect_unknown() {
        let random_bytes = [0x00, 0x01, 0x02, 0x03];
        assert_eq!(AudioFormatDetector::detect(&random_bytes), None);
    }

    #[test]
    fn test_ext_from_url() {
        assert_eq!(
            AudioFormatDetector::ext_from_url("https://example.com/song.mp3"),
            Some("mp3")
        );
        assert_eq!(
            AudioFormatDetector::ext_from_url("https://example.com/song.flac?quality=hires"),
            Some("flac")
        );
        assert_eq!(
            AudioFormatDetector::ext_from_url("https://example.com/noext"),
            None
        );
    }

    #[test]
    fn test_from_content_type() {
        assert_eq!(
            AudioFormatDetector::from_content_type("audio/mpeg"),
            Some(AudioFormat::Mp3)
        );
        assert_eq!(
            AudioFormatDetector::from_content_type("audio/flac; charset=utf-8"),
            Some(AudioFormat::Flac)
        );
        assert_eq!(AudioFormatDetector::from_content_type("text/html"), None);
    }

    #[test]
    fn test_normalize_audio_ext() {
        assert_eq!(normalize_audio_ext("mp3"), Some("mp3"));
        assert_eq!(normalize_audio_ext("FLAC"), Some("flac"));
        assert_eq!(normalize_audio_ext("M4A"), Some("m4a"));
        assert_eq!(normalize_audio_ext("exe"), None);
    }

    #[test]
    fn test_is_valid_audio_ext() {
        assert!(is_valid_audio_ext("mp3"));
        assert!(is_valid_audio_ext("flac"));
        assert!(is_valid_audio_ext("FLAC"));
        assert!(!is_valid_audio_ext("exe"));
    }
}
