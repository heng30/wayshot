use std::fmt;

/// Video resolution configuration for screen recording.
///
/// This enum defines the target resolution for recorded video.
/// When a resolution other than `Original` is selected, the captured frames
/// will be scaled to maintain aspect ratio while fitting within the target dimensions.
///
/// # Examples
///
/// ```
/// use recorder::Resolution;
///
/// let res_1080p = Resolution::P1080;
/// let res_original = Resolution::Original((1920, 1080));
///
/// println!("1080p dimensions: {:?}", res_1080p.to_dimension());
/// println!("Original dimensions: {:?}", res_original.to_dimension());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resolution {
    /// Keep original resolution with specified dimensions
    Original((u32, u32)),
    /// 480p resolution (480x640 pixels)
    P480,
    /// 720p resolution (1280x720 pixels)
    P720,
    /// 1080p resolution (1920x1080 pixels)
    P1080,
    /// 2K resolution (2560x1440 pixels)
    P2K,
    /// 4K resolution (3840x2160 pixels)
    P4K,
}

impl Resolution {
    /// Calculate the target dimensions for a given original screen size.
    ///
    /// This method returns the width and height that should be used for recording,
    /// taking into account the configured resolution and maintaining aspect ratio.
    ///
    /// # Arguments
    ///
    /// * `original_width` - Original screen width in pixels
    /// * `original_height` - Original screen height in pixels
    ///
    /// # Returns
    ///
    /// A tuple `(width, height)` representing the target dimensions.
    ///
    /// # Examples
    ///
    /// ```
    /// use recorder::Resolution;
    ///
    /// let res = Resolution::P1080;
    /// let (width, height) = res.dimensions(2560, 1440);
    /// assert_eq!(width, 1920);
    /// assert_eq!(height, 1080);
    ///
    /// let res_original = Resolution::Original((1920, 1080));
    /// let (width, height) = res_original.dimensions(2560, 1440);
    /// assert_eq!(width, 2560);
    /// assert_eq!(height, 1440);
    /// ```
    pub fn dimensions(&self, original_width: u32, original_height: u32) -> (u32, u32) {
        match self {
            Resolution::Original(_) => (original_width, original_height),
            Resolution::P480 => Self::calculate_scaled_dimensions(
                (original_width, original_height),
                Resolution::P480.to_dimension(),
            ),
            Resolution::P720 => Self::calculate_scaled_dimensions(
                (original_width, original_height),
                Resolution::P720.to_dimension(),
            ),
            Resolution::P1080 => Self::calculate_scaled_dimensions(
                (original_width, original_height),
                Resolution::P1080.to_dimension(),
            ),
            Resolution::P2K => Self::calculate_scaled_dimensions(
                (original_width, original_height),
                Resolution::P2K.to_dimension(),
            ),
            Resolution::P4K => Self::calculate_scaled_dimensions(
                (original_width, original_height),
                Resolution::P4K.to_dimension(),
            ),
        }
    }

    /// Calculate scaled dimensions while preserving aspect ratio.
    ///
    /// This method ensures that the scaled image fits within the target dimensions
    /// without distortion by maintaining the original aspect ratio.
    ///
    /// # Arguments
    ///
    /// * `original` - Original dimensions as `(width, height)`
    /// * `target` - Target dimensions as `(width, height)`
    ///
    /// # Returns
    ///
    /// Scaled dimensions that fit within the target while preserving aspect ratio.
    fn calculate_scaled_dimensions(original: (u32, u32), target: (u32, u32)) -> (u32, u32) {
        let original_ratio = original.0 as f64 / original.1 as f64;
        let target_ratio = target.0 as f64 / target.1 as f64;

        if original_ratio > target_ratio {
            // Original image is wider, scale based on width
            let height = (target.0 as f64 / original_ratio) as u32;
            (target.0, height.max(1))
        } else {
            // Original image is taller, scale based on height
            let width = (target.1 as f64 * original_ratio) as u32;
            (width.max(1), target.1)
        }
    }

    /// Check if scaling is needed for the given original dimensions.
    ///
    /// # Arguments
    ///
    /// * `original_width` - Original screen width
    /// * `original_height` - Original screen height
    ///
    /// # Returns
    ///
    /// `true` if scaling is needed, `false` if original dimensions should be used.
    ///
    /// # Examples
    ///
    /// ```
    /// use recorder::Resolution;
    ///
    /// let res = Resolution::P1080;
    /// assert!(res.needs_scaling(2560, 1440)); // Needs scaling from 1440p to 1080p
    /// assert!(!res.needs_scaling(1920, 1080)); // Already at 1080p
    ///
    /// let res_original = Resolution::Original((1920, 1080));
    /// assert!(!res_original.needs_scaling(2560, 1440)); // Always uses original
    /// ```
    pub fn needs_scaling(&self, original_width: u32, original_height: u32) -> bool {
        match self {
            Resolution::Original(_) => false,
            _ => {
                let (target_width, target_height) =
                    self.dimensions(original_width, original_height);
                target_width != original_width || target_height != original_height
            }
        }
    }

