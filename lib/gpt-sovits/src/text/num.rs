use crate::{GSVError, Result, text::Lang};
use pest::Parser;

static NUM_OP: [char; 8] = ['+', '-', '*', '×', '/', '÷', '=', '%'];

#[derive(pest_derive::Parser)]
#[grammar = "asset/rule.pest"]
pub struct ExprParser;

pub mod zh {
    use super::*;
    use pest::iterators::Pair;

    static UNITS: [&str; 4] = ["", "十", "百", "千"];
    static BASE_UNITS: [&str; 4] = ["", "万", "亿", "万"];

    pub fn parse_all(pair: Pair<Rule>, dst_string: &mut String) -> Result<()> {
        assert_eq!(pair.as_rule(), Rule::all);
        let inner = pair.into_inner();
        for pair in inner {
            match pair.as_rule() {
                Rule::signs => parse_signs(pair, dst_string)?,
                Rule::ident => parse_ident(pair, dst_string)?,
                _ => return Err(GSVError::UnknownRuleAll(pair.as_str().to_owned())),
            }
        }
        Ok(())
    }

    fn parse_pn(pair: Pair<Rule>, dst_string: &mut String) -> Result<()> {
        assert_eq!(pair.as_rule(), Rule::pn);
        match pair.as_str() {
            "+" => dst_string.push('加'),
            "-" => dst_string.push('减'),
            "*" | "×" => dst_string.push('乘'),
            "/" | "÷" => dst_string.push_str("除以"),
            "=" => dst_string.push_str("等于"),
            _ => return Err(GSVError::UnknownOperator(pair.as_str().to_owned())),
        }
        Ok(())
    }

    fn parse_flag(pair: Pair<Rule>, dst_string: &mut String) -> Result<()> {
        assert_eq!(pair.as_rule(), Rule::flag);
        match pair.as_str() {
            "+" => dst_string.push('正'),
            "-" => dst_string.push('负'),
            _ => return Err(GSVError::UnknownFlag(pair.as_str().to_owned())),
        }
        Ok(())
    }

    fn parse_percent(pair: Pair<Rule>, dst_string: &mut String) -> Result<()> {
        assert_eq!(pair.as_rule(), Rule::percent);
        dst_string.push_str("百分之");
        for pair in pair.into_inner() {
            match pair.as_rule() {
                Rule::decimals => parse_decimals(pair, dst_string)?,
                Rule::integer => parse_integer(pair, dst_string, true)?,
                _ => return Err(GSVError::UnknownRuleInPercent(pair.as_str().to_owned())),
            }
        }
        Ok(())
    }

    fn parse_integer(pair: Pair<Rule>, dst_string: &mut String, unit: bool) -> Result<()> {
        assert_eq!(pair.as_rule(), Rule::integer);

        let digits: Vec<_> = pair.into_inner().collect();
        let mut result = String::new();
        let mut has_non_zero = false;

        for (i, pair) in digits.iter().enumerate() {
            let txt = digit_to_zh(pair.as_str().chars().next().unwrap())?;
            let pos = digits.len() - 1 - i;
            let u = if pos % 4 != 0 {
                UNITS[pos % 4]
            } else {
                BASE_UNITS[(pos / 4) % 4]
            };

            if txt != "零" {
                has_non_zero = true;
                if !(pos == 1 && txt == "一") {
                    result.push_str(txt);
                }
                if unit {
                    result.push_str(u);
                }
            } else if has_non_zero && unit && pos > 0 {
                result.push_str(txt);
            }
        }

        if result.is_empty() {
            dst_string.push('零');
        } else {
            if result.ends_with('零') {
                result.truncate(result.len() - 1);
            }
            dst_string.push_str(&result);
        }

        Ok(())
    }

    fn parse_decimals(pair: Pair<Rule>, dst_string: &mut String) -> Result<()> {
        assert_eq!(pair.as_rule(), Rule::decimals);

        let parts: Vec<_> = pair.into_inner().collect();
        for (i, part) in parts.iter().enumerate() {
            if part.as_rule() == Rule::integer {
                let digits: Vec<_> = part.clone().into_inner().collect();
                for digit_pair in digits {
                    let txt = digit_to_zh(digit_pair.as_str().chars().next().unwrap())?;
                    dst_string.push_str(txt);
                }
                if i < parts.len() - 1 {
                    dst_string.push('点');
                }
            }
        }

        Ok(())
    }

