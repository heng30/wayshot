//! LOC token parser for PaddleOCR-VL spotting task
//!
//! This module handles parsing of location (LOC) tokens from the spotting task output.
//! The spotting task outputs text with bounding box coordinates encoded as special tokens.
//!
//! ## LOC Token Format
//! - `<|LOC_0|>` to `<|LOC_999|>` (100297-101296) - Coordinate values (0-999, representing thousandths)
//! - `<|LOC_BEGIN|>` (101298) - Start of location sequence (when present)
//! - `<|LOC_END|>` (101299) - End of location sequence (when present)
//! - `<|LOC_SEP|>` (101300) - Separator between coordinates (when present)

use regex::Regex;

use crate::Error;

/// LOC token ID constants
pub const LOC_BEGIN_TOKEN: u32 = 101298;
pub const LOC_END_TOKEN: u32 = 101299;
pub const LOC_SEP_TOKEN: u32 = 101300;
pub const LOC_BASE_TOKEN: u32 = 100297;  // LOC_0
pub const LOC_MAX_TOKEN: u32 = 101296;   // LOC_999

/// Check if a token ID is a LOC coordinate token (LOC_0 to LOC_999)
pub fn is_loc_token(token: u32) -> bool {
    token >= LOC_BASE_TOKEN && token <= LOC_MAX_TOKEN
}

/// Check if a token ID is any LOC-related token (BEGIN, END, SEP, or coordinate)
pub fn is_any_loc_token(token: u32) -> bool {
    token == LOC_BEGIN_TOKEN || token == LOC_END_TOKEN || token == LOC_SEP_TOKEN || is_loc_token(token)
}

/// Convert LOC coordinate token to its value (0-999)
pub fn loc_token_to_value(token: u32) -> Result<u32, Error> {
    if !is_loc_token(token) {
        return Err(Error::InvalidLocToken(token));
    }
    Ok(token - LOC_BASE_TOKEN)
}

/// Bounding box coordinates (thousandths of image dimensions)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BBox {
    /// X coordinate of top-left corner (thousandths)
    pub x1: u32,
    /// Y coordinate of top-left corner (thousandths)
    pub y1: u32,
    /// X coordinate of bottom-right corner (thousandths)
    pub x2: u32,
    /// Y coordinate of bottom-right corner (thousandths)
    pub y2: u32,
}

impl BBox {
    /// Create a new bounding box
    pub fn new(x1: u32, y1: u32, x2: u32, y2: u32) -> Self {
        Self { x1, y1, x2, y2 }
    }

    /// Convert to pixel coordinates
    pub fn to_pixels(&self, width: u32, height: u32) -> (u32, u32, u32, u32) {
        let x1 = (self.x1 as f64 / 1000.0 * width as f64).round() as u32;
        let y1 = (self.y1 as f64 / 1000.0 * height as f64).round() as u32;
        let x2 = (self.x2 as f64 / 1000.0 * width as f64).round() as u32;
        let y2 = (self.y2 as f64 / 1000.0 * height as f64).round() as u32;
        (x1, y1, x2, y2)
    }

    /// Convert to normalized coordinates (0.0-1.0)
    pub fn to_normalized(&self) -> (f64, f64, f64, f64) {
        (
            self.x1 as f64 / 1000.0,
            self.y1 as f64 / 1000.0,
            self.x2 as f64 / 1000.0,
            self.y2 as f64 / 1000.0,
        )
    }
}

/// A single text block with position information
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextBlock {
    /// The recognized text
    pub text: String,
    /// Optional bounding box (None for pure text OCR)
    pub bbox: Option<BBox>,
}

impl TextBlock {
    /// Create a new text block with optional bounding box
    pub fn new(text: String, bbox: Option<BBox>) -> Self {
        Self { text, bbox }
    }

    /// Create a text block without position information
    pub fn text_only(text: String) -> Self {
        Self { text, bbox: None }
    }
}

/// Result of parsing spotting output
#[derive(Debug, Clone)]
pub struct ParsedSpottingResult {
    /// All recognized text blocks
    pub blocks: Vec<TextBlock>,
    /// Full text without LOC tokens
    pub full_text: String,
}

