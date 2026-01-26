use crate::Result;
use chinese_number::{ChineseCountMethod, ChineseToNumber};
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

pub fn chinese_numbers_to_primitive_numbers(chinese_numbers: &str) -> String {
    // 中文数字字符集合（包括简体、繁体和数字单位）
    let chinese_digit_chars = [
        '零', '〇', '一', '二', '三', '四', '五', '六', '七', '八', '九', '十', '百', '千', '万',
        '亿', '兆', '壹', '贰', '叁', '肆', '伍', '陆', '柒', '捌', '玖', '拾', '佰', '仟', '两',
        '俩',
    ];

    let mut result = String::new();
    let mut current_chinese_number = String::new();
    let mut in_number = false;
    let mut after_decimal_point = false;

    for ch in chinese_numbers.chars() {
        if ch == '点' {
            if in_number {
                if after_decimal_point {
                    for digit_ch in current_chinese_number.chars() {
                        if let Ok(number) =
                            <String as ChineseToNumber<u64>>::to_number_naive(&digit_ch.to_string())
                        {
                            result.push_str(&number.to_string());
                        } else {
                            result.push(digit_ch);
                        }
                    }
                } else {
                    if let Ok(number) = <String as ChineseToNumber<u64>>::to_number(
                        &current_chinese_number,
                        ChineseCountMethod::TenThousand,
                    ) {
                        result.push_str(&number.to_string());
                    } else {
                        result.push_str(&current_chinese_number);
                    }
                }
                current_chinese_number.clear();
            }
            result.push('.');
            in_number = true;
            after_decimal_point = true;
        } else if chinese_digit_chars.contains(&ch) {
            current_chinese_number.push(ch);
            in_number = true;
        } else {
            if in_number {
                if after_decimal_point {
                    for digit_ch in current_chinese_number.chars() {
                        if let Ok(number) =
                            <String as ChineseToNumber<u64>>::to_number_naive(&digit_ch.to_string())
                        {
                            result.push_str(&number.to_string());
                        } else {
                            result.push(digit_ch);
                        }
                    }
                } else {
                    if let Ok(number) = <String as ChineseToNumber<u64>>::to_number(
                        &current_chinese_number,
                        ChineseCountMethod::TenThousand,
                    ) {
                        result.push_str(&number.to_string());
                    } else {
                        result.push_str(&current_chinese_number);
                    }
                }
                current_chinese_number.clear();
                in_number = false;
                after_decimal_point = false;
            }
            result.push(ch);
        }
    }

    if in_number {
        if after_decimal_point {
            for digit_ch in current_chinese_number.chars() {
                if let Ok(number) =
                    <String as ChineseToNumber<u64>>::to_number_naive(&digit_ch.to_string())
                {
                    result.push_str(&number.to_string());
                } else {
                    result.push(digit_ch);
                }
            }
        } else {
            if let Ok(number) = <String as ChineseToNumber<u64>>::to_number(
                &current_chinese_number,
                ChineseCountMethod::TenThousand,
            ) {
                result.push_str(&number.to_string());
            } else {
                result.push_str(&current_chinese_number);
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chinese_numbers_simple() {
        // 测试简单中文数字
        assert_eq!(chinese_numbers_to_primitive_numbers("五"), "5");
        assert_eq!(chinese_numbers_to_primitive_numbers("十"), "10");
        assert_eq!(chinese_numbers_to_primitive_numbers("一百"), "100");
        assert_eq!(chinese_numbers_to_primitive_numbers("一千"), "1000");
        assert_eq!(chinese_numbers_to_primitive_numbers("一万"), "10000");
    }

    #[test]
    fn test_chinese_numbers_complex() {
        // 测试复杂中文数字
        assert_eq!(chinese_numbers_to_primitive_numbers("十五"), "15");
        assert_eq!(chinese_numbers_to_primitive_numbers("三十五"), "35");
        assert_eq!(chinese_numbers_to_primitive_numbers("一百二十三"), "123");
        assert_eq!(
            chinese_numbers_to_primitive_numbers("一千二百三十四"),
            "1234"
        );
        assert_eq!(
            chinese_numbers_to_primitive_numbers("一万二千三百四十五"),
            "12345"
        );
    }

    #[test]
    fn test_mixed_text() {
        // 测试混合文本
        assert_eq!(
            chinese_numbers_to_primitive_numbers("我有五本书"),
            "我有5本书"
        );
        assert_eq!(
            chinese_numbers_to_primitive_numbers("今天十五号"),
            "今天15号"
        );
        assert_eq!(
            chinese_numbers_to_primitive_numbers("价格是一千二百元"),
            "价格是1200元"
        );
        assert_eq!(
            chinese_numbers_to_primitive_numbers("第三章：基础教程"),
            "第3章：基础教程"
        );
    }

    #[test]
    fn test_multiple_numbers() {
        // 测试包含多个数字的文本
        assert_eq!(
            chinese_numbers_to_primitive_numbers("五加十等于十五"),
            "5加10等于15"
        );
        assert_eq!(
            chinese_numbers_to_primitive_numbers("一百减五十等于五十"),
            "100减50等于50"
        );
    }

    #[test]
    fn test_zero_and_special() {
        // 测试零和特殊情况
        assert_eq!(chinese_numbers_to_primitive_numbers("零"), "0");
        assert_eq!(chinese_numbers_to_primitive_numbers("一百零五"), "105");
        assert_eq!(
            chinese_numbers_to_primitive_numbers("今天零下五度"),
            "今天0下5度"
        );
    }

    #[test]
    fn test_empty_and_no_numbers() {
        // 测试空字符串和没有数字的情况
        assert_eq!(chinese_numbers_to_primitive_numbers(""), "");
        assert_eq!(chinese_numbers_to_primitive_numbers("你好世界"), "你好世界");
        assert_eq!(
            chinese_numbers_to_primitive_numbers("没有数字的文本"),
            "没有数字的文本"
        );
    }

    #[test]
    fn test_traditional_chinese() {
        // 测试繁体中文数字
        assert_eq!(chinese_numbers_to_primitive_numbers("壹"), "1");
        assert_eq!(chinese_numbers_to_primitive_numbers("叁拾伍"), "35");
    }

    #[test]
    fn test_decimal_numbers() {
        // 测试小数转换
        assert_eq!(chinese_numbers_to_primitive_numbers("一点八"), "1.8");
        assert_eq!(chinese_numbers_to_primitive_numbers("一点八点二"), "1.8.2");
        assert_eq!(chinese_numbers_to_primitive_numbers("三点一四"), "3.14");
        assert_eq!(chinese_numbers_to_primitive_numbers("零点五"), "0.5");
        assert_eq!(chinese_numbers_to_primitive_numbers("十点五"), "10.5");
    }

    #[test]
    fn test_decimal_mixed_text() {
        // 测试混合文本中的小数
        assert_eq!(
            chinese_numbers_to_primitive_numbers("版本一点八点二"),
            "版本1.8.2"
        );
        assert_eq!(
            chinese_numbers_to_primitive_numbers("温度三点五度和三点一五度"),
            "温度3.5度和3.15度"
        );
        assert_eq!(
            chinese_numbers_to_primitive_numbers("圆周率约等于三点一四一五九"),
            "圆周率约等于3.14159"
        );
    }

    #[test]
    fn test_complex_decimal() {
        // 测试复杂小数场景
        assert_eq!(
            chinese_numbers_to_primitive_numbers("一百二十三点四五"),
            "123.45"
        );
        assert_eq!(
            chinese_numbers_to_primitive_numbers("一千点零零一"),
            "1000.001"
        );
        assert_eq!(
            chinese_numbers_to_primitive_numbers("价格是一点五万元"),
            "价格是1.5万元"
        );
    }
}