    fn parse_fractional(pair: Pair<Rule>, dst_string: &mut String) -> Result<()> {
        assert_eq!(pair.as_rule(), Rule::fractional);

        let mut inner = pair.into_inner();
        let numerator = inner.next().unwrap();
        let denominator = inner.next().unwrap();
        parse_integer(denominator, dst_string, true)?;
        dst_string.push_str("分之");
        parse_integer(numerator, dst_string, true)?;
        Ok(())
    }

    fn parse_num(pair: Pair<Rule>, dst_string: &mut String) -> Result<()> {
        assert_eq!(pair.as_rule(), Rule::num);

        let inner = pair.into_inner();
        for pair in inner {
            match pair.as_rule() {
                Rule::flag => parse_flag(pair, dst_string)?,
                Rule::percent => parse_percent(pair, dst_string)?,
                Rule::decimals => parse_decimals(pair, dst_string)?,
                Rule::fractional => parse_fractional(pair, dst_string)?,
                Rule::integer => parse_integer(pair, dst_string, true)?,
                _ => return Err(GSVError::UnknownRuleInNum(pair.as_str().to_owned())),
            }
        }
        Ok(())
    }

    fn parse_signs(pair: Pair<Rule>, dst_string: &mut String) -> Result<()> {
        assert_eq!(pair.as_rule(), Rule::signs);

        let inner = pair.into_inner();
        for pair in inner {
            match pair.as_rule() {
                Rule::num => parse_num(pair, dst_string)?,
                Rule::pn => parse_pn(pair, dst_string)?,
                Rule::word => log::warn!("word: {}", pair.as_str()),
                _ => return Err(GSVError::UnknownRuleInSigns(pair.as_str().to_owned())),
            }
        }
        Ok(())
    }

    fn parse_link(pair: Pair<Rule>, dst_string: &mut String) -> Result<()> {
        assert_eq!(pair.as_rule(), Rule::link);
        if pair.as_str() == "-" {
            dst_string.push('杠');
        }
        Ok(())
    }

    fn parse_word(pair: Pair<Rule>, dst_string: &mut String) -> Result<()> {
        assert_eq!(pair.as_rule(), Rule::word);
        let inner = pair.into_inner();
        for pair in inner {
            match pair.as_rule() {
                Rule::digit => {
                    let txt = digit_to_zh(pair.as_str().chars().next().unwrap())?;
                    dst_string.push_str(txt);
                }
                Rule::alpha => {
                    dst_string.push_str(pair.as_str());
                }
                Rule::greek => {
                    let txt = greek_to_zh(pair.as_str().chars().next().unwrap())?;
                    dst_string.push_str(txt);
                }
                _ => return Err(GSVError::UnknownRuleWord(pair.as_str().to_owned())),
            }
        }
        Ok(())
    }

    fn parse_ident(pair: Pair<Rule>, dst_string: &mut String) -> Result<()> {
        assert_eq!(pair.as_rule(), Rule::ident);
        let inner = pair.into_inner();
        for pair in inner {
            match pair.as_rule() {
                Rule::word => parse_word(pair, dst_string)?,
                Rule::link => parse_link(pair, dst_string)?,
                _ => return Err(GSVError::UnknownRuleIdent(pair.as_str().to_owned())),
            }
        }
        Ok(())
    }
}

pub mod en {
    use {super::*, pest::iterators::Pair};

    pub fn parse_all(pair: Pair<Rule>, dst_string: &mut String) -> Result<()> {
        assert_eq!(pair.as_rule(), Rule::all);
        let inner = pair.into_inner();
        for pair in inner {
            match pair.as_rule() {
                Rule::signs => parse_signs(pair, dst_string)?,
                Rule::ident => parse_ident(pair, dst_string)?,
                _ => return Err(GSVError::UnknownRuleAll(pair.as_str().to_owned())),
            }
        }

        Ok(())
    }

    #[inline]
    fn add_separator(dst_string: &mut String) {
        if !dst_string.is_empty() {
            dst_string.push_str(" ");
        }
    }

