use std::{collections::HashMap, sync::LazyLock};

static SYMBOLS_V2: &str = include_str!("symbols_v2.json");

pub static SYMBOLS: LazyLock<HashMap<String, i64>> = LazyLock::new(|| {
    let mut symbols: HashMap<String, i64> = serde_json::from_str(SYMBOLS_V2).unwrap();
    symbols.insert(" ".to_string(), symbols["\u{7a7a}"]);
    symbols.insert("'".to_string(), symbols["-"]);
    symbols
});

#[inline]
pub fn get_phone_symbol(ph: &str) -> i64 {
    SYMBOLS.get(ph).copied().unwrap_or(3)
}
