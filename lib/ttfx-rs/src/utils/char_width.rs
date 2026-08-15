//! Character cell width (wcwidth): how many terminal columns a character
//! occupies. CJK/full-width glyphs render twice as wide as ASCII cells, so
//! canvas sizing and text anchoring must account for them.

/// Number of terminal columns `c` occupies (1 or 2).
pub fn char_cell_width(c: char) -> i64 {
    let cp = c as u32;
    if (0x1100..=0x115F).contains(&cp) // Hangul Jamo
        || (0x2E80..=0xA4CF).contains(&cp) // CJK Radicals .. Yi
        || (0xAC00..=0xD7A3).contains(&cp) // Hangul Syllables
        || (0xF900..=0xFAFF).contains(&cp) // CJK Compatibility Ideographs
        || (0xFE30..=0xFE4F).contains(&cp) // CJK Compatibility Forms
        || (0xFF00..=0xFF60).contains(&cp) // Fullwidth Forms
        || (0xFFE0..=0xFFE6).contains(&cp) // Fullwidth Signs
        || (0x1F300..=0x1F64F).contains(&cp) // Emoji
        || (0x1F900..=0x1F9FF).contains(&cp) // Supplemental Symbols
        || (0x20000..=0x2FFFD).contains(&cp) // CJK Ext B..
    {
        2
    } else {
        1
    }
}

/// Width of the first character of `s`.
pub fn first_char_width(s: &str) -> i64 {
    char_cell_width(s.chars().next().unwrap_or(' '))
}