    fn parse_pn(pair: Pair<Rule>, dst_string: &mut String) -> Result<()> {
        assert_eq!(pair.as_rule(), Rule::pn);
        add_separator(dst_string);
        match pair.as_str() {
            "+" => dst_string.push_str("plus"),
            "-" => dst_string.push_str("minus"),
            "*" | "×" => dst_string.push_str("times"),
            "/" | "÷" => dst_string.push_str("divided by"),
            "=" => dst_string.push_str("is"),
            _ => return Err(GSVError::UnknownOperator(pair.as_str().to_owned())),
        }
        Ok(())
    }

    fn parse_flag(pair: Pair<Rule>, dst_string: &mut String) -> Result<()> {
        assert_eq!(pair.as_rule(), Rule::flag);
        add_separator(dst_string);
        match pair.as_str() {
            "-" => dst_string.push_str("negative"),
            _ => return Err(GSVError::UnknownFlag(pair.as_str().to_owned())),
        }
        Ok(())
    }

    fn parse_percent(pair: Pair<Rule>, dst_string: &mut String) -> Result<()> {
        assert_eq!(pair.as_rule(), Rule::percent);
        let inner = pair.into_inner();
        for pair in inner {
            match pair.as_rule() {
                Rule::decimals => parse_decimals(pair, dst_string)?,
                Rule::integer => parse_integer(pair, dst_string, true)?,
                _ => return Err(GSVError::UnknownRuleInPercent(pair.as_str().to_owned())),
            }
        }
        add_separator(dst_string);
        dst_string.push_str("percent");
        Ok(())
    }

    fn parse_integer(pair: Pair<Rule>, dst_string: &mut String, unit: bool) -> Result<()> {
        assert_eq!(pair.as_rule(), Rule::integer);
        add_separator(dst_string);

        let digits: Vec<_> = pair.into_inner().collect();
        for pair in digits {
            let txt = digit_to_en(pair.as_str().chars().next().unwrap())?;
            dst_string.push_str(txt);
            if unit {
                add_separator(dst_string);
            }
        }
        Ok(())
    }

    fn parse_decimals(pair: Pair<Rule>, dst_string: &mut String) -> Result<()> {
        assert_eq!(pair.as_rule(), Rule::decimals);

        let parts: Vec<_> = pair.into_inner().collect();
        let mut first_integer = true;
        for (_i, part) in parts.iter().enumerate() {
            if part.as_rule() == Rule::integer {
                if !first_integer {
                    dst_string.push_str(" point ");
                }
                first_integer = false;

                let digits: Vec<_> = part.clone().into_inner().collect();
                for (j, digit_pair) in digits.iter().enumerate() {
                    if j > 0 {
                        dst_string.push(' ');
                    }
                    let txt = digit_to_en(digit_pair.as_str().chars().next().unwrap())?;
                    dst_string.push_str(txt);
                }
            }
        }

        Ok(())
    }

    fn parse_fractional(pair: Pair<Rule>, dst_string: &mut String) -> Result<()> {
        assert_eq!(pair.as_rule(), Rule::fractional);
        let mut inner = pair.into_inner();
        let numerator = inner.next().unwrap();
        let denominator = inner.next().unwrap();
        parse_integer(numerator, dst_string, true)?;
        add_separator(dst_string);
        dst_string.push_str("over");
        add_separator(dst_string);
        parse_integer(denominator, dst_string, true)?;
        Ok(())
    }

    fn parse_num(pair: Pair<Rule>, dst_string: &mut String) -> Result<()> {
        assert_eq!(pair.as_rule(), Rule::num);
        let inner = pair.into_inner();
        for pair in inner {
            match pair.as_rule() {
                Rule::flag => parse_flag(pair, dst_string)?,
                Rule::percent => parse_percent(pair, dst_string)?,
                Rule::decimals => parse_decimals(pair, dst_string)?,
                Rule::fractional => parse_fractional(pair, dst_string)?,
                Rule::integer => parse_integer(pair, dst_string, true)?,
                _ => return Err(GSVError::UnknownRuleInNum(pair.as_str().to_owned())),
            }
        }
        Ok(())
    }

    fn parse_signs(pair: Pair<Rule>, dst_string: &mut String) -> Result<()> {
        assert_eq!(pair.as_rule(), Rule::signs);
        let inner = pair.into_inner();
        for pair in inner {
            match pair.as_rule() {
                Rule::num => parse_num(pair, dst_string)?,
                Rule::pn => parse_pn(pair, dst_string)?,
                Rule::word => {}
                _ => return Err(GSVError::UnknownRuleInSigns(pair.as_str().to_owned())),
            }
        }
        Ok(())
    }

