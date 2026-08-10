use crate::{Error, Result};
use chinese_number::{ChineseCountMethod, ChineseToNumber};
use chrono::{NaiveTime, Timelike};
use std::{fs, path::Path};
use unicode_segmentation::UnicodeSegmentation;

type SubtitleSplitResult = Option<((u64, u64, String), (u64, u64, String))>;

#[derive(Debug, Clone, Default)]
pub struct LrcEntry {
    pub timestamp_ms: u64,
    pub text: String,
}

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

pub fn valid_srt_timestamps(start: &str, end: &str) -> Result<()> {
    let start_ms = srt_timestamp_to_ms(start)
        .map_err(|_e| Error::SrtParse(format!("invalid srt timestamp: {start}")))?;
    let end_ms = srt_timestamp_to_ms(end)
        .map_err(|_e| Error::SrtParse(format!("invalid srt timestamp: {end}")))?;

    if start_ms >= end_ms {
        return Err(Error::SrtParse(
            "Start timestamp must be before end timestamp".to_string(),
        ));
    }

    Ok(())
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
        .map(|item| format!("{}\n\n", subtitle_to_srt(item)))
        .collect::<String>();

    fs::write(path.as_ref(), contents)?;

    Ok(())
}

pub fn split_subtitle(
    start_timestamp: u64,
    end_timestamp: u64,
    content: &str,
) -> SubtitleSplitResult {
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
        let best_split = split_positions
            .iter()
            .min_by_key(|&&pos| (pos as isize - target_split as isize).abs())
            .copied()?;

        let first_part = content[..best_split].trim().to_string();
        let second_part = content[best_split..].trim().to_string();
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

pub fn chinese_numbers_to_primitive_numbers(chinese_numbers: &str) -> String {
    // 中文数字字符集合（包括简体、繁体和数字单位）
    let chinese_digit_chars = [
        '零', '〇', '一', '二', '三', '四', '五', '六', '七', '八', '九', '十', '百', '千', '万',
        '亿', '兆', '壹', '贰', '叁', '肆', '伍', '陆', '柒', '捌', '玖', '拾', '佰', '仟', '两',
        '俩',
    ];

    // 0-9 基本数字：单独出现时不转换为阿拉伯数字（如"一个"、"一些"中的"一"）
    let basic_digits = [
        '零', '〇', '一', '二', '两', '三', '四', '五', '六', '七', '八', '九',
    ];

    let chars: Vec<char> = chinese_numbers.chars().collect();
    let mut result = String::new();
    let mut i = 0;
    let mut after_decimal = false; // 标记是否在小数点后面

    while i < chars.len() {
        let ch = chars[i];

        if ch == '点' {
            // 检查后面是否有中文数字（决定是否是小数点）
            let has_number_after = if i + 1 < chars.len() {
                chinese_digit_chars.contains(&chars[i + 1])
            } else {
                false
            };

            if has_number_after {
                // 前面有数字（阿拉伯数字或未转换的单个中文数字）才视为小数点
                let mut is_decimal_point = false;
                if let Some(last) = result.chars().last() {
                    if last.is_ascii_digit() {
                        is_decimal_point = true;
                    } else if let Some(digit) = chinese_digit_to_arabic(last) {
                        // 整数部分是单个 0-9 数字时，小数点场景下也要转为阿拉伯数字
                        // （如"三点一四" -> "3.14"）
                        result.pop();
                        result.push(digit);
                        is_decimal_point = true;
                    }
                }

                if is_decimal_point {
                    result.push('.');
                    after_decimal = true; // 设置标志
                } else {
                    result.push(ch);
                    after_decimal = false; // 不是小数点，重置标志
                }
            } else {
                result.push(ch);
                after_decimal = false; // 不是小数点，重置标志
            }
            i += 1;
        } else if chinese_digit_chars.contains(&ch) {
            if after_decimal {
                // 小数点后的数字单独转换为阿拉伯数字
                if let Ok(number) =
                    <String as ChineseToNumber<u64>>::to_number_naive(&ch.to_string())
                {
                    result.push_str(&number.to_string());
                } else {
                    result.push(ch);
                }
                i += 1;
            } else {
                // 正常数字处理
                let mut number_end = i + 1;
                while number_end < chars.len() && chinese_digit_chars.contains(&chars[number_end]) {
                    number_end += 1;
                }

                let number_str: String = chars[i..number_end].iter().collect();

                // 单个 0-9 基本数字不转换（如"一个"、"一些"中的"一"）
                if number_end == i + 1 && basic_digits.contains(&ch) {
                    result.push(ch);
                } else if let Ok(number) = <String as ChineseToNumber<u64>>::to_number(
                    &number_str,
                    ChineseCountMethod::TenThousand,
                ) {
                    result.push_str(&format_thousands(number));
                } else {
                    // 标准解析失败，尝试智能分割转换（处理"八六"、"二十六十四"等非标准格式）
                    let converted = try_smart_convert(&number_str);

                    if !converted.is_empty() {
                        result.push_str(&converted);
                    } else {
                        // 无法转换，保留原字符串
                        result.push_str(&number_str);
                    }
                }
                i = number_end;
            }
        } else {
            result.push(ch);
            after_decimal = false; // 遇到非数字字符，重置小数点标志
            i += 1;
        }
    }

    result
}

/// 智能转换非标准中文数字格式（如"八六"、"二十六十四"等）
fn try_smart_convert(number_str: &str) -> String {
    let chars: Vec<char> = number_str.chars().collect();
    let mut result = String::new();
    let mut i = 0;

    while i < chars.len() {
        // 尝试从当前位置开始找到最长的可解析数字
        let mut parsed = false;
        let mut best_end = i;
        let mut best_value: Option<u64> = None;

        // 尝试不同长度，优先匹配更长的数字
        for end in (i + 1..=chars.len()).rev() {
            let substr: String = chars[i..end].iter().collect();
            if let Ok(number) = <String as ChineseToNumber<u64>>::to_number(
                &substr,
                ChineseCountMethod::TenThousand,
            ) {
                best_end = end;
                best_value = Some(number);
                parsed = true;
                break; // 找到最长的可解析数字
            }
        }

        if parsed {
            if let Some(value) = best_value {
                result.push_str(&format_thousands(value));
            }
            i = best_end;
        } else {
            // 无法解析，尝试逐位转换
            if let Ok(number) =
                <String as ChineseToNumber<u64>>::to_number_naive(&chars[i].to_string())
            {
                result.push_str(&number.to_string());
            } else {
                result.push(chars[i]);
            }
            i += 1;
        }
    }

    result
}

/// 单个中文数字字符转阿拉伯数字字符（0-9），非数字字符返回 None
fn chinese_digit_to_arabic(ch: char) -> Option<char> {
    match ch {
        '零' | '〇' => Some('0'),
        '一' => Some('1'),
        '二' | '两' | '俩' => Some('2'),
        '三' => Some('3'),
        '四' => Some('4'),
        '五' => Some('5'),
        '六' => Some('6'),
        '七' => Some('7'),
        '八' => Some('8'),
        '九' => Some('9'),
        _ => None,
    }
}

/// 为超过三位的数字添加千分位分隔符（如 1000 -> "1,000"）
fn format_thousands(number: u64) -> String {
    let digits = number.to_string();
    let mut result = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
    }
    result
}

