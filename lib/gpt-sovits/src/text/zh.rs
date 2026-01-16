mod g2pw;
mod jyutping_list;
mod split;
mod yue;

use crate::text::get_phone_symbol;

pub use g2pw::{G2PW, G2PWOut};
pub use split::split_zh_ph;

#[derive(Debug)]
pub enum ZhMode {
    Mandarin,
    Cantonese,
}

#[derive(Debug, Default)]
pub struct ZhSentence {
    pub phone_ids: Vec<i64>,
    pub phones: Vec<G2PWOut>,
    pub word2ph: Vec<i32>,
    pub text: String,
}

impl ZhSentence {
    pub fn new() -> Self {
        Self {
            phone_ids: Vec::with_capacity(16),
            phones: Vec::with_capacity(16),
            word2ph: Vec::with_capacity(16),
            text: String::with_capacity(32),
        }
    }

    pub fn g2p(&mut self, g2pw: &mut G2PW, mode: ZhMode) {
        match mode {
            ZhMode::Mandarin => self.g2p_mandarin(g2pw),
            ZhMode::Cantonese => self.g2p_cantonese(),
        }
    }

    fn g2p_mandarin(&mut self, g2pw: &mut G2PW) {
        let pinyin = g2pw.g2p(&self.text);
        let text_len = self.text.chars().count();
        if pinyin.len() != text_len && !self.text.is_empty() {
            log::warn!(
                "Pinyin length mismatch: {} (pinyin) vs {} (text chars) for text '{}'",
                pinyin.len(),
                text_len,
                self.text
            );
        }
        self.phones = pinyin;
        self.build_phone_id_and_word2ph();
    }

    fn g2p_cantonese(&mut self) {
        let (pinyin, word2ph) = yue::g2p(&self.text);
        self.phones = Vec::from_iter(pinyin.into_iter().map(G2PWOut::Yue));
        self.build_phone_id_and_word2ph();
        self.word2ph = word2ph;
    }

    fn build_phone_id_and_word2ph(&mut self) {
        for p in &self.phones {
            match p {
                G2PWOut::Pinyin(p) => {
                    let (initial, final_) = split_zh_ph(p);
                    self.phone_ids.push(get_phone_symbol(initial));
                    if final_.is_empty() {
                        self.word2ph.push(1);
                    } else {
                        self.phone_ids.push(get_phone_symbol(final_));
                        self.word2ph.push(2);
                    }
                }
                G2PWOut::Yue(c) => {
                    self.phone_ids.push(get_phone_symbol(c));
                    self.word2ph.push(2);
                }
                G2PWOut::RawChar(c) => {
                    self.phone_ids.push(get_phone_symbol(&c.to_string()));
                    self.word2ph.push(1);
                }
            }
        }

        log::debug!("ZhSentence phones: {:?}", self.phones);
        log::debug!("ZhSentence phone_ids: {:?}", self.phone_ids);
        log::debug!("ZhSentence word2ph: {:?}", self.word2ph);
    }
}
