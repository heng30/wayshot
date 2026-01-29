use crate::Result;
use chinese_number::{ChineseCountMethod, ChineseToNumber};
use chrono::{NaiveTime, Timelike};
use std::{fs, path::Path};
use unicode_segmentation::UnicodeSegmentation;

type SubtitleSplitResult = Option<((u64, u64, String), (u64, u64, String))>;

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

    // 不应该转换的上下文：一后面跟这些字时，不转换为数字
    let non_number_context_after_yi: &[char] = &['些', '样', '般', '直', '定', '经', '方', '下'];

    let chars: Vec<char> = chinese_numbers.chars().collect();
    let mut result = String::new();
    let mut i = 0;
    let mut after_decimal = false; // 标记是否在小数点后面

    while i < chars.len() {
        let ch = chars[i];

        if ch == '一' {
            if after_decimal {
                // 小数点后的"一"直接转换为"1"
                result.push('1');
                i += 1;
                continue;
            }

            // 检查后面一个字符
            let next_char = if i + 1 < chars.len() {
                Some(chars[i + 1])
            } else {
                None
            };

            // 如果后面跟着非数字上下文的字，保持'一'不变
            if let Some(next) = next_char
                && non_number_context_after_yi.contains(&next)
            {
                result.push(ch);
                i += 1;
                continue;
            }

            // 否则按正常数字处理
            let mut number_end = i + 1;
            while number_end < chars.len() && chinese_digit_chars.contains(&chars[number_end]) {
                number_end += 1;
            }

            let number_str: String = chars[i..number_end].iter().collect();
            if let Ok(number) = <String as ChineseToNumber<u64>>::to_number(
                &number_str,
                ChineseCountMethod::TenThousand,
            ) {
                result.push_str(&number.to_string());
            } else {
                result.push_str(&number_str);
            }
            i = number_end;
        } else if ch == '点' {
            // 检查是否是真正的小数点（前面有数字，后面也有数字）
            let has_number_before = !result.is_empty()
                && result
                    .chars()
                    .last()
                    .map(|c| c.is_ascii_digit())
                    .unwrap_or(false);

            let has_number_after = if i + 1 < chars.len() {
                chinese_digit_chars.contains(&chars[i + 1])
            } else {
                false
            };

            if has_number_before && has_number_after {
                result.push('.');
                after_decimal = true; // 设置标志
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
                if let Ok(number) = <String as ChineseToNumber<u64>>::to_number(
                    &number_str,
                    ChineseCountMethod::TenThousand,
                ) {
                    result.push_str(&number.to_string());
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
                result.push_str(&value.to_string());
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
