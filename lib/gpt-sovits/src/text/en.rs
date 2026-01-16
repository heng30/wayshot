mod g2p_en;

use crate::{Result, text::get_phone_symbol};
use std::borrow::Cow;

pub use g2p_en::*;

#[derive(PartialEq, Eq, Clone, Debug)]
pub enum EnWord {
    Word(String),
    Punctuation(&'static str),
}

#[derive(Debug, Default)]
pub struct EnSentence {
    pub phone_ids: Vec<i64>,
    pub phones: Vec<Cow<'static, str>>,
    pub word2ph: Vec<i32>,
    pub text: Vec<EnWord>,
}

impl EnSentence {
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

    #[inline]
    fn has_stress_marker(ph: &str) -> bool {
        ph.chars().any(|c| c.is_ascii_digit() && c <= '4')
    }

    #[inline]
    fn clear(&mut self) {
        self.phones.clear();
        self.phone_ids.clear();
        self.word2ph.clear();
    }

    pub fn g2p(&mut self, g2p_en: &mut G2pEn) -> Result<()> {
        self.clear();

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
        log::debug!("EnSentence phones: {:?}", self.phones);
        log::debug!("EnSentence phone_ids: {:?}", self.phone_ids);
        log::debug!("EnSentence word2ph: {:?}", self.word2ph);
        Ok(())
    }
}
