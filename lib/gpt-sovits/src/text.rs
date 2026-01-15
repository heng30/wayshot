mod bert;
mod dict;
mod en;
mod num;
mod phone_symbol;
mod utils;
mod zh;

use {
    crate::error::GSVError,
    jieba_rs::Jieba,
    log::{debug, warn},
    ndarray::Array2,
    regex::Regex,
    std::sync::LazyLock,
    unicode_segmentation::UnicodeSegmentation,
};
pub use {
    bert::BertModel,
    en::{EnSentence, EnWord, G2pEn},
    num::{NumSentence, is_numeric},
    phone_symbol::get_phone_symbol,
    utils::{BERT_TOKENIZER, DICT_MONO_CHARS, DICT_POLY_CHARS, argmax_2d, str_is_chinese},
    zh::{G2PW, G2PWOut, ZhMode, ZhSentence},
};

/// Type alias for phone and BERT feature results
type PhoneAndBertResult = Vec<(String, Vec<i64>, Array2<f32>)>;

// Regex to handle emojis and symbols
static CLEANUP_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"[\u{1F600}-\u{1F64F}\u{1F300}-\u{1F5FF}\u{1F680}-\u{1F6FF}\u{1F900}-\u{1F9FF}\u{2600}-\u{27BF}\u{2000}-\u{206F}\u{2300}-\u{23FF}]+",
    )
    .expect("Failed to compile CLEANUP_REGEX")
});

// Simplified regex for tokenization
static TOKEN_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?x)
        \p{Han}+ |              # Chinese characters
        [a-zA-Z]+(?:['-][a-zA-Z]+)* | # English words with optional apostrophes/hyphens
        \d+(?:\.\d+)? |          # Numbers (including decimals)
        [.,!?;:()\[\]<>\-"$/\u{3001}\u{3002}\u{FF01}\u{FF1F}\u{FF1B}\u{FF1A}\u{FF0C}\u{2018}\u{2019}\u{201C}\u{201D}] | # Punctuation
        \s+                      # Whitespace
        "#,
    )
    .expect("Failed to compile TOKEN_REGEX")
});

/// Filters out emojis and other non-essential symbols from the text.
fn cleanup_text(text: &str) -> String {
    CLEANUP_REGEX.replace_all(text, " ").into_owned()
}

/// Helper to push trimmed non-empty text to items vector
#[inline]
fn push_trimmed_non_empty(items: &mut Vec<String>, text: &str) {
    let trimmed = text.trim();
    if !trimmed.is_empty() {
        items.push(trimmed.to_string());
    }
}

