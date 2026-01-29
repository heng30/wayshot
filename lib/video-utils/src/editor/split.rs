//! Video splitting functionality
//!
//! Allows dividing a video into multiple segments at specified timestamps.

use crate::{Result, Error};
use derivative::Derivative;
use derive_setters::Setters;
use std::path::Path;
use std::time::Duration;

/// Configuration for video splitting
#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct SplitConfig {
    /// Input video file
    #[derivative(Default(value = "String::new()"))]
    pub input: String,
    /// Output directory for split segments
    #[derivative(Default(value = "String::new()"))]
    pub output_dir: String,
    /// Split points (timestamps in seconds)
    #[derivative(Default(value = "Vec::new()"))]
    pub split_points: Vec<f64>,
    /// Output file name pattern (use {index} for segment number, {start} and {end} for timestamps)
    #[derivative(Default(value = "\"segment_{index}.mp4\".to_string()"))]
    pub name_pattern: String,
    /// Whether to include a concat list file for re-merging
    #[derivative(Default(value = "false"))]
    pub generate_concat_list: bool,
}

impl SplitConfig {
    /// Create a new split config (convenience method)
    pub fn new(input: impl Into<String>, output_dir: impl Into<String>, split_points: Vec<f64>) -> Self {
        Self::default()
            .with_input(input.into())
            .with_output_dir(output_dir.into())
            .with_split_points(split_points)
            .with_name_pattern("segment_{index}.mp4".to_string())
    }

    /// Create a split config with common defaults
    pub fn with_segments(input: impl Into<String>, output_dir: impl Into<String>, split_points: Vec<f64>) -> Self {
        Self {
            input: input.into(),
            output_dir: output_dir.into(),
            split_points,
            ..Default::default()
        }
    }
}

/// Split a video at specified timestamps
///
/// This function divides a video into multiple segments at the given timestamps.
/// Each segment is saved as a separate file.
///
/// # Arguments
/// * `config` - Split configuration
///
/// # Returns
/// * Vector of output file paths
///
/// # Example
/// ```no_run
/// use video_utils::editor::split::{split_video, SplitConfig};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// // Split at 10s, 30s, and 60s
/// let config = SplitConfig::new(
///     "input.mp4",
///     "output_dir",
///     vec![10.0, 30.0, 60.0],
/// );
///
/// let outputs = split_video(config)?;
/// println!("Created {} segments", outputs.len());
/// # Ok(())
/// # }
/// ```
pub fn split_video(config: SplitConfig) -> Result<Vec<String>> {
    use crate::editor::trim::trim_video;
    use crate::metadata::get_metadata;

    log::info!("Splitting video at {} points", config.split_points.len());

    // Validate input file exists
    if !Path::new(&config.input).exists() {
        return Err(Error::IO(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Input file not found: {}", config.input),
        )));
    }

    // Create output directory if it doesn't exist
    std::fs::create_dir_all(&config.output_dir)
        .map_err(|e| Error::IO(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("Failed to create output directory: {}", e),
        )))?;

    // Get video duration
    let metadata = get_metadata(&config.input)?;
    let total_duration = metadata.duration;

    // Sort split points and add start (0) and end (duration)
    let mut all_points = config.split_points.clone();
    all_points.push(0.0);
    all_points.push(total_duration);
    all_points.sort_by(|a, b| a.partial_cmp(b).unwrap());

    // Remove duplicates
    all_points.dedup();

    log::debug!("Split points: {:?}", all_points);

    let mut output_files = Vec::new();
    let mut concat_list_entries = Vec::new();

    // Create segments
    for (idx, window) in all_points.windows(2).enumerate() {
        let start = window[0];
        let end = window[1];
        let duration = end - start;

        if duration <= 0.0 {
            continue;
        }

        // Generate output filename
        let filename = config.name_pattern
            .replace("{index}", &(idx + 1).to_string())
            .replace("{start}", &format!("{:.2}", start))
            .replace("{end}", &format!("{:.2}", end));

        let output_path = Path::new(&config.output_dir).join(&filename);
        let output_path_str = output_path.to_string_lossy().to_string();

        log::info!("Creating segment {}/{}: {:.2}s - {:.2}s (duration: {:.2}s)",
                   idx + 1, all_points.len() - 1, start, end, duration);

        // Use trim_video to extract this segment
        let trim_config = crate::editor::trim::TrimConfig::new(
            config.input.clone(),
            output_path_str.clone(),
            Duration::from_secs_f64(start),
        )
        .with_duration(Some(Duration::from_secs_f64(duration)));

        trim_video(trim_config)
            .map_err(|e| Error::FFmpeg(format!("Failed to create segment {}: {}", idx + 1, e)))?;

        output_files.push(output_path_str.clone());

        // Add to concat list
        if config.generate_concat_list {
            concat_list_entries.push(format!("file '{}'", output_path_str));
        }
    }

    // Generate concat list file if requested
    if config.generate_concat_list && !concat_list_entries.is_empty() {
        let concat_list_path = Path::new(&config.output_dir).join("concat_list.txt");
        let concat_content = concat_list_entries.join("\n");
        std::fs::write(&concat_list_path, concat_content)
            .map_err(|e| Error::IO(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to write concat list: {}", e),
            )))?;
        log::info!("Created concat list: {}", concat_list_path.display());
    }

    log::info!("Split complete: {} segments created in {}", output_files.len(), config.output_dir);

    Ok(output_files)
}