/// Parse LOC token string like "<|LOC_21|>" to its value
#[allow(dead_code)]
fn parse_loc_str(loc_str: &str) -> Option<u32> {
    let re = Regex::new(r"<\|LOC_(\d+)\|>").unwrap();
    if let Some(caps) = re.captures(loc_str) {
        let num: u32 = caps[1].parse().ok()?;
        if num <= 999 {
            return Some(num);
        }
    }
    None
}

/// Find all LOC token strings in text
fn find_loc_tokens_in_text(text: &str) -> Vec<(usize, usize, u32)> {
    let re = Regex::new(r"<\|LOC_(\d+)\|>").unwrap();
    let mut tokens = Vec::new();

    for cap in re.captures_iter(text) {
        let match_range = cap.get(0).unwrap();
        let num: u32 = cap[1].parse().unwrap_or(0);
        if num <= 999 {
            tokens.push((match_range.start(), match_range.end(), num));
        }
    }

    tokens
}

/// Parse spotting tokens into text blocks with positions
pub fn parse_spotting_text(text: &str) -> ParsedSpottingResult {
    let mut blocks = Vec::new();

    let loc_tokens = find_loc_tokens_in_text(text);

    if loc_tokens.is_empty() {
        return ParsedSpottingResult {
            blocks: vec![TextBlock::text_only(text.trim().to_string())],
            full_text: text.trim().to_string(),
        };
    }

    let clean_text = strip_loc_tokens(text);

    let mut bbox_list = Vec::new();
    let mut idx = 0;

    while idx + 8 <= loc_tokens.len() {
        let x1 = loc_tokens[idx].2;
        let y1 = loc_tokens[idx + 1].2;
        let x2 = loc_tokens[idx + 2].2;
        let y2 = loc_tokens[idx + 5].2;  // 6th token is y2

        bbox_list.push(BBox::new(x1, y1, x2, y2));
        idx += 8;
    }

    let lines: Vec<&str> = clean_text.lines().collect();

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim().to_string();
        if !trimmed.is_empty() {
            let bbox = if i < bbox_list.len() {
                Some(bbox_list[i])
            } else {
                None
            };
            blocks.push(TextBlock::new(trimmed, bbox));
        }
    }

    if blocks.is_empty() {
        blocks.push(TextBlock::text_only(clean_text.trim().to_string()));
    }

    ParsedSpottingResult {
        blocks,
        full_text: clean_text.trim().to_string(),
    }
}

/// Strip all LOC tokens from text
pub fn strip_loc_tokens(text: &str) -> String {
    let re = Regex::new(r"<\|LOC_\d+\|>").unwrap();
    let result = re.replace_all(text, "").to_string();
    result.replace("</s>", "").trim().to_string()
}

