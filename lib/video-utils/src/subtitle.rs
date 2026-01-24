use crate::Result;
use chrono::{NaiveTime, Timelike};
use std::{fs, path::Path};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone, Default)]
pub struct Subtitle {
    pub index: u32,
    pub start_timestamp: u64,
    pub end_timestamp: u64,
    pub text: String,
}

#[inline]
pub fn ms_to_srt_timestamp(milliseconds: u64) -> String {
    ms_to_timestamp(milliseconds, ",")
}

fn ms_to_timestamp(milliseconds: u64, ms_sep: &str) -> String {
    let total_seconds = milliseconds / 1000;
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    let millis = milliseconds % 1000;

    format!(
        "{:02}:{:02}:{:02}{ms_sep}{:03}",
        hours, minutes, seconds, millis
    )
}

pub fn srt_timestamp_to_ms(timestamp: &str) -> Result<u64> {
    let time = NaiveTime::parse_from_str(timestamp, "%H:%M:%S,%f")?;

    Ok((time.hour() as u64 * 3600000)
        + (time.minute() as u64 * 60000)
        + (time.second() as u64 * 1000)
        // This's not a bug，chrono would parse ',%f' into nanosecond field
        + (time.nanosecond() as u64))
}

pub fn valid_srt_timestamp(timestamp: &str) -> bool {
    srt_timestamp_to_ms(timestamp).is_ok()
}

pub fn subtitle_to_srt(subtitle: &Subtitle) -> String {
    format!(
        "{}\n{} --> {}\n{}",
        subtitle.index,
        ms_to_srt_timestamp(subtitle.start_timestamp),
        ms_to_srt_timestamp(subtitle.end_timestamp),
        subtitle.text
    )
}

pub fn save_as_srt(subtitle: &[Subtitle], path: impl AsRef<Path>) -> Result<()> {
    let contents = subtitle
        .iter()
        .map(|item| format!("{}\n\n", subtitle_to_srt(&item)))
        .collect::<String>();

    fs::write(path.as_ref(), contents)?;

    Ok(())
}

pub fn split_subtitle_into_two(
    start_timestamp: u64,
    end_timestamp: u64,
    content: &str,
) -> Option<((u64, u64, String), (u64, u64, String))> {
    if content.is_empty() || content.trim().len() <= 1 {
        return None;
    }

    let delimiters = [' ', ',', '.', '，', '。'];
    let mut split_positions: Vec<usize> = Vec::new();

    for (i, c) in content.char_indices() {
        if delimiters.contains(&c) {
            let next_pos = i + c.len_utf8();
            if next_pos <= content.len() {
                split_positions.push(next_pos);
            }
        }
    }

    let (first_part, second_part) = if split_positions.is_empty() {
        let graphemes: Vec<&str> = content.graphemes(true).collect();
        let mid = graphemes.len() / 2;
        let first_part = graphemes[..mid].concat();
        let second_part = graphemes[mid..].concat();
        (first_part, second_part)
    } else {
        let target_split = content.len() / 2;
        let Some(best_split) = split_positions
            .iter()
            .min_by_key(|&&pos| (pos as isize - target_split as isize).abs())
        else {
            return None;
        };

        let first_part = content[..*best_split].trim().to_string();
        let second_part = content[*best_split..].trim().to_string();
        (first_part, second_part)
    };

    let total_chars = content.chars().count();
    let first_part_chars = first_part.chars().count();

    let duration = end_timestamp - start_timestamp;
    let split_time = start_timestamp + (duration * first_part_chars as u64) / total_chars as u64;

    Some((
        (start_timestamp, split_time, first_part),
        (split_time, end_timestamp, second_part),
    ))
}
