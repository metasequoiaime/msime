//! Per-key Cantonese candidate latency on a built `msime-cantonese.db`.
//!
//! Each key is timed through `set_raw_input` with the letters typed so far plus `candidates`, which is the work the scheme does for every key a person types (`handle_key` is not public, and replaying a few letters costs nothing beside the lookups). The readings include the syllables with the most entries in the database (`zi`, `ji`, `jyu`, `ci`, `jau`, `wai`), whose single-syllable lookups read up to `SPAN_LIMIT` rows, typed both as Jyutping letters and as the double-pinyin codes of the dedicated double-pinyin entry. The report prints the distribution, not a mean: a person feels the slow tail.
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use msime_engine::cantonese::scheme::SPAN_LIMIT;
use msime_engine::cantonese::{shuangpin, CantoneseScheme, Inventory};
use msime_engine::language_dictionary;

/// Readings typed one after another, each committed before the next starts.
const READINGS: [&[&str]; 15] = [
    &["zi"],
    &["ji"],
    &["jyu"],
    &["ci"],
    &["jau"],
    &["wai"],
    &["fu"],
    &["si"],
    &["jin"],
    &["jyun"],
    &["jyu", "kei"],
    &["nei", "hou"],
    &["soeng", "ping"],
    &["gung", "gaa", "saan"],
    &["zi", "ji", "jyu"],
];
/// Rounds when `CANTONESE_ROUNDS` is unset.
const DEFAULT_ROUNDS: usize = 20;

fn report(label: &str, mut samples: Vec<f64>) {
    if samples.is_empty() {
        return;
    }
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let at = |q: f64| samples[((samples.len() - 1) as f64 * q).round() as usize];
    println!(
        "{label:<12} n={:<5} p50={:7.3}ms p95={:7.3}ms p99={:7.3}ms max={:7.3}ms",
        samples.len(),
        at(0.50),
        at(0.95),
        at(0.99),
        at(1.00)
    );
}

fn keys(reading: &[&str], double_pinyin: bool) -> Vec<u8> {
    let mut keys = Vec::new();
    for syllable in reading {
        if double_pinyin {
            let code = shuangpin::encode(syllable)
                .unwrap_or_else(|| panic!("{syllable} has no double-pinyin code"));
            keys.extend(code);
        } else {
            keys.extend(syllable.bytes());
        }
    }
    keys
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .ok_or("usage: cantonese_latency <msime-cantonese.db>")?;
    let dictionary = language_dictionary::open_read_only(&path)?;
    let inventory = Arc::new(Inventory::load(&dictionary)?);
    let rounds = std::env::var("CANTONESE_ROUNDS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_ROUNDS);
    println!("SPAN_LIMIT={SPAN_LIMIT} rounds={rounds}");
    for (label, double_pinyin) in [("jyutping", false), ("shuangpin", true)] {
        let mut scheme = CantoneseScheme::new(inventory.clone());
        scheme.set_force_shuangpin(double_pinyin);
        let mut all = Vec::new();
        let mut rows = Vec::new();
        for round in 0..rounds {
            for reading in READINGS {
                let typed = keys(reading, double_pinyin);
                for length in 1..=typed.len() {
                    let started = Instant::now();
                    scheme.set_raw_input(std::str::from_utf8(&typed[..length])?);
                    let candidates = scheme.candidates(&dictionary)?;
                    let elapsed = started.elapsed().as_secs_f64() * 1000.0;
                    // The first round warms SQLite's page cache and the prepared statements.
                    if round > 0 {
                        all.push(elapsed);
                    }
                    if round == 0 && length == typed.len() {
                        rows.push(format!("{}={}", reading.join(" "), candidates.len()));
                    }
                }
            }
        }
        report(label, all);
        println!("{label:<12} candidates: {}", rows.join(", "));
    }
    Ok(())
}