/// Split video at equal intervals
///
/// # Arguments
/// * `input` - Input video path
/// * `output_dir` - Output directory
/// * `num_segments` - Number of equal segments to create
///
/// # Example
/// ```no_run
/// use video_utils::editor::split::split_equal;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// // Split into 3 equal parts
/// let outputs = split_equal("input.mp4", "output", 3)?;
/// # Ok(())
/// # }
/// ```
pub fn split_equal(input: impl Into<String>, output_dir: impl Into<String>, num_segments: usize) -> Result<Vec<String>> {
    use crate::metadata::get_metadata;

    let input = input.into();
    let metadata = get_metadata(&input)?;

    let total_duration = metadata.duration;
    let segment_duration = total_duration / num_segments as f64;

    let mut split_points = Vec::new();
    for i in 1..num_segments {
        split_points.push(i as f64 * segment_duration);
    }

    let config = SplitConfig::new(input, output_dir, split_points)
        .with_name_pattern("segment_{index}.mp4".to_string());

    split_video(config)
}

/// Split video into fixed-duration segments
///
/// # Arguments
/// * `input` - Input video path
/// * `output_dir` - Output directory
/// * `segment_duration` - Duration of each segment in seconds
///
/// # Example
/// ```no_run
/// use video_utils::editor::split::split_by_duration;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// // Split into 10-second segments
/// let outputs = split_by_duration("input.mp4", "output", 10.0)?;
/// # Ok(())
/// # }
/// ```
pub fn split_by_duration(input: impl Into<String>, output_dir: impl Into<String>, segment_duration: f64) -> Result<Vec<String>> {
    use crate::metadata::get_metadata;

    let input = input.into();
    let metadata = get_metadata(&input)?;

    let total_duration = metadata.duration;
    let num_segments = (total_duration / segment_duration).ceil() as usize;

    let mut split_points = Vec::new();
    for i in 1..num_segments {
        split_points.push(i as f64 * segment_duration);
    }

    let config = SplitConfig::new(input, output_dir, split_points)
        .with_name_pattern("segment_{index}_{start}s-{end}s.mp4".to_string());

    split_video(config)
}

/// Convenience function to split at specific points
pub fn split_at_points(input: &str, output_dir: &str, points: Vec<f64>) -> Result<Vec<String>> {
    let config = SplitConfig::new(input, output_dir, points);
    split_video(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_config_creation() {
        let config = SplitConfig::new(
            "input.mp4",
            "output",
            vec![10.0, 20.0, 30.0],
        );

        assert_eq!(config.input, "input.mp4");
        assert_eq!(config.output_dir, "output");
        assert_eq!(config.split_points.len(), 3);
        assert_eq!(config.name_pattern, "segment_{index}.mp4");
    }

    #[test]
    fn test_split_config_with_pattern() {
        let config = SplitConfig::new("input.mp4", "output", vec![10.0])
            .with_name_pattern("clip_{index}_{start}s.mp4");

        assert_eq!(config.name_pattern, "clip_{index}_{start}s.mp4");
    }

    #[test]
    fn test_split_config_with_concat() {
        let config = SplitConfig::new("input.mp4", "output", vec![10.0])
            .with_concat_list(true);

        assert!(config.generate_concat_list);
    }
}
