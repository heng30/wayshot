use std::{collections::HashMap, sync::LazyLock};

use ndarray::{ArrayView, IxDyn};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PolyChar {
    pub index: usize,
    pub phones: Vec<(String, usize)>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MonoChar {
    pub phone: String,
}

pub static MONO_CHARS_DIST_STR: &str = include_str!("dict_mono_chars.json");
pub static POLY_CHARS_DIST_STR: &str = include_str!("dict_poly_chars.json");
pub static DEFAULT_ZH_WORD_DICT: &str = include_str!("zh_word_dict.json");
pub static BERT_TOKENIZER: &str = include_str!("g2pw_tokenizer.json");

/// Helper to load dictionary content from file or default
fn load_dict_content(filename: &str, default_content: &str) -> String {
    if let Ok(dir) = std::env::var("G2PW_DIST_DIR") {
        let path = std::path::Path::new(&dir).join(filename);
        if path.is_file() {
            return std::fs::read_to_string(&path)
                .unwrap_or_else(|_| panic!("{} not found", filename));
        }
    }
    default_content.to_string()
}

pub fn load_mono_chars() -> HashMap<char, MonoChar> {
    let content = load_dict_content("dict_mono_chars.json", MONO_CHARS_DIST_STR);
    serde_json::from_str(&content).expect("dict_mono_chars.json parse error")
}

pub fn load_poly_chars() -> HashMap<char, PolyChar> {
    let content = load_dict_content("dict_poly_chars.json", POLY_CHARS_DIST_STR);
    serde_json::from_str(&content).expect("dict_poly_chars.json parse error")
}

pub static DICT_MONO_CHARS: LazyLock<HashMap<char, MonoChar>> = LazyLock::new(load_mono_chars);
pub static DICT_POLY_CHARS: LazyLock<HashMap<char, PolyChar>> = LazyLock::new(load_poly_chars);

pub fn str_is_chinese(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| DICT_MONO_CHARS.contains_key(&c) || DICT_POLY_CHARS.contains_key(&c))
}

// Finds the index of the maximum value in a 2D tensor
pub fn argmax_2d(tensor: &ArrayView<f32, IxDyn>) -> (usize, usize) {
    let cols = tensor.shape()[1];

    tensor
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(idx, _)| (idx / cols, idx % cols))
        .unwrap_or((0, 0))
}
