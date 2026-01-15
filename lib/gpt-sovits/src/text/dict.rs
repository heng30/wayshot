use std::{collections::HashMap, path::PathBuf, sync::LazyLock};

use crate::text::utils::*;

/// Helper to load dictionary from file or use default
fn load_dict(filename: &str, default_content: Option<&str>) -> HashMap<String, Vec<String>> {
    let word_dict_path = std::env::var("GPT_SOVITS_DICT_PATH").unwrap_or_else(|_| ".".to_string());
    let path = PathBuf::from(word_dict_path).join(filename);
    if path.is_file() {
        let content = std::fs::read_to_string(path).unwrap();
        serde_json::from_str(&content).unwrap()
    } else if let Some(default) = default_content {
        serde_json::from_str(default).unwrap()
    } else {
        HashMap::default()
    }
}

static ZN_DICT: LazyLock<HashMap<String, Vec<String>>> =
    LazyLock::new(|| load_dict("zh_word_dict.json", Some(DEFAULT_ZH_WORD_DICT)));

static EN_DICT: LazyLock<HashMap<String, Vec<String>>> =
    LazyLock::new(|| load_dict("en_word_dict.json", None));

pub fn zh_word_dict(word: &str) -> Option<&'static [String]> {
    ZN_DICT.get(word).map(|s| s.as_slice())
}

pub fn en_word_dict(word: &str) -> Option<&'static [String]> {
    EN_DICT.get(word).map(|s| s.as_slice())
}
