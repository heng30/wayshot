fn chinese_numbers_to_primitive_numbers(chinese_numbers: &str) -> String {
    let chinese_digit_chars = [
        '零', '〇', '一', '二', '三', '四', '五', '六', '七', '八', '九', '十', '百', '千', '万',
        '亿', '兆', '壹', '贰', '叁', '肆', '伍', '陆', '柒', '捌', '玖', '拾', '佰', '仟', '两',
        '俩',
    ];

    let non_number_context_after_yi: &[char] = &['些', '样', '般', '直', '定', '经', '方', '下'];

    let chars: Vec<char> = chinese_numbers.chars().collect();
    let mut result = String::new();
    let mut i = 0;
    let mut after_decimal = false;

    while i < chars.len() {
        let ch = chars[i];

        if ch == '一' {
            if after_decimal {
                result.push('1');
                i += 1;
                continue;
            }

            let next_char = if i + 1 < chars.len() {
                Some(chars[i + 1])
            } else {
                None
            };

            if let Some(next) = next_char {
                if non_number_context_after_yi.contains(&next) {
                    result.push(ch);
                    i += 1;
                    continue;
                }
            }

            let mut number_end = i + 1;
            while number_end < chars.len() && chinese_digit_chars.contains(&chars[number_end]) {
                number_end += 1;
            }

            let number_str: String = chars[i..number_end].iter().collect();
            if let Ok(number) = <String as chinese_number::ChineseToNumber<u64>>::to_number(
                &number_str,
                chinese_number::ChineseCountMethod::TenThousand,
            ) {
                result.push_str(&number.to_string());
            } else {
                result.push_str(&number_str);
            }
            i = number_end;
        } else if ch == '点' {
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
                after_decimal = true;
            } else {
                result.push(ch);
                after_decimal = false;
            }
            i += 1;
        } else if chinese_digit_chars.contains(&ch) {
            if after_decimal {
                if let Ok(number) =
                    <String as chinese_number::ChineseToNumber<u64>>::to_number_naive(&ch.to_string())
                {
                    result.push_str(&number.to_string());
                } else {
                    result.push(ch);
                }
                i += 1;
            } else {
                let mut number_end = i + 1;
                while number_end < chars.len() && chinese_digit_chars.contains(&chars[number_end]) {
                    number_end += 1;
                }

                let number_str: String = chars[i..number_end].iter().collect();
                if let Ok(number) = <String as chinese_number::ChineseToNumber<u64>>::to_number(
                    &number_str,
                    chinese_number::ChineseCountMethod::TenThousand,
                ) {
                    result.push_str(&number.to_string());
                } else {
                    result.push_str(&number_str);
                }
                i = number_end;
            }
        } else {
            result.push(ch);
            after_decimal = false;
            i += 1;
        }
    }

    result
}

fn main() {
    let input = "主要针对的平台是叉八六杠六十四二十六十四，还有power P C64。";
    let output = chinese_numbers_to_primitive_numbers(input);
    println!("输入: {}", input);
    println!("输出: {}", output);
}
