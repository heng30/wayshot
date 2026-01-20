use regex::Regex;
use std::{collections::HashMap, sync::LazyLock};

static JYUTPING_DICTIONARY_JSON: &str = include_str!("../../../asset/jyutping_dictionary.json");
static JYUT6PING3_WORDS_DICT_YAML: &str = include_str!("../../../asset/jyut6ping3.words.dict.yaml");

static JYUTPING_DICTIONARY: LazyLock<HashMap<u32, String>> =
    LazyLock::new(load_jyutping_dictionary_json);

static JYUT6PING3_WORDS_DICT: LazyLock<HashMap<String, Vec<String>>> =
    LazyLock::new(load_jyut6ping3_words_dict_yaml);

pub fn get_jyutping_list(text: &str) -> Vec<(String, String)> {
    let mut res = Vec::new();
    let mut i = 0;
    let chars: Vec<char> = text.chars().collect();

    while i < chars.len() {
        let mut found = false;

        // Try to match the longest dictionary word starting at position i
        for len in (1..=chars.len() - i).rev() {
            let slice: String = chars[i..i + len].iter().collect();

            if let Some(jyutping) = JYUT6PING3_WORDS_DICT.get(&slice) {
                let text_single: Vec<String> = slice.chars().map(|v| v.to_string()).collect();

                if jyutping.len() != text_single.len() {
                    log::warn!(
                        "char and jyutping size not match in dict, fallback to single character"
                    );
                    i += len;
                } else {
                    for (ii, jy) in jyutping.iter().enumerate() {
                        res.push((text_single[ii].clone(), jy.to_string()));
                    }

                    i += len;
                    found = true;
                }
                break;
            }
        }

        if !found {
            let ch: String = chars[i].to_string();
            let jyutping = get_jyutping(&ch);
            res.push((ch, jyutping));
            i += 1;
        }
    }

    res
}

fn load_jyutping_dictionary_json() -> HashMap<u32, String> {
    let mut dict = HashMap::new();
    let table: HashMap<String, String> =
        serde_json::from_str(JYUTPING_DICTIONARY_JSON).expect("Failed to parse JSON");

    for (code, jyutping) in table {
        let jyutping = jyutping.split_whitespace().next().unwrap_or("").to_string();
        let code_int = u32::from_str_radix(&code, 16).expect("Failed to parse code as u32");
        dict.insert(code_int, format!(" {} ", jyutping));
    }

    dict
}

fn load_jyut6ping3_words_dict_yaml() -> HashMap<String, Vec<String>> {
    let parts: Vec<&str> = JYUT6PING3_WORDS_DICT_YAML.split("...").collect();
    let word_lines = parts[1].trim().lines();
    let mut res = HashMap::new();

    for line in word_lines {
        if line.trim().is_empty() || line.trim().starts_with('#') {
            continue;
        }

        let columns: Vec<&str> = line.split('\t').collect();
        if columns.len() == 2 {
            let mut t: Vec<String> = columns[1].split(" ").map(|v| v.to_string()).collect();
            let matches = match_indices_by_chars(columns[0], "，");
            for m in matches {
                t.insert(m.0, m.1);
            }

            let matches = match_indices_by_chars(columns[0], "：");
            for m in matches {
                t.insert(m.0, m.1);
            }

            if columns[0].chars().count() != t.len() {
                log::warn!(
                    "char and jyutping size not match in dict, {} vs {:?}",
                    columns[0],
                    t
                );
            } else {
                res.insert(columns[0].to_owned(), t);
            }
        } else {
            log::warn!("Skipping malformed line: {}", line);
        }
    }
    res
}

fn match_indices_by_chars(text: &str, pattern: &str) -> Vec<(usize, String)> {
    let matches = text.match_indices(pattern);
    let mut result = Vec::new();
    for (byte_idx, matched) in matches {
        let char_idx = text[..byte_idx].chars().count();
        result.push((char_idx, matched.to_string()));
    }
    result
}

fn get_jyutping(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }

    let re = Regex::new(r"[\u{4e00}-\u{9fff}]+")
        .map_err(|e| format!("Regex error: {}", e))
        .unwrap();

    if !re.is_match(text) {
        return text.to_string();
    }

    let mut converted = String::new();
    for c in text.chars() {
        if let Some(jyutping) = JYUTPING_DICTIONARY.get(&(c as u32)) {
            converted.push_str(jyutping);
        } else {
            converted.push(c);
        }
    }

    converted
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ")
}
