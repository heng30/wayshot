//! Image format detection from bytes and URL paths.

use crate::types::ImageFormat;

/// Detects image format from file content bytes.
pub struct ImageFormatDetector;

impl ImageFormatDetector {
    /// Detect image format from bytes.
    ///
    /// Returns `None` if the bytes are not a recognized image format.
    /// Uses the `infer` crate which checks magic bytes (file signatures).
    pub fn detect(data: &[u8]) -> Option<ImageFormat> {
        let kind = infer::get(data)?;
        match kind.mime_type() {
            "image/jpeg" => Some(ImageFormat::Jpeg),
            "image/png" => Some(ImageFormat::Png),
            "image/gif" => Some(ImageFormat::Gif),
            "image/webp" => Some(ImageFormat::WebP),
            "image/bmp" => Some(ImageFormat::Bmp),
            "image/tiff" => Some(ImageFormat::Tiff),
            "image/x-icon" => Some(ImageFormat::Ico),
            "image/avif" => Some(ImageFormat::Avif),
            "image/heic" | "image/heif" => Some(ImageFormat::Heif),
            _ => None,
        }
    }

    /// Extract and normalize the file extension from a URL path.
    ///
    /// Strips fragment and query, then extracts the extension from the last
    /// path segment. Normalizes "jpeg" → "jpg", "tiff" → "tif".
    pub fn ext_from_url(url: &str) -> Option<&'static str> {
        // Strip fragment and query
        let url_stripped = url.split('#').next()?.split('?').next()?;
        // Get the last path segment
        let last_segment = url_stripped.rsplit('/').next()?;
        // Get the extension after the last dot
        let ext = last_segment.rsplit('.').next()?;
        if ext == last_segment {
            // No dot found
            return None;
        }
        normalize_ext(ext)
    }
}

/// Normalize a file extension to a canonical form.
///
/// - "jpeg" → "jpg"
/// - "tiff" → "tif"
/// - All others are lowercased.
pub fn normalize_ext(ext: &str) -> Option<&'static str> {
    match ext.to_ascii_lowercase().as_str() {
        "jpg" | "jpeg" => Some("jpg"),
        "png" => Some("png"),
        "gif" => Some("gif"),
        "webp" => Some("webp"),
        "bmp" => Some("bmp"),
        "tif" | "tiff" => Some("tif"),
        "ico" => Some("ico"),
        "avif" => Some("avif"),
        "heic" | "heif" => Some("heif"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_jpeg() {
        // Minimal JPEG magic bytes: FF D8 FF
        let jpeg_bytes = [0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46];
        assert_eq!(
            ImageFormatDetector::detect(&jpeg_bytes),
            Some(ImageFormat::Jpeg)
        );
    }

    #[test]
    fn test_detect_png() {
        // PNG magic bytes
        let png_bytes: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        assert_eq!(
            ImageFormatDetector::detect(&png_bytes),
            Some(ImageFormat::Png)
        );
    }

    #[test]
    fn test_detect_gif() {
        // GIF87a magic bytes
        let gif_bytes = b"GIF87a\x00\x00";
        assert_eq!(
            ImageFormatDetector::detect(gif_bytes),
            Some(ImageFormat::Gif)
        );
    }

    #[test]
    fn test_detect_unknown() {
        let random_bytes = [0x00, 0x01, 0x02, 0x03];
        assert_eq!(ImageFormatDetector::detect(&random_bytes), None);
    }

    #[test]
    fn test_ext_from_url() {
        assert_eq!(
            ImageFormatDetector::ext_from_url("https://example.com/photo.jpg"),
            Some("jpg")
        );
        assert_eq!(
            ImageFormatDetector::ext_from_url("https://example.com/photo.png?size=large"),
            Some("png")
        );
        assert_eq!(
            ImageFormatDetector::ext_from_url("https://example.com/photo.jpeg#fragment"),
            Some("jpg")
        );
        assert_eq!(
            ImageFormatDetector::ext_from_url("https://example.com/photo.tiff"),
            Some("tif")
        );
        assert_eq!(
            ImageFormatDetector::ext_from_url("https://example.com/noext"),
            None
        );
    }

    #[test]
    fn test_normalize_ext() {
        assert_eq!(normalize_ext("jpg"), Some("jpg"));
        assert_eq!(normalize_ext("jpeg"), Some("jpg"));
        assert_eq!(normalize_ext("JPEG"), Some("jpg"));
        assert_eq!(normalize_ext("tiff"), Some("tif"));
        assert_eq!(normalize_ext("TIF"), Some("tif"));
        assert_eq!(normalize_ext("webp"), Some("webp"));
        assert_eq!(normalize_ext("exe"), None);
    }

    #[test]
    fn test_image_format_extension() {
        assert_eq!(ImageFormat::Jpeg.extension(), "jpg");
        assert_eq!(ImageFormat::Png.extension(), "png");
        assert_eq!(ImageFormat::Gif.extension(), "gif");
        assert_eq!(ImageFormat::WebP.extension(), "webp");
        assert_eq!(ImageFormat::Tiff.extension(), "tif");
        assert_eq!(ImageFormat::Avif.extension(), "avif");
        assert_eq!(ImageFormat::Heif.extension(), "heif");
    }
}
