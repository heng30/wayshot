mod g2pw;
mod jyutping_list;
mod split;
mod yue;

use {
    crate::text::get_phone_symbol,
    log::{debug, warn},
};
pub use {
    g2pw::{G2PW, G2PWOut},
    split::split_zh_ph,
};

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
    /// Creates a new ZhSentence with pre-allocated capacity
    pub fn new() -> Self {
        Self {
            phone_ids: Vec::with_capacity(16),
            phones: Vec::with_capacity(16),
            word2ph: Vec::with_capacity(16),
            text: String::with_capacity(32),
        }
    }

    /// Processes Chinese text into phonemes and phone IDs based on the specified mode.
    pub fn g2p(&mut self, g2pw: &mut G2PW, mode: ZhMode) {
        match mode {
            ZhMode::Mandarin => self.g2p_mandarin(g2pw),
            ZhMode::Cantonese => self.g2p_cantonese(),
        }
    }

    /// Processes Mandarin text using the G2PW model.
    fn g2p_mandarin(&mut self, g2pw: &mut G2PW) {
        let pinyin = g2pw.g2p(&self.text);
        let text_len = self.text.chars().count();
        if pinyin.len() != text_len && !self.text.is_empty() {
            warn!(
                "Pinyin length mismatch: {} (pinyin) vs {} (text chars) for text '{}'",
                pinyin.len(), text_len, self.text
            );
        }
        self.phones = pinyin;
        debug!("phones: {:?}", self.phones);
        self.build_phone_id_and_word2ph();
    }

    /// Processes Cantonese text using the yue module.
    fn g2p_cantonese(&mut self) {
        let (pinyin, word2ph) = yue::g2p(&self.text);
        debug!("pinyin: {:?}", pinyin);
        self.phones = Vec::from_iter(pinyin.into_iter().map(G2PWOut::Yue));
        self.build_phone_id_and_word2ph();
        self.word2ph = word2ph;
    }

    /// Converts phonemes to phone IDs and generates word-to-phoneme mapping.
    fn build_phone_id_and_word2ph(&mut self) {
        let phone_count = self.phones.len();
        self.phone_ids.clear();
        self.phone_ids.reserve(phone_count * 2);
        self.word2ph.clear();
        self.word2ph.reserve(phone_count);

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
        debug!("phone_id {:?}", self.phone_ids);
    }
}