pub fn split_text(text: &str) -> Vec<String> {
    let mut items = Vec::with_capacity(text.len() / 20);
    let mut current = String::with_capacity(64);
    let mut chars = text.chars().peekable();

    while let Some(c) = chars.next() {
        // Handle newlines separately - don't add them to current sentence
        if c == '\n' || c == '\r' {
            push_trimmed_non_empty(&mut items, &current);
            current.clear();
            continue;
        }

        current.push(c);

        // Check if current character is end punctuation
        let is_end_punctuation = matches!(c, '。' | '！' | '？' | '；' | '.' | '!' | '?' | ';');

        if is_end_punctuation {
            // Special handling for period (.)
            if c == '.' {
                if let Some(&next_char) = chars.peek() {
                    // Case 1: Abbreviation like "Dr. Smith" - next char is space followed by uppercase
                    if next_char == ' ' {
                        let mut peek_iter = chars.clone();
                        peek_iter.next(); // Skip the space
                        if let Some(after_space) = peek_iter.next()
                            && after_space.is_uppercase()
                        {
                            continue;
                        }
                    }

                    // Case 2: Decimal number like "1.0版本" - next char is digit
                    // Case 3: Abbreviation with lowercase letter following
                    if next_char.is_ascii_digit() || next_char.is_lowercase() {
                        continue;
                    }
                }
            }
            // For other punctuation, check if next character is lowercase letter
            else if matches!(c, '!' | '?' | ';')
                && matches!(chars.peek(), Some(&c) if c.is_lowercase())
            {
                continue;
            }

            push_trimmed_non_empty(&mut items, &current);
            current.clear();
        }
    }

    // Handle any remaining text
    push_trimmed_non_empty(&mut items, &current);

    items
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Lang {
    Zh,
    En,
}

#[derive(Debug, Clone, Copy)]
pub enum LangId {
    Auto,    // Mandarin
    AutoYue, // Cantonese
}

pub trait SentenceProcessor {
    fn get_text_for_bert(&self) -> String;
    fn get_word2ph(&self) -> &[i32];
    fn get_phone_ids(&self) -> &[i64];
}

impl SentenceProcessor for EnSentence {
    fn get_text_for_bert(&self) -> String {
        let mut result = String::with_capacity(self.text.len() * 10);
        for word in &self.text {
            match word {
                EnWord::Word(w) => {
                    if !result.is_empty() && !result.ends_with(' ') {
                        result.push(' ');
                    }
                    result.push_str(w);
                }
                EnWord::Punctuation(p) => {
                    result.push_str(p);
                }
            }
        }
        debug!("English BERT text: {}", result);
        result
    }

    fn get_word2ph(&self) -> &[i32] {
        &self.word2ph
    }

    fn get_phone_ids(&self) -> &[i64] {
        &self.phone_ids
    }
}

impl SentenceProcessor for ZhSentence {
    fn get_text_for_bert(&self) -> String {
        debug!("Chinese BERT text: {}", self.text);
        self.text.clone()
    }

    fn get_word2ph(&self) -> &[i32] {
        &self.word2ph
    }

    fn get_phone_ids(&self) -> &[i64] {
        &self.phone_ids
    }
}

pub struct TextProcessor {
    pub jieba: Jieba,
    pub g2pw: G2PW,
    pub g2p_en: G2pEn,
    pub bert_model: BertModel,
}

impl TextProcessor {
    pub fn new(g2pw: G2PW, g2p_en: G2pEn, bert_model: BertModel) -> Result<Self, GSVError> {
        Ok(Self {
            jieba: Jieba::new(),
            g2pw,
            g2p_en,
            bert_model,
        })
    }

    pub fn get_phone_and_bert(
        &mut self,
        text: &str,
        lang_id: LangId,
    ) -> Result<PhoneAndBertResult, GSVError> {
        if text.trim().is_empty() {
            return Err(GSVError::InputEmpty);
        }

        let chunks = split_text(&cleanup_text(text));
        let mut result = Vec::with_capacity(chunks.len());

        for chunk in chunks.iter() {
            debug!("Processing chunk: {}", chunk);
            let mut phone_builder = PhoneBuilder::new(chunk);
            phone_builder.extend_text(&self.jieba, chunk);

            if !chunk
                .trim_end()
                .ends_with(['。', '.', '?', '？', '!', '！', '；', ';', '\n'])
            {
                phone_builder.push_punctuation(".");
            }

            // --- A. Collect data for all sub-sentences in the chunk ---
            #[derive(Debug)]
            struct SubSentenceData {
                bert_text: String,
                word2ph: Vec<i32>,
                phone_ids: Vec<i64>,
            }

            let sentence_count = phone_builder.sentences.len();
            let mut sub_sentences_data: Vec<SubSentenceData> = Vec::with_capacity(sentence_count);

            for mut sentence in phone_builder.sentences {
                let g2p_result = match &mut sentence {
                    Sentence::Zh(zh) => {
                        let mode = if matches!(lang_id, LangId::AutoYue) {
                            ZhMode::Cantonese
                        } else {
                            ZhMode::Mandarin
                        };
                        zh.g2p(&mut self.g2pw, mode);
                        Ok(())
                    }
                    Sentence::En(en) => en.g2p(&mut self.g2p_en),
                };

                if g2p_result.is_ok() && !sentence.get_phone_ids().is_empty() {
                    sub_sentences_data.push(SubSentenceData {
                        bert_text: sentence.get_text_for_bert(),
                        word2ph: sentence.get_word2ph().to_vec(),
                        phone_ids: sentence.get_phone_ids().to_vec(),
                    });
                } else if let Err(e) = g2p_result {
                    warn!("G2P failed for a sentence part in chunk '{}': {}", chunk, e);
                }
            }

            // --- B. Group sub-sentences into logically complete sentences ---
            #[derive(Default, Debug)]
            struct GroupedSentence {
                text: String,
                word2ph: Vec<i32>,
                phone_ids: Vec<i64>,
            }

            let group_count = sub_sentences_data.len() / 2 + 1;
            let mut grouped_sentences: Vec<GroupedSentence> = Vec::with_capacity(group_count);
            let mut current_group = GroupedSentence::default();

            for data in sub_sentences_data {
                let ends_sentence = data
                    .bert_text
                    .find(['。', '.', '?', '？', '!', '！', '；', ';']);

                current_group.text.push_str(&data.bert_text);
                current_group.word2ph.extend(data.word2ph);
                current_group.phone_ids.extend(data.phone_ids);
                if ends_sentence.is_some() {
                    grouped_sentences.push(current_group);
                    current_group = GroupedSentence::default()
                }
            }
            // Add any remaining part that didn't end with punctuation
            if !current_group.text.is_empty() {
                grouped_sentences.push(current_group);
            }

            // --- C. Process each complete sentence with BERT ---
            for group in grouped_sentences {
                debug!("Processing grouped sentence: '{}'", group.text);
                let total_expected_bert_len = group.phone_ids.len();

                match self
                    .bert_model
                    .get_bert(&group.text, &group.word2ph, total_expected_bert_len)
                {
                    Ok(bert_features) => {
                        if bert_features.shape()[0] != total_expected_bert_len {
                            warn!(
                                "BERT output length mismatch for text '{}': expected {}, got {}",
                                group.text,
                                total_expected_bert_len,
                                bert_features.shape()[0]
                            );
                            continue;
                        }
                        result.push((group.text, group.phone_ids, bert_features));
                    }
                    Err(e) => {
                        warn!(
                            "Failed to get BERT features for text '{}': {}",
                            group.text, e
                        );
                    }
                }
            }
        }

        debug!("RESULT (total sentences: {})", result.len());
        if result.is_empty() {
            return Err(GSVError::GeneratePhonemesOrBertFeaturesFailed(
                text.to_owned(),
            ));
        }
        Ok(result)
    }
}

fn parse_punctuation(p: &str) -> Option<&'static str> {
    match p {
        "，" | "," => Some(","),
        "。" | "." => Some("."),
        "！" | "!" => Some("!"),
        "？" | "?" => Some("?"),
        "；" | ";" => Some(";"),
        "：" | ":" => Some(":"),
        "'" => Some("'"),
        "＇" => Some("'"),
        "＂" => Some("\""),
        "（" | "(" => Some("("),
        "）" | ")" => Some(")"),
        "【" | "[" => Some("["),
        "】" | "]" => Some("]"),
        "《" | "<" => Some("<"),
        "》" | ">" => Some(">"),
        "—" | "–" => Some("-"),
        "～" | "~" => Some("~"),
        "…" | "..." => Some("..."),
        "·" => Some("·"),
        "、" => Some("、"),
        "$" => Some("$"),
        "/" => Some("/"),
        "\n" => Some("\n"),
        " " => Some(" "),
        _ => None,
    }
}

