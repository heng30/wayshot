mod g2p_en;

pub use g2p_en::*;
use {
    crate::{GSVError, text::get_phone_symbol},
    log::debug,
    std::borrow::Cow,
};

#[derive(PartialEq, Eq, Clone)]
pub enum EnWord {
    Word(String),
    Punctuation(&'static str),
}

impl std::fmt::Debug for EnWord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EnWord::Word(w) => write!(f, "\"{}\"", w),
            EnWord::Punctuation(p) => write!(f, "\"{}\"", p),
        }
    }
}

#[derive(Debug, Default)]
pub struct EnSentence {
    pub phone_ids: Vec<i64>,
    pub phones: Vec<Cow<'static, str>>,
    pub word2ph: Vec<i32>,
    pub text: Vec<EnWord>,
}

impl EnSentence {
    /// Creates a new EnSentence with a single word and pre-allocated capacity
    pub fn new_with_word(word: EnWord) -> Self {
        let mut en = Self {
            phone_ids: Vec::with_capacity(16),
            phones: Vec::with_capacity(16),
            text: Vec::with_capacity(16),
            word2ph: Vec::with_capacity(16),
        };
        en.text.push(word);
        en
    }

    /// Checks if a phoneme contains stress markers
    #[inline]
    fn has_stress_marker(ph: &str) -> bool {
        ph.chars().any(|c| c.is_ascii_digit() && c <= '4')
    }

    pub fn g2p(&mut self, g2p_en: &mut G2pEn) -> Result<(), GSVError> {
        self.phones.clear();
        self.phone_ids.clear();
        self.word2ph.clear();

        let phone_count = self.text.len() * 3;
        self.phones.reserve(phone_count);
        self.phone_ids.reserve(phone_count);
        self.word2ph.reserve(self.text.len());

        for word in &self.text {
            match word {
                EnWord::Word(w) => {
                    let phonemes = g2p_en.g2p(w)?;
                    let mut cnt = 0;
                    for ph in &phonemes {
                        self.phones.push(Cow::Owned(ph.clone()));
                        self.phone_ids.push(get_phone_symbol(ph));
                        cnt += 1;
                        if Self::has_stress_marker(ph) {
                            self.word2ph.push(cnt);
                            cnt = 0;
                        }
                    }
                    if cnt > 0 {
                        self.word2ph.push(cnt);
                    }
                }
                EnWord::Punctuation(p) => {
                    self.phones.push(Cow::Borrowed(p));
                    self.phone_ids.push(get_phone_symbol(p));
                    self.word2ph.push(1);
                }
            }
        }
        debug!("EnSentence phones: {:?}", self.phones);
        debug!("EnSentence phone_ids: {:?}", self.phone_ids);
        debug!("EnSentence word2ph: {:?}", self.word2ph);
        Ok(())
    }
}
