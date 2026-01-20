use ndarray::{ArrayView, IxDyn};
use std::{collections::HashMap, sync::LazyLock};

pub static MONO_CHARS_DIST_STR: &str = include_str!("../../asset/dict_mono_chars.json");
pub static POLY_CHARS_DIST_STR: &str = include_str!("../../asset/dict_poly_chars.json");
pub static DEFAULT_ZH_WORD_DICT: &str = include_str!("../../asset/zh_word_dict.json");
pub static BERT_TOKENIZER: &str = include_str!("../../asset/g2pw_tokenizer.json");

pub static DICT_MONO_CHARS: LazyLock<HashMap<char, MonoChar>> = LazyLock::new(|| {
    serde_json::from_str(MONO_CHARS_DIST_STR).expect("dict_mono_chars.json parse error")
});
pub static DICT_POLY_CHARS: LazyLock<HashMap<char, PolyChar>> = LazyLock::new(|| {
    serde_json::from_str(POLY_CHARS_DIST_STR).expect("dict_poly_chars.json parse error")
});
static ZN_DICT: LazyLock<HashMap<String, Vec<String>>> =
    LazyLock::new(|| serde_json::from_str(DEFAULT_ZH_WORD_DICT).unwrap());
static EN_DICT: LazyLock<HashMap<String, Vec<String>>> = LazyLock::new(|| HashMap::default());

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MonoChar {
    pub phone: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PolyChar {
    pub index: usize,
    pub phones: Vec<(String, usize)>,
}

pub fn str_is_chinese(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| DICT_MONO_CHARS.contains_key(&c) || DICT_POLY_CHARS.contains_key(&c))
}

pub fn argmax_2d(tensor: &ArrayView<f32, IxDyn>) -> (usize, usize) {
    let cols = tensor.shape()[1];

    tensor
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(idx, _)| (idx / cols, idx % cols))
        .unwrap_or((0, 0))
}

pub fn zh_word_dict(word: &str) -> Option<&'static [String]> {
    ZN_DICT.get(word).map(|s| s.as_slice())
}
pub fn en_word_dict(word: &str) -> Option<&'static [String]> {
    EN_DICT.get(word).map(|s| s.as_slice())
}