#[derive(Debug)]
enum Sentence {
    Zh(ZhSentence),
    En(EnSentence),
}

impl SentenceProcessor for Sentence {
    fn get_text_for_bert(&self) -> String {
        match self {
            Sentence::Zh(zh) => zh.get_text_for_bert(),
            Sentence::En(en) => en.get_text_for_bert(),
        }
    }

    fn get_word2ph(&self) -> &[i32] {
        match self {
            Sentence::Zh(zh) => zh.get_word2ph(),
            Sentence::En(en) => en.get_word2ph(),
        }
    }

    fn get_phone_ids(&self) -> &[i64] {
        match self {
            Sentence::Zh(s) => s.get_phone_ids(),
            Sentence::En(s) => s.get_phone_ids(),
        }
    }
}

struct PhoneBuilder {
    sentences: Vec<Sentence>,
    sentence_lang: Lang,
}

impl PhoneBuilder {
    fn new(text: &str) -> Self {
        let sentence_lang = detect_sentence_language(text);
        Self {
            sentences: Vec::with_capacity(16),
            sentence_lang,
        }
    }

    /// Helper to process numeric tokens and convert them to language-specific text
    fn process_numeric_token(&mut self, token: &str) {
        let ns = NumSentence {
            text: token.to_owned(),
            lang: self.sentence_lang,
        };
        let txt = match ns.to_lang_text() {
            Ok(txt) => txt,
            Err(e) => {
                warn!("Failed to process numeric token '{}': {}", token, e);
                token.to_string()
            }
        };
        match self.sentence_lang {
            Lang::Zh => self.push_zh_word(&txt),
            Lang::En => self.push_en_word(&txt),
        }
    }

    /// Helper to process a single token (word, number, or punctuation)
    fn process_token(&mut self, token: &str) {
        if let Some(p) = parse_punctuation(token) {
            self.push_punctuation(p);
        } else if is_numeric(token) {
            self.process_numeric_token(token);
        } else if str_is_chinese(token) {
            self.push_zh_word(token);
        } else if token
            .chars()
            .all(|c| c.is_ascii_alphabetic() || c == '\'' || c == '-')
        {
            self.push_en_word(token);
        }
    }