/// Parse an LRC file into a list of timestamped entries.
///
/// Supports standard LRC format with timestamps like `[mm:ss.xx]` or `[mm:ss.xxx]`.
/// Multiple timestamps per line are supported (e.g., `[00:10.00][00:20.00]Lyric`).
/// Metadata tags like `[ti:...]`, `[ar:...]`, `[al:...]`, `[offset:...]` are handled.
pub fn parse_lrc(content: &str) -> Vec<LrcEntry> {
    let mut entries: Vec<LrcEntry> = Vec::new();
    let mut offset_ms: i64 = 0;

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Parse metadata tags
        if let Some(inner) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            // Check if it's a metadata tag (contains ':')
            if let Some(colon_pos) = inner.find(':') {
                let tag = &inner[..colon_pos];
                let value = &inner[colon_pos + 1..];

                match tag {
                    "offset" if let Ok(val) = value.parse::<i64>() => {
                        offset_ms = val;
                    }
                    // Other metadata tags (ti, ar, al, by, etc.) are ignored for subtitle purposes
                    _ => {}
                }

                // If the entire line is a single metadata tag, skip further parsing
                if !inner.contains(']') {
                    continue;
                }
            }
        }

        // Extract all timestamps from the line
        let mut timestamps: Vec<u64> = Vec::new();
        let mut text_start = 0;

        let chars: Vec<char> = line.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            if chars[i] == '[' {
                // Try to parse a timestamp
                let mut j = i + 1;
                while j < chars.len() && chars[j] != ']' {
                    j += 1;
                }
                if j < chars.len() {
                    let inner: String = chars[i + 1..j].iter().collect();
                    if let Some(ms) = parse_lrc_timestamp(&inner) {
                        timestamps.push(ms);
                        text_start = j + 1;
                        i = j + 1;
                        continue;
                    }
                }
            }
            break;
        }

        if timestamps.is_empty() {
            continue;
        }

        let text = chars[text_start..].iter().collect::<String>();
        let text = text.trim().to_string();

        if text.is_empty() {
            continue;
        }

        for ts in timestamps {
            let adjusted = if offset_ms >= 0 {
                ts.saturating_add(offset_ms as u64)
            } else {
                ts.saturating_sub(offset_ms.unsigned_abs())
            };
            entries.push(LrcEntry {
                timestamp_ms: adjusted,
                text: text.clone(),
            });
        }
    }

    // Sort by timestamp
    entries.sort_by_key(|e| e.timestamp_ms);
    entries
}