/// Parse spotting task output tokens into text blocks with positions
pub fn parse_spotting_tokens(tokens: &[u32]) -> ParsedSpottingResult {
    let mut blocks = Vec::new();
    let mut clean_tokens = Vec::new();
    let mut current_bbox_tokens = Vec::new();
    let mut current_text_tokens = Vec::new();
    let mut idx = 0;

    while idx < tokens.len() {
        if is_loc_token(tokens[idx]) {
            current_bbox_tokens.push(tokens[idx]);
            idx += 1;
        } else {
            if current_bbox_tokens.len() >= 8 && current_text_tokens.is_empty() {
                let x1 = loc_token_to_value(current_bbox_tokens[0]).unwrap_or(0);
                let y1 = loc_token_to_value(current_bbox_tokens[1]).unwrap_or(0);
                let x2 = loc_token_to_value(current_bbox_tokens[2]).unwrap_or(0);
                let y2 = loc_token_to_value(current_bbox_tokens[5]).unwrap_or(0);

                let bbox = BBox::new(x1, y1, x2, y2);

                while idx < tokens.len() && !is_loc_token(tokens[idx]) {
                    current_text_tokens.push(tokens[idx]);
                    clean_tokens.push(tokens[idx]);
                    idx += 1;
                }

                if !current_text_tokens.is_empty() {
                    blocks.push(TextBlock::new(String::new(), Some(bbox)));
                }

                current_bbox_tokens.clear();
                current_text_tokens.clear();
            } else {
                current_text_tokens.push(tokens[idx]);
                clean_tokens.push(tokens[idx]);
                idx += 1;
            }
        }
    }

    if !current_text_tokens.is_empty() {
        blocks.push(TextBlock::text_only(String::new()));
    }

    ParsedSpottingResult {
        blocks,
        full_text: String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_loc_token() {
        assert!(is_loc_token(100297));  // LOC_0
        assert!(is_loc_token(100500));  // LOC_203
        assert!(is_loc_token(101296));  // LOC_999
        assert!(!is_loc_token(100296)); // Below range
        assert!(!is_loc_token(101297)); // Above range
        assert!(!is_loc_token(101298)); // LOC_BEGIN (not a coordinate)
        assert!(!is_loc_token(101299)); // LOC_END
        assert!(!is_loc_token(101300)); // LOC_SEP
    }

    #[test]
    fn test_loc_token_to_value() {
        assert_eq!(loc_token_to_value(100297).unwrap(), 0);    // LOC_0
        assert_eq!(loc_token_to_value(100500).unwrap(), 203);  // LOC_203
        assert_eq!(loc_token_to_value(101296).unwrap(), 999);  // LOC_999
        assert!(loc_token_to_value(101298).is_err());  // LOC_BEGIN
    }

    #[test]
    fn test_bbox_to_pixels() {
        let bbox = BBox::new(100, 200, 500, 800);  // 10%, 20%, 50%, 80%
        let (x1, y1, x2, y2) = bbox.to_pixels(1000, 500);
        assert_eq!(x1, 100);
        assert_eq!(y1, 100);
        assert_eq!(x2, 500);
        assert_eq!(y2, 400);
    }

    #[test]
    fn test_bbox_to_normalized() {
        let bbox = BBox::new(100, 200, 500, 800);
        let (x1, y1, x2, y2) = bbox.to_normalized();
        assert!((x1 - 0.1).abs() < 0.001);
        assert!((y1 - 0.2).abs() < 0.001);
        assert!((x2 - 0.5).abs() < 0.001);
        assert!((y2 - 0.8).abs() < 0.001);
    }

    #[test]
    fn test_parse_loc_str() {
        assert_eq!(parse_loc_str("<|LOC_21|>"), Some(21));
        assert_eq!(parse_loc_str("<|LOC_337|>"), Some(337));
        assert_eq!(parse_loc_str("<|LOC_999|>"), Some(999));
        assert_eq!(parse_loc_str("<|LOC_1000|>"), None);  // > 999
        assert_eq!(parse_loc_str("not a loc"), None);
    }

    #[test]
    fn test_strip_loc_tokens() {
        let text = "Hello<|LOC_21|><|LOC_337|>World";
        let stripped = strip_loc_tokens(text);
        assert_eq!(stripped, "HelloWorld");

        let text2 = "<|LOC_21|><|LOC_337|><|LOC_930|><|LOC_337|>";
        let stripped2 = strip_loc_tokens(text2);
        assert_eq!(stripped2, "");
    }

    #[test]
    fn test_find_loc_tokens_in_text() {
        let text = "text<|LOC_21|><|LOC_337|><|LOC_930|><|LOC_422|>";
        let tokens = find_loc_tokens_in_text(text);
        assert_eq!(tokens.len(), 4);
        assert_eq!(tokens[0].2, 21);
        assert_eq!(tokens[1].2, 337);
        assert_eq!(tokens[2].2, 930);
        assert_eq!(tokens[3].2, 422);
    }

    #[test]
    fn test_parse_spotting_text() {
        let text = "Line 1<|LOC_21|><|LOC_337|><|LOC_930|><|LOC_337|><|LOC_930|><|LOC_422|><|LOC_21|><|LOC_422|>\nLine 2";
        let result = parse_spotting_text(text);
        assert_eq!(result.full_text, "Line 1\nLine 2");
        assert_eq!(result.blocks.len(), 2);

        if let Some(bbox) = result.blocks[0].bbox {
            assert_eq!(bbox.x1, 21);
            assert_eq!(bbox.y1, 337);
            assert_eq!(bbox.x2, 930);
            assert_eq!(bbox.y2, 422);
        }
    }
}