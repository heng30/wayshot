use {crate::text::zh::jyutping_list::get_jyutping_list, log::debug, regex::Regex};

const INITIALS: &[&str] = &[
    "aa", "aai", "aak", "aap", "aat", "aau", "ai", "au", "ap", "at", "ak", "a", "p", "b", "e",
    "ts", "t", "dz", "d", "kw", "k", "gw", "g", "f", "h", "l", "m", "ng", "n", "s", "y", "w", "c",
    "z", "j", "ong", "on", "ou", "oi", "ok", "o", "uk", "ung", "sp", "spl", "spn", "sil",
];

static PUNCTUATIONS: &str = ",.!?;:()[]{}'\"-…";

fn get_jyutping(text: &str) -> Vec<String> {
    let punct_pattern = Regex::new(&format!(r"^[{}]+$", regex::escape(PUNCTUATIONS))).unwrap();

    let syllables = get_jyutping_list(text);
    debug!("jyutping {:?}", syllables);
    let mut jyutping_array = Vec::with_capacity(syllables.len());

    for (word, syllable) in syllables {
        if punct_pattern.is_match(&word) {
            jyutping_array.extend(word.chars().map(|c| c.to_string()));
        } else {
            jyutping_array.push(syllable);
        }
    }

    jyutping_array
}

fn jyuping_to_initials_finals_tones(jyuping_syllables: Vec<String>) -> (Vec<String>, Vec<i32>) {
    let mut phones = Vec::with_capacity(jyuping_syllables.len() * 2);
    let mut word2ph = Vec::with_capacity(jyuping_syllables.len());

    for syllable in jyuping_syllables {
        if PUNCTUATIONS.contains(syllable.chars().next().unwrap_or_default()) || syllable == "_" {
            phones.push(syllable.clone());
            word2ph.push(1);
        } else {
            let (tone, syllable_without_tone) =
                if let Some(last_char) = syllable.chars().last()
                    && last_char.is_ascii_digit() {
                    (last_char.to_digit(10).unwrap() as i32, &syllable[..syllable.len() - 1])
                } else {
                    (0, syllable.as_str())
                };

            let mut found = false;
            for &initial in INITIALS {
                if syllable_without_tone.starts_with(initial) {
                    if syllable_without_tone.starts_with("nga") {
                        let initial_part = &syllable_without_tone[..2];
                        let final_part = if syllable_without_tone.len() > 2 {
                            &syllable_without_tone[2..]
                        } else {
                            &syllable_without_tone[syllable_without_tone.len() - 1..]
                        };
                        phones.push(format!("Y{}", initial_part));
                        phones.push(format!("Y{}{}", final_part, tone));
                        word2ph.push(2);
                    } else {
                        let f = syllable_without_tone
                            .strip_prefix(initial)
                            .filter(|s| !s.is_empty())
                            .unwrap_or(&initial[initial.len() - 1..]);
                        phones.push(format!("Y{}", initial));
                        phones.push(format!("Y{}{}", f, tone));
                        word2ph.push(2);
                    }
                    found = true;
                    break;
                }
            }
            if !found {
                phones.push(format!("Y{}", syllable_without_tone));
                word2ph.push(1);
            }
        }
    }

    (phones, word2ph)
}

pub fn g2p(text: &str) -> (Vec<String>, Vec<i32>) {
    let jyuping = get_jyutping(text);
    debug!("jyuping {:?}", jyuping);
    jyuping_to_initials_finals_tones(jyuping)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_g2p() {
        let text = "佢個鋤頭太短啦。";
        let (phones, word2ph) = g2p(&text);
        println!("Phones: {:?}", phones);
        println!("Word2Ph: {:?}", word2ph);
    }
}
