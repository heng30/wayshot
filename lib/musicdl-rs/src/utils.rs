//! Common utility functions for the musicdl-rs library.

/// Extract a value from a nested JSON structure using a path of keys/indices.
///
/// Mirrors Python's `safeextractfromdict()`. Keys can be string object keys
/// or numeric array indices (as string).
pub fn safe_extract_from_dict<'a>(
    data: &'a serde_json::Value,
    path: &[&str],
) -> Option<&'a serde_json::Value> {
    let mut current = data;
    for key in path {
        current = current
            .get(*key)
            .or_else(|| key.parse::<usize>().ok().and_then(|idx| current.get(idx)))?;
    }
    Some(current)
}

/// Extract a string value from a nested JSON structure.
pub fn extract_str<'a>(data: &'a serde_json::Value, path: &[&str]) -> Option<&'a str> {
    safe_extract_from_dict(data, path).and_then(|v| v.as_str())
}

/// Sanitize a string for use as a filename.
///
/// Replaces characters that are invalid in filenames (/\:*?"<>|) with spaces,
/// then collapses multiple spaces into one.
pub fn legalize_string(s: &str) -> String {
    s.chars()
        .map(|c| {
            if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
                ' '
            } else {
                c
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ")
}

/// Convert seconds to human-readable duration string (HH:MM:SS or MM:SS).
///
/// Mirrors Python's `SongInfoUtils.seconds2hms()`.
pub fn seconds_to_hms(seconds: u64) -> String {
    let h = seconds / 3600;
    let m = (seconds % 3600) / 60;
    let s = seconds % 60;
    if h > 0 {
        format!("{}:{:02}:{:02}", h, m, s)
    } else {
        format!("{:02}:{:02}", m, s)
    }
}

/// Convert bytes to human-readable size string (e.g. "4.20 MB").
///
/// Mirrors Python's `SongInfoUtils.byte2mb()`.
pub fn bytes_to_mb(bytes: u64) -> String {
    let mb = bytes as f64 / (1024.0 * 1024.0);
    format!("{:.2} MB", mb)
}

/// Parse a duration string into seconds.
///
/// Handles formats like "3:45", "1:02:30", and Chinese formats like "3分45秒".
pub fn parse_duration_to_seconds(s: &str) -> Option<u64> {
    let s = s.trim();

    // Try HH:MM:SS or MM:SS format
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() >= 2 {
        let nums: Vec<u64> = parts.iter().filter_map(|p| p.parse().ok()).collect();
        if nums.len() == parts.len() {
            return match nums.len() {
                2 => Some(nums[0] * 60 + nums[1]),
                3 => Some(nums[0] * 3600 + nums[1] * 60 + nums[2]),
                _ => None,
            };
        }
    }

    // Try Chinese format
    let re = regex::Regex::new(r"(\d+)\s*(?:小时|时|h|hr)").ok()?;
    let hours: u64 = re.captures(s).and_then(|c| c[1].parse().ok()).unwrap_or(0);

    let re = regex::Regex::new(r"(\d+)\s*(?:分钟|分|m|min)").ok()?;
    let minutes: u64 = re.captures(s).and_then(|c| c[1].parse().ok()).unwrap_or(0);

    let re = regex::Regex::new(r"(\d+)\s*(?:秒|s|sec)").ok()?;
    let secs: u64 = re.captures(s).and_then(|c| c[1].parse().ok()).unwrap_or(0);

    let total = hours * 3600 + minutes * 60 + secs;
    if total > 0 {
        return Some(total);
    }

    // Try bare number as seconds
    s.parse().ok()
}

/// Clean LRC lyrics: remove timestamp-only lines, normalize.
///
/// Returns `None` if the lyrics are empty or contain only timestamps.
pub fn clean_lrc(lrc: &str) -> Option<String> {
    let re = regex::Regex::new(r"^\[\d+:\d+(?:\.\d+)?\]").ok()?;
    let cleaned: Vec<String> = lrc
        .lines()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .filter(|line| {
            // Keep lines that have content after timestamps
            // A line like "[00:03.45]Hello world" has content
            // A line like "[00:03.45]" is timestamp-only
            if let Some(stripped) = re.find(line) {
                let after = line[stripped.end()..].trim();
                !after.is_empty()
            } else {
                true // Non-timestamp lines are kept
            }
        })
        .collect();

    if cleaned.is_empty() {
        return None;
    }

    Some(cleaned.join("\n"))
}

/// Extract duration in seconds from LRC lyrics by looking at the last timestamp.
pub fn extract_duration_from_lrc(lrc: &str) -> Option<u64> {
    let re = regex::Regex::new(r"\[(\d+):(\d+)(?:\.(\d+))?\]").ok()?;
    let mut max_seconds: u64 = 0;
    for cap in re.captures_iter(lrc) {
        let min: u64 = cap[1].parse().ok()?;
        let sec: u64 = cap[2].parse().ok()?;
        max_seconds = max_seconds.max(min * 60 + sec);
    }
    if max_seconds > 0 {
        Some(max_seconds)
    } else {
        None
    }
}

/// Convert Kuwo's lrclist format to standard LRC format.
///
/// Kuwo's H5 API returns lyrics as `[{time: "13.5", lineLyric: "歌词"}, ...]`
/// where `time` is in seconds (as string or number). This converts to `[mm:ss.xx]歌词` format.
pub fn kuwo_lrclist_to_lrc(lrclist: &serde_json::Value) -> Option<String> {
    let items = lrclist.as_array()?;
    if items.is_empty() {
        return None;
    }

    let lines: Vec<String> = items
        .iter()
        .filter_map(|item| {
            // time can be a number or a string
            let time: f64 = item.get("time").and_then(|v| v.as_f64()).or_else(|| {
                item.get("time")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse().ok())
            })?;
            let text = item.get("lineLyric")?.as_str()?.trim();
            if text.is_empty() {
                return None;
            }
            let total_ms = (time * 1000.0).round() as u64;
            let min = total_ms / 60000;
            let sec = (total_ms % 60000) as f64 / 1000.0;
            Some(format!("[{:02}:{:05.2}]{}", min, sec, text))
        })
        .collect();

    if lines.is_empty() {
        return None;
    }

    Some(lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safe_extract_from_dict() {
        let data = serde_json::json!({
            "songResultData": {
                "result": [
                    {"name": "Song1", "id": 123},
                    {"name": "Song2", "id": 456}
                ]
            }
        });
        assert_eq!(
            safe_extract_from_dict(&data, &["songResultData", "result", "0", "name"])
                .and_then(|v| v.as_str()),
            Some("Song1")
        );
        assert_eq!(
            safe_extract_from_dict(&data, &["songResultData", "result", "1", "id"])
                .and_then(|v| v.as_u64()),
            Some(456)
        );
        assert!(safe_extract_from_dict(&data, &["nonexistent"]).is_none());
    }

    #[test]
    fn test_legalize_string() {
        assert_eq!(legalize_string("Hello/World"), "Hello World");
        assert_eq!(legalize_string("Test:Name*Here"), "Test Name Here");
        assert_eq!(legalize_string("  Multiple   Spaces  "), "Multiple Spaces");
    }

    #[test]
    fn test_seconds_to_hms() {
        assert_eq!(seconds_to_hms(0), "00:00");
        assert_eq!(seconds_to_hms(45), "00:45");
        assert_eq!(seconds_to_hms(225), "03:45");
        assert_eq!(seconds_to_hms(3750), "1:02:30");
    }

    #[test]
    fn test_bytes_to_mb() {
        assert_eq!(bytes_to_mb(1024 * 1024), "1.00 MB");
        assert_eq!(bytes_to_mb(4 * 1024 * 1024 + 200 * 1024), "4.20 MB");
    }

    #[test]
    fn test_parse_duration() {
        assert_eq!(parse_duration_to_seconds("3:45"), Some(225));
        assert_eq!(parse_duration_to_seconds("1:02:30"), Some(3750));
        assert_eq!(parse_duration_to_seconds("3分45秒"), Some(225));
        assert_eq!(parse_duration_to_seconds("45"), Some(45));
    }

    #[test]
    fn test_clean_lrc() {
        let lrc = "[00:00.00]\n[00:03.45]Hello world\n[00:07.89]Second line\n";
        let cleaned = clean_lrc(lrc).unwrap();
        assert!(cleaned.contains("Hello world"));
        assert!(cleaned.contains("Second line"));
    }

    #[test]
    fn test_clean_lrc_empty() {
        assert!(clean_lrc("").is_none());
        assert!(clean_lrc("[00:00.00]\n[00:01.00]").is_none());
    }

    #[test]
    fn test_extract_duration_from_lrc() {
        let lrc = "[00:00.00]\n[00:03.45]Hello\n[03:45.00]Last line\n";
        assert_eq!(extract_duration_from_lrc(lrc), Some(225));
    }

    #[test]
    fn test_kuwo_lrclist_to_lrc() {
        // Test with string time values (as returned by the actual API)
        let lrclist = serde_json::json!([
            {"time": "0.0", "lineLyric": "晴天 - 周杰伦"},
            {"time": "29.26", "lineLyric": "故事的小黄花"},
            {"time": "32.71", "lineLyric": "从出生那年就飘着"}
        ]);
        let lrc = kuwo_lrclist_to_lrc(&lrclist).unwrap();
        assert!(lrc.contains("晴天 - 周杰伦"));
        assert!(lrc.contains("故事的小黄花"));
        assert!(lrc.starts_with("[00:00.00]"));

        // Test with numeric time values
        let lrclist_num = serde_json::json!([
            {"time": 0.0, "lineLyric": "晴天 - 周杰伦"},
            {"time": 29.26, "lineLyric": "故事的小黄花"}
        ]);
        let lrc_num = kuwo_lrclist_to_lrc(&lrclist_num).unwrap();
        assert!(lrc_num.contains("晴天 - 周杰伦"));
    }

    #[test]
    fn test_kuwo_lrclist_to_lrc_empty() {
        assert!(kuwo_lrclist_to_lrc(&serde_json::json!([])).is_none());
        assert!(
            kuwo_lrclist_to_lrc(&serde_json::json!([{"time": 0.0, "lineLyric": ""}])).is_none()
        );
    }
}
