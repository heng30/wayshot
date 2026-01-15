use log::{debug, warn};
use regex::Regex;
use std::{collections::HashMap, sync::LazyLock};

static TONED: LazyLock<HashMap<u32, String>> = LazyLock::new(load_data);
static TONELESS: LazyLock<HashMap<u32, String>> = LazyLock::new(load_data);

static DICT: LazyLock<HashMap<String, Vec<String>>> = LazyLock::new(load_dict);

fn match_indices_by_chars(text: &str, pattern: &str) -> Vec<(usize, String)> {
    // Get all matches with byte indices
    let matches = text.match_indices(pattern);
    // Map byte indices to char indices
    let mut result = Vec::new();
    for (byte_idx, matched) in matches {
        // Count characters up to the byte index
        let char_idx = text[..byte_idx].chars().count();
        result.push((char_idx, matched.to_string()));
    }
    result
}

fn load_dict() -> HashMap<String, Vec<String>> {
    let content = include_str!("jyut6ping3.words.dict.yaml");
    // Split content at "..." to separate YAML metadata and word list
    let parts: Vec<&str> = content.split("...").collect();

    // Parse the YAML metadata (first part)
    // let yaml_part = parts[0].trim();

    // Process the word list (second part)
    let word_lines = parts[1].trim().lines();
    let mut res = HashMap::new();

    for line in word_lines {
        // Skip empty lines or comments
        if line.trim().is_empty() || line.trim().starts_with('#') {
            continue;
        }
        // Split each line by tab
        let columns: Vec<&str> = line.split('\t').collect();
        if columns.len() == 2 {
            // res
            let mut t: Vec<String> = columns[1].split(" ").map(|v| v.to_string()).collect();
            let matches = match_indices_by_chars(columns[0], "，");
            for m in matches {
                t.insert(m.0, m.1);
                // println!("{:?}, {}", t, columns[0]);
            }
            let matches = match_indices_by_chars(columns[0], "：");
            for m in matches {
                t.insert(m.0, m.1);
            }
            if columns[0].chars().count() != t.len() {
                // todo: fix in future
                // warn!(
                //     "char and jyutping size not match in dict, {} vs {:?}",
                //     columns[0], t
                // );
            }
            res.insert(columns[0].to_owned(), t);
            // debug!("{}: {}",columns[0].to_owned(), columns[1].to_owned())
        } else {
            warn!("Skipping malformed line: {}", line);
        }
    }
    res
}

fn load_data() -> HashMap<u32, String> {
    let mut toned = HashMap::new();
    let mut toneless = HashMap::new();
    let contents = include_str!("jyutping_dictionary.json");

    let table: HashMap<String, String> =
        serde_json::from_str(contents).expect("Failed to parse JSON");

    for (code, jyutping) in table {
        let jyutping = jyutping.split_whitespace().next().unwrap_or("").to_string();
        let code_int = u32::from_str_radix(&code, 16).expect("Failed to parse code as u32");
        toned.insert(code_int, format!(" {} ", jyutping));
        toneless.insert(
            code_int,
            format!(" {} ", &jyutping[..jyutping.len() - 1]),
        );
    }

    // Return toned for TONED and toneless for TONELESS
    if std::env::var("TONELESS").is_ok() {
        toneless
    } else {
        toned
    }
}

pub fn get_jyutping(text: &str, tone: bool) -> String {
    if text.is_empty() {
        return String::new();
    }

    let re = Regex::new(r"[\u{4e00}-\u{9fff}]+")
        .map_err(|e| format!("Regex error: {}", e))
        .unwrap();
    if !re.is_match(text) {
        return text.to_string();
    }

    let dict = if tone { &*TONED } else { &*TONELESS };
    let mut converted = String::new();

    for c in text.chars() {
        let code = c as u32;
        if let Some(jyutping) = dict.get(&code) {
            converted.push_str(jyutping);
        } else {
            converted.push(c);
        }
    }

    // Join and trim multiple spaces
    let result: String = converted
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ");
    result
}

pub fn get_jyutping_list(text: &str) -> Vec<(String, String)> {
    let mut res = Vec::new();
    let mut i = 0;
    let chars: Vec<char> = text.chars().collect();

    while i < chars.len() {
        let mut found = false;

        // Try to match the longest dictionary word starting at position i
        for len in (1..=chars.len() - i).rev() {
            let slice: String = chars[i..i + len].iter().collect();
            if let Some(jyutping) = DICT.get(&slice) {
                debug!("use dict: {}: {:?}", slice, jyutping);
                let text_single: Vec<String> = slice.chars().map(|v| v.to_string()).collect();
                if jyutping.len() != text_single.len() {
                    warn!("char and jyutping size not match in dict, fallback to single character");
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

        // If no dictionary match, use get_jyutping for single character
        if !found {
            let ch: String = chars[i].to_string();
            let jyutping = get_jyutping(&ch, false);
            res.push((ch, jyutping));
            i += 1;
        }
    }

    res
}
