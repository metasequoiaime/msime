//! Per-digit nine-key latency on the pinned dictionary.
//!
//! Each digit is timed through `character` plus `snapshot`, the pair every host runs per key. The words are the ones typed on the iPhone XR trace that showed 15-72 ms per digit, each committed with Space so the next word starts from an empty composition, plus one long sentence typed without committing, because the cost of a digit grows with the length already composed. The report groups digits by their position in the composition and prints the distribution, not a mean: a person feels the slow tail.
use msime_engine::host::{prepare_options, Session};
use std::time::Instant;

/// Nine-key digits for 你好, 我们, 今天天气, 很好, then 今天天气很好 as one composition, then two whole sentences typed without committing: 感觉现在还挺准的 (26 digits) and the same continued to 47 digits, one under `DIGIT_LIMIT`.
const WORDS: [&str; 7] = [
    "64426",
    "96636",
    "5468426842674",
    "436426",
    "5468426842674436426",
    "42658394269244248464948633",
    "42658394269244248464948633966366464842694747878",
];
/// Rounds when `NINEKEY_ROUNDS` is unset; a profiler needs a longer run.
const DEFAULT_ROUNDS: usize = 20;

fn report(label: &str, mut samples: Vec<f64>) {
    if samples.is_empty() {
        return;
    }
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let at = |q: f64| samples[((samples.len() - 1) as f64 * q).round() as usize];
    println!(
        "{label:<14} n={:<5} p50={:7.2}ms p95={:7.2}ms max={:7.2}ms",
        samples.len(),
        at(0.50),
        at(0.95),
        at(1.00)
    );
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let resources = std::env::args_os()
        .nth(1)
        .ok_or("usage: ninekey_latency <verified-dictionary-directory>")?;
    let resources = std::fs::canonicalize(resources)?;
    let state = tempfile::tempdir()?;
    let options = prepare_options(
        resources.to_str().ok_or("non-UTF-8 resource path")?,
        state.path().join("user").to_str().unwrap(),
        state.path().join("cache").to_str().unwrap(),
        "nine-key-latency",
    )?;
    let mut session = Session::new(&options)?;
    session.set_nine_key_enabled(true)?;

    let mut all = Vec::new();
    let mut by_position: Vec<Vec<f64>> = Vec::new();
    let rounds = std::env::var("NINEKEY_ROUNDS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_ROUNDS);
    for round in 0..rounds {
        for word in WORDS {
            for (position, digit) in word.bytes().enumerate() {
                let started = Instant::now();
                session.character(digit, false)?;
                let snapshot = session.snapshot()?;
                let elapsed = started.elapsed().as_secs_f64() * 1_000.0;
                assert!(
                    !snapshot.candidates.is_empty(),
                    "{word} produced no candidates at digit {position}"
                );
                // The first round warms caches and the dictionary pages; a person types many words.
                if round > 0 {
                    all.push(elapsed);
                    if by_position.len() <= position {
                        by_position.resize_with(position + 1, Vec::new);
                    }
                    by_position[position].push(elapsed);
                }
            }
            // Commit the way a person does: pick the first candidate until nothing is left composing. A first candidate that covers only part of the digits leaves the rest for another pick.
            for _ in 0..word.len() {
                if session.snapshot()?.editing_text.is_empty() {
                    break;
                }
                session.select(0)?;
            }
            assert!(
                session.snapshot()?.editing_text.is_empty(),
                "{word} was not committed"
            );
        }
    }
    report("all digits", all);
    for (position, samples) in by_position.into_iter().enumerate() {
        report(&format!("digit #{}", position + 1), samples);
    }
    Ok(())
}