    fn parse_link(pair: Pair<Rule>) -> Result<()> {
        assert_eq!(pair.as_rule(), Rule::link);
        Ok(())
    }

    fn parse_word(pair: Pair<Rule>, dst_string: &mut String) -> Result<()> {
        assert_eq!(pair.as_rule(), Rule::word);
        let inner = pair.into_inner();
        for pair in inner {
            match pair.as_rule() {
                Rule::digit => {
                    let txt = digit_to_en(pair.as_str().chars().next().unwrap())?;
                    add_separator(dst_string);
                    dst_string.push_str(txt);
                }
                Rule::alpha => {
                    add_separator(dst_string);
                    dst_string.push_str(pair.as_str());
                }
                Rule::greek => {
                    let txt = greek_to_en(pair.as_str().chars().next().unwrap())?;
                    add_separator(dst_string);
                    dst_string.push_str(txt);
                }
                _ => return Err(GSVError::UnknownRuleWord(pair.as_str().to_owned())),
            }
        }
        Ok(())
    }

    fn parse_ident(pair: Pair<Rule>, dst_string: &mut String) -> Result<()> {
        assert_eq!(pair.as_rule(), Rule::ident);
        let inner = pair.into_inner();
        for pair in inner {
            match pair.as_rule() {
                Rule::word => parse_word(pair, dst_string)?,
                Rule::link => parse_link(pair)?,
                _ => return Err(GSVError::UnknownRuleIdent(pair.as_str().to_owned())),
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct NumSentence {
    pub text: String,
    pub lang: Lang,
}

impl NumSentence {
    pub fn need_drop(&self) -> bool {
        let num_text = self.text.trim();
        num_text.is_empty() || num_text.chars().all(|c| NUM_OP.contains(&c))
    }

    pub fn is_link_symbol(&self) -> bool {
        self.text == "-"
    }

    pub fn to_lang_text(&self) -> Result<String> {
        let mut dst_string = String::new();
        let pairs = ExprParser::parse(Rule::all, &self.text)?;
        for pair in pairs {
            match self.lang {
                Lang::Zh => zh::parse_all(pair, &mut dst_string)?,
                Lang::En => en::parse_all(pair, &mut dst_string)?,
            }
        }
        Ok(dst_string.trim().to_string())
    }
}

pub fn is_numeric(p: &str) -> bool {
    if p.to_lowercase().contains([
        'α', 'β', 'γ', 'δ', 'ε', 'ζ', 'η', 'θ', 'ι', 'κ', 'λ', 'μ', 'ν', 'ξ', 'ο', 'π', 'ρ', 'σ',
        'ς', 'τ', 'υ', 'φ', 'χ', 'ψ', 'ω',
    ]) {
        return true;
    }

    if p.chars().any(|c| c.is_numeric()) {
        return true;
    }

    // This prevents words like "cross-platform" from being classified as numeric
    if p.contains(NUM_OP) && p.chars().any(|c| c.is_numeric()) {
        return true;
    }

    false
}

#[inline]
fn digit_to_zh(c: char) -> Result<&'static str> {
    match c {
        '0' => Ok("零"),
        '1' => Ok("一"),
        '2' => Ok("二"),
        '3' => Ok("三"),
        '4' => Ok("四"),
        '5' => Ok("五"),
        '6' => Ok("六"),
        '7' => Ok("七"),
        '8' => Ok("八"),
        '9' => Ok("九"),
        _ => Err(GSVError::UnknownDigit(c.to_string())),
    }
}

#[inline]
fn digit_to_en(c: char) -> Result<&'static str> {
    match c {
        '0' => Ok("zero"),
        '1' => Ok("one"),
        '2' => Ok("two"),
        '3' => Ok("three"),
        '4' => Ok("four"),
        '5' => Ok("five"),
        '6' => Ok("six"),
        '7' => Ok("seven"),
        '8' => Ok("eight"),
        '9' => Ok("nine"),
        _ => Err(GSVError::UnknownDigit(c.to_string())),
    }
}

fn greek_to_zh(c: char) -> Result<&'static str> {
    match c {
        'α' | 'Α' => Ok("阿尔法"),
        'β' | 'Β' => Ok("贝塔"),
        'γ' | 'Γ' => Ok("伽马"),
        'δ' | 'Δ' => Ok("德尔塔"),
        'ε' | 'Ε' => Ok("艾普西龙"),
        'ζ' | 'Ζ' => Ok("泽塔"),
        'η' | 'Η' => Ok("艾塔"),
        'θ' | 'Θ' => Ok("西塔"),
        'ι' | 'Ι' => Ok("约塔"),
        'κ' | 'Κ' => Ok("卡帕"),
        'λ' | 'Λ' => Ok("兰姆达"),
        'μ' | 'Μ' => Ok("缪"),
        'ν' | 'Ν' => Ok("纽"),
        'ξ' | 'Ξ' => Ok("克西"),
        'ο' | 'Ο' => Ok("欧米克戈"),
        'π' | 'Π' => Ok("派"),
        'ρ' | 'Ρ' => Ok("罗"),
        'σ' | 'Σ' => Ok("西格玛"),
        'τ' | 'Τ' => Ok("套"),
        'υ' | 'Υ' => Ok("宇普西龙"),
        'φ' | 'Φ' => Ok("斐"),
        'χ' | 'Χ' => Ok("希"),
        'ψ' | 'Ψ' => Ok("普西"),
        'ω' | 'Ω' => Ok("欧米伽"),
        _ => Err(GSVError::UnknownGreekLetter(c.to_string())),
    }
}

fn greek_to_en(c: char) -> Result<&'static str> {
    match c {
        'α' | 'Α' => Ok("alpha"),
        'β' | 'Β' => Ok("beta"),
        'γ' | 'Γ' => Ok("gamma"),
        'δ' | 'Δ' => Ok("delta"),
        'ε' | 'Ε' => Ok("epsilon"),
        'ζ' | 'Ζ' => Ok("zeta"),
        'η' | 'Η' => Ok("eta"),
        'θ' | 'Θ' => Ok("theta"),
        'ι' | 'Ι' => Ok("iota"),
        'κ' | 'Κ' => Ok("kappa"),
        'λ' | 'Λ' => Ok("lambda"),
        'μ' | 'Μ' => Ok("mu"),
        'ν' | 'Ν' => Ok("nu"),
        'ξ' | 'Ξ' => Ok("xi"),
        'ο' | 'Ο' => Ok("omicron"),
        'π' | 'Π' => Ok("pi"),
        'ρ' | 'Ρ' => Ok("rho"),
        'σ' | 'Σ' => Ok("sigma"),
        'τ' | 'Τ' => Ok("tau"),
        'υ' | 'Υ' => Ok("upsilon"),
        'φ' | 'Φ' => Ok("phi"),
        'χ' | 'Χ' => Ok("chi"),
        'ψ' | 'Ψ' => Ok("psi"),
        'ω' | 'Ω' => Ok("omega"),
        _ => Err(GSVError::UnknownGreekLetter(c.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_number_zh() {
        let num = NumSentence {
            text: "1.12.3".to_string(),
            lang: Lang::Zh,
        };
        let result = num.to_lang_text().unwrap();
        assert_eq!(result, "一点一二点三");
    }

    #[test]
    fn test_version_number_en() {
        let num = NumSentence {
            text: "1.12.3".to_string(),
            lang: Lang::En,
        };
        let result = num.to_lang_text().unwrap();
        assert_eq!(result, "one point one two point three");
    }

    #[test]
    fn test_simple_decimal_zh() {
        let num = NumSentence {
            text: "3.14".to_string(),
            lang: Lang::Zh,
        };
        let result = num.to_lang_text().unwrap();
        assert_eq!(result, "三点一四");
    }

    #[test]
    fn test_simple_decimal_en() {
        let num = NumSentence {
            text: "3.14".to_string(),
            lang: Lang::En,
        };
        let result = num.to_lang_text().unwrap();
        assert_eq!(result, "three point one four");
    }

    #[test]
    fn test_complex_version_zh() {
        let num = NumSentence {
            text: "2.0.1".to_string(),
            lang: Lang::Zh,
        };
        let result = num.to_lang_text().unwrap();
        assert_eq!(result, "二点零点一");
    }
}