/// Parse a single LRC timestamp like "01:23.45" or "1:23.456" into milliseconds.
fn parse_lrc_timestamp(s: &str) -> Option<u64> {
    // Format: mm:ss.xx or mm:ss.xxx or mm:ss
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 2 {
        return None;
    }

    let minutes: u64 = parts[0].parse().ok()?;

    let sec_parts: Vec<&str> = parts[1].split('.').collect();
    let seconds: u64 = sec_parts[0].parse().ok()?;

    let millis = if sec_parts.len() > 1 {
        let frac = sec_parts[1];
        match frac.len() {
            1 => frac.parse::<u64>().ok()? * 100,
            2 => frac.parse::<u64>().ok()? * 10,
            3 => frac.parse::<u64>().ok()?,
            _ => {
                // Take first 3 digits
                let digits: String = frac.chars().take(3).collect();
                digits.parse::<u64>().ok()?
            }
        }
    } else {
        0
    };

    Some(minutes * 60000 + seconds * 1000 + millis)
}

/// Convert parsed LRC entries into Subtitle items with calculated end timestamps.
/// The end timestamp of each entry is set to the start timestamp of the next entry.
/// The last entry gets a default duration of 3 seconds.
pub fn lrc_to_subtitles(entries: &[LrcEntry]) -> Vec<Subtitle> {
    let default_duration_ms = 3000u64;

    entries
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let end_timestamp = entries
                .get(i + 1)
                .map(|next| next.timestamp_ms)
                .unwrap_or_else(|| entry.timestamp_ms + default_duration_ms);

            Subtitle {
                index: i as u32 + 1,
                start_timestamp: entry.timestamp_ms,
                end_timestamp,
                text: entry.text.clone(),
            }
        })
        .collect()
}

