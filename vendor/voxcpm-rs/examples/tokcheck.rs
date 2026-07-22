//! Smoke test: print the tokenizer output for a string so we can compare
//! against the Python reference.
use voxcpm_rs::tokenizer::TextTokenizer;

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| "/home/nipah/dev/ai_space/VoxCPM2".into());
    let text = std::env::args().nth(2).unwrap_or_else(|| "Hello world.".into());
    let tok = TextTokenizer::from_local(&dir).expect("tok");
    let ids = tok.encode(&text).expect("encode");
    println!("rust ids: {ids:?}");
}