    fn extend_text(&mut self, jieba: &Jieba, text: &str) {
        let tokens: Vec<&str> = if str_is_chinese(text) {
            jieba.cut(text, true).into_iter().collect()
        } else {
            TOKEN_REGEX.find_iter(text).map(|m| m.as_str()).collect()
        };

        for t in tokens {
            // First, try to process the token directly
            if let Some(p) = parse_punctuation(t) {
                self.push_punctuation(p);
                continue;
            }

            if is_numeric(t)
                || str_is_chinese(t)
                || t.chars()
                    .all(|c| c.is_ascii_alphabetic() || c == '\'' || c == '-')
            {
                self.process_token(t);
            } else {
                // Handle mixed-language tokens by re-tokenizing
                for sub_token in TOKEN_REGEX.find_iter(t) {
                    self.process_token(sub_token.as_str());
                }
            }
        }
    }

    fn push_punctuation(&mut self, p: &'static str) {
        match self.sentences.last_mut() {
            Some(Sentence::Zh(zh)) => {
                zh.text.push_str(p);
                let first_char = p.chars().next().unwrap_or(' ');
                zh.phones.push(G2PWOut::RawChar(first_char));
            }
            Some(Sentence::En(en)) => {
                // Skip space after "a"
                if p == " " && matches!(en.text.last(), Some(EnWord::Word(w)) if w == "a") {
                    return;
                }
                en.text.push(EnWord::Punctuation(p));
            }
            None => {
                self.sentences.push(Sentence::En(EnSentence::new_with_word(
                    EnWord::Punctuation(p),
                )));
            }
        }
    }

    fn push_en_word(&mut self, word: &str) {
        // Create word variant once to avoid repeated to_string() calls
        let word_variant = EnWord::Word(word.to_string());

        if word.ends_with(['。', '.', '?', '？', '!', '！', '；', ';', '\n']) {
            self.sentences.push(Sentence::En(EnSentence::new_with_word(
                word_variant.clone(),
            )));
            return;
        }

        match self.sentences.last_mut() {
            Some(Sentence::En(en)) => {
                // Handle contraction merging: if last token is ' or -, append to previous word
                if let Some(&EnWord::Punctuation(p)) = en.text.last()
                    && (p == "'" || p == "-")
                    && let Some(EnWord::Punctuation(p_str)) = en.text.pop()
                    && let Some(EnWord::Word(last_word)) = en.text.last_mut()
                {
                    last_word.push_str(p_str);
                    last_word.push_str(word);
                    return;
                }
                en.text.push(word_variant);
            }
            _ => {
                self.sentences
                    .push(Sentence::En(EnSentence::new_with_word(word_variant)));
            }
        }
    }

    fn push_zh_word(&mut self, word: &str) {
        fn add_zh_word(zh: &mut ZhSentence, word: &str) {
            zh.text.push_str(word);
            match dict::zh_word_dict(word) {
                Some(phones) => {
                    zh.phones
                        .extend(phones.iter().map(|p| G2PWOut::Pinyin(p.clone())));
                }
                None => {
                    zh.phones
                        .extend(word.chars().map(|_| G2PWOut::Pinyin(String::new())));
                }
            }
        }

        if word.ends_with(['。', '.', '?', '？', '!', '！', '；', ';', '\n']) {
            self.sentences.push(Sentence::Zh(ZhSentence::new()));
        }

        match self.sentences.last_mut() {
            Some(Sentence::Zh(zh)) => add_zh_word(zh, word),
            _ => {
                let mut zh = ZhSentence::new();
                add_zh_word(&mut zh, word);
                self.sentences.push(Sentence::Zh(zh));
            }
        }
    }
}

/// Detects the dominant language of a sentence based on character distribution.
fn detect_sentence_language(text: &str) -> Lang {
    let graphemes: Vec<_> = text.graphemes(true).collect();
    if graphemes.is_empty() {
        return Lang::Zh; // Default to Chinese for empty input
    }

    let zh_count = graphemes.iter().filter(|g| str_is_chinese(g)).count();
    let zh_percent = zh_count as f32 / graphemes.len() as f32;

    debug!("chinese percent {}", zh_percent);
    if zh_percent > 0.3 { Lang::Zh } else { Lang::En }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_text() {
        assert_eq!(split_text("Dr. Smith"), ["Dr. Smith"]);
        assert_eq!(split_text("1.0版本"), ["1.0版本"]);
    }
}