/// Parse an LRC file and convert to Subtitle items.
pub fn parse_lrc_file(path: &Path) -> Result<Vec<Subtitle>> {
    let content = fs::read_to_string(path)?;
    let entries = parse_lrc(&content);
    Ok(lrc_to_subtitles(&entries))
}

#[cfg(test)]
mod tests_lrc {
    use super::*;

    #[test]
    fn test_parse_lrc_basic() {
        let content = "[00:10.00]Hello World\n[00:20.50]Second Line\n[01:30.99]Final Line";
        let entries = parse_lrc(content);
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].timestamp_ms, 10_000);
        assert_eq!(entries[0].text, "Hello World");
        assert_eq!(entries[1].timestamp_ms, 20_500);
        assert_eq!(entries[1].text, "Second Line");
        assert_eq!(entries[2].timestamp_ms, 90_990);
        assert_eq!(entries[2].text, "Final Line");
    }

    #[test]
    fn test_parse_lrc_multiple_timestamps() {
        let content = "[00:10.00][00:30.00]Repeated lyric";
        let entries = parse_lrc(content);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].timestamp_ms, 10_000);
        assert_eq!(entries[0].text, "Repeated lyric");
        assert_eq!(entries[1].timestamp_ms, 30_000);
        assert_eq!(entries[1].text, "Repeated lyric");
    }

    #[test]
    fn test_parse_lrc_metadata_tags() {
        let content = "[ti:Song Title]\n[ar:Artist]\n[offset:500]\n[00:10.00]With offset";
        let entries = parse_lrc(content);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].timestamp_ms, 10_500); // 10000 + 500 offset
        assert_eq!(entries[0].text, "With offset");
    }

    #[test]
    fn test_parse_lrc_negative_offset() {
        let content = "[offset:-2000]\n[00:10.00]Shifted back";
        let entries = parse_lrc(content);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].timestamp_ms, 8_000); // 10000 - 2000
    }

    #[test]
    fn test_parse_lrc_empty_lines() {
        let content = "\n[00:05.00]First\n\n\n[00:15.00]Second\n";
        let entries = parse_lrc(content);
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn test_lrc_to_subtitles() {
        let entries = vec![
            LrcEntry {
                timestamp_ms: 10_000,
                text: "Line 1".into(),
            },
            LrcEntry {
                timestamp_ms: 20_000,
                text: "Line 2".into(),
            },
            LrcEntry {
                timestamp_ms: 30_000,
                text: "Line 3".into(),
            },
        ];
        let subs = lrc_to_subtitles(&entries);
        assert_eq!(subs.len(), 3);
        assert_eq!(subs[0].start_timestamp, 10_000);
        assert_eq!(subs[0].end_timestamp, 20_000);
        assert_eq!(subs[1].start_timestamp, 20_000);
        assert_eq!(subs[1].end_timestamp, 30_000);
        // Last entry gets 3s default duration
        assert_eq!(subs[2].start_timestamp, 30_000);
        assert_eq!(subs[2].end_timestamp, 33_000);
    }

    #[test]
    fn test_parse_lrc_timestamp_formats() {
        // mm:ss.xx (2 decimal)
        assert_eq!(parse_lrc_timestamp("01:23.45"), Some(83_450));
        // mm:ss.xxx (3 decimal)
        assert_eq!(parse_lrc_timestamp("01:23.456"), Some(83_456));
        // mm:ss (no decimal)
        assert_eq!(parse_lrc_timestamp("01:23"), Some(83_000));
        // mm:ss.x (1 decimal)
        assert_eq!(parse_lrc_timestamp("01:23.4"), Some(83_400));
    }

    #[test]
    fn test_parse_lrc_invalid_timestamp() {
        assert_eq!(parse_lrc_timestamp("invalid"), None);
        assert_eq!(parse_lrc_timestamp("1:2:3"), None);
    }
}