    /// Determine the preferred resolution based on screen dimensions.
    ///
    /// This method automatically selects an appropriate resolution based on
    /// the screen width, choosing the highest standard resolution that fits.
    ///
    /// # Arguments
    ///
    /// * `width` - Screen width in pixels
    /// * `height` - Screen height in pixels
    ///
    /// # Returns
    ///
    /// The recommended `Resolution` for the given screen size.
    ///
    /// # Examples
    ///
    /// ```
    /// use recorder::Resolution;
    ///
    /// let res = Resolution::preference_resolution(3840, 2160);
    /// assert!(matches!(res, Resolution::P4K));
    ///
    /// let res = Resolution::preference_resolution(1920, 1080);
    /// assert!(matches!(res, Resolution::P1080));
    ///
    /// let res = Resolution::preference_resolution(800, 600);
    /// assert!(matches!(res, Resolution::Original((800, 600))));
    /// ```
    pub fn preference_resolution(width: u32, height: u32) -> Self {
        if height >= 2160 {
            Resolution::P4K
        } else if height >= 1440 {
            Resolution::P2K
        } else if height >= 1080 {
            Resolution::P1080
        } else if height >= 720 {
            Resolution::P720
        } else if height >= 480 {
            Resolution::P480
        } else {
            Resolution::Original((width, height))
        }
    }

    /// Get the standard dimensions for this resolution.
    ///
    /// # Returns
    ///
    /// A tuple `(width, height)` representing the standard dimensions
    /// for this resolution.
    ///
    /// # Examples
    ///
    /// ```
    /// use recorder::Resolution;
    ///
    /// assert_eq!(Resolution::P1080.to_dimension(), (1920, 1080));
    /// assert_eq!(Resolution::P720.to_dimension(), (1280, 720));
    /// assert_eq!(Resolution::Original((800, 600)).to_dimension(), (800, 600));
    /// ```
    pub fn to_dimension(&self) -> (u32, u32) {
        match self {
            Resolution::Original(d) => *d,
            Resolution::P4K => (3840, 2160),
            Resolution::P2K => (2560, 1440),
            Resolution::P1080 => (1920, 1080),
            Resolution::P720 => (1280, 720),
            Resolution::P480 => (640, 480),
        }
    }
}

impl fmt::Display for Resolution {
    /// Format the resolution for display purposes.
    ///
    /// # Examples
    ///
    /// ```
    /// use recorder::Resolution;
    ///
    /// let res = Resolution::P1080;
    /// assert_eq!(res.to_string(), "1080p (1920x1080)");
    ///
    /// let res = Resolution::Original((800, 600));
    /// assert_eq!(res.to_string(), "Original(800x600)");
    /// ```
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Resolution::Original((w, h)) => write!(f, "Original({}x{})", w, h),
            Resolution::P480 => write!(f, "480p (640x480)"),
            Resolution::P720 => write!(f, "720p (1280x720)"),
            Resolution::P1080 => write!(f, "1080p (1920x1080)"),
            Resolution::P2K => write!(f, "2K (2560x1440)"),
            Resolution::P4K => write!(f, "4K (3840x2160)"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolution_dimensions() {
        let res = Resolution::P720;
        let (w, h) = res.dimensions(1920, 1080);
        assert_eq!(w, 1280);
        assert_eq!(h, 720);

        let res = Resolution::P1080;
        let (w, h) = res.dimensions(2560, 1440);
        assert_eq!(w, 1920);
        assert_eq!(h, 1080);
    }

    #[test]
    fn test_aspect_ratio_preservation() {
        // 16:9 original image
        let res = Resolution::P720;
        let (w, h) = res.dimensions(1920, 1080);
        let ratio = w as f64 / h as f64;
        assert!((ratio - 16.0 / 9.0).abs() < 0.01);

        // 4:3 original image
        let (w, h) = res.dimensions(1024, 768);
        let ratio = w as f64 / h as f64;
        assert!((ratio - 4.0 / 3.0).abs() < 0.01);
    }

    #[test]
    fn test_needs_scaling() {
        let res = Resolution::P720;
        assert!(res.needs_scaling(1920, 1080));
        assert!(!res.needs_scaling(1280, 720));

        let res = Resolution::Original((1, 2));
        assert!(!res.needs_scaling(1920, 1080));
    }
}
