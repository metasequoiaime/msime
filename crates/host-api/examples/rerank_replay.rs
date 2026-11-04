//! Per-keystroke latency of the keyboard reranker as a real host sees it: typed segments replayed through the C ABI with that host's own runtime options.
//!
//! `keystroke_latency` types fixed words and cancels them, so nothing is ever committed and the reranker never sees a context. A real session commits every segment, and the first keystroke that needs the model after a commit used to re-encode the whole committed context (64 characters for the large model), which no test with an empty context could show. This replays segments the way the macOS typing driver does: each one is committed with the highlighted candidate, followed by the driver's 143 ms pause.
//!
//! The host's state directories are swapped for temporary ones and its dictionaries are cloned, so the replay never writes to the host's data. `cases.json` is a list of `{"segments": [[expected, keys], ...]}`, for example built from the test split of `resources/eval/typing-chat-v1.tsv`; `models-root` is the directory holding `rerank-model/` (the macOS bundle's `Contents`). Build with `--features onnx-rerank`.
#![allow(unsafe_code)]
use msime_host_api::*;
use serde_json::{json, Value};
use std::ffi::{c_char, CString};
use std::time::Instant;

fn read(pointer: *mut c_char) -> Value {
    // SAFETY: each fresh owned host response is consumed exactly once.
    let response = unsafe { CString::from_raw(pointer) };
    let value: Value = serde_json::from_slice(response.as_bytes()).unwrap();
    assert_eq!(value["ok"], true, "{value}");
    value["value"].clone()
}

fn quantiles(label: &str, mut samples: Vec<f64>) {
    samples.sort_by(f64::total_cmp);
    let at = |q: f64| samples[((samples.len() - 1) as f64 * q).round() as usize];
    println!(
        "{label:<28} n={:<5} p50={:6.2} p95={:6.2} p99={:6.2} max={:6.2} ms",
        samples.len(),
        at(0.5),
        at(0.95),
        at(0.99),
        at(1.0)
    );
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let options_path = args.next().ok_or(
        "usage: rerank_replay <runtime-options.json> <cases.json> <models-root> <on|off> [rounds]",
    )?;
    let cases_path = args.next().ok_or("cases")?;
    let models = args.next().ok_or("models")?;
    let arm = args.next().ok_or("arm")?;
    let rounds: usize = args.next().map(|r| r.parse().unwrap()).unwrap_or(1);
    let state = tempfile::tempdir()?;
    let mut options: Value = serde_json::from_slice(&std::fs::read(options_path)?)?;
    for key in [
        "cache",
        "dictionaries",
        "preferences_directory",
        "user_data",
    ] {
        let directory = state.path().join(key);
        if key == "dictionaries" {
            // The host's working copy of the system dictionaries, with what it has learned; cloned (APFS copy-on-write) so the replay never writes to it.
            let source = options[key].as_str().unwrap().to_owned();
            assert!(std::process::Command::new("/bin/cp")
                .args(["-c", "-R", &source])
                .arg(&directory)
                .status()?
                .success());
        } else {
            std::fs::create_dir_all(&directory)?;
        }
        options[key] = json!(directory);
    }
    options["rerank_models"] = json!(models);
    let preferences = &mut options["preferences"];
    preferences["scheme"] = json!("quanpin");
    preferences["learning"] = json!(false);
    preferences["sentence_association"]["neural_keyboard"] = json!(arm == "on");
    preferences["keyboard_reranker_migrated"] = json!(true);
    let options = options.to_string();
    let created = read(unsafe { msime_client_create(options.as_ptr(), options.len()) });
    let handle = created["session"].as_u64().unwrap();
    read(msime_client_focus(handle, true));

    let cases: Value = serde_json::from_slice(&std::fs::read(cases_path)?)?;
    let mut character = Vec::new();
    let mut view = Vec::new();
    let mut by_length: Vec<Vec<f64>> = vec![Vec::new(); 8];
    let (mut segments, mut correct) = (0, 0);
    for _ in 0..rounds {
        for case in cases.as_array().unwrap() {
            for segment in case["segments"].as_array().unwrap() {
                let (expected, keys) = (segment[0].as_str().unwrap(), segment[1].as_str().unwrap());
                let mut current = Value::Null;
                for (index, byte) in keys.bytes().enumerate() {
                    let started = Instant::now();
                    read(msime_client_character(handle, byte, false));
                    let ms = started.elapsed().as_secs_f64() * 1000.0;
                    character.push(ms);
                    by_length[(index / 6).min(7)].push(ms);
                    let started = Instant::now();
                    current = read(msime_client_view(handle));
                    view.push(started.elapsed().as_secs_f64() * 1000.0);
                }
                segments += 1;
                let first = current["candidates"][0]["text"]
                    .as_str()
                    .unwrap_or_default();
                if first == expected {
                    correct += 1;
                }
                // Space commits what is highlighted, as the typing driver does, so the committed text becomes the context the next segment is ranked against.
                for _ in 0..12 {
                    read(msime_client_command(handle, 1));
                    if read(msime_client_view(handle))["editing_text"]
                        .as_str()
                        .unwrap_or_default()
                        .is_empty()
                    {
                        break;
                    }
                }
                // A person takes a moment before the next segment's first key; the typing driver waits 143 ms.
                std::thread::sleep(std::time::Duration::from_millis(143));
            }
        }
    }
    read(msime_client_command(handle, 3));
    println!(
        "arm={arm} segments={segments} first={correct} ({:.1}%)",
        100.0 * correct as f64 / segments as f64
    );
    let total: Vec<f64> = character.iter().zip(&view).map(|(c, v)| c + v).collect();
    quantiles("character", character);
    quantiles("view", view);
    quantiles("character+view", total);
    for (index, samples) in by_length.into_iter().enumerate() {
        if samples.len() > 20 {
            quantiles(
                &format!("  keys {}-{}", index * 6 + 1, index * 6 + 6),
                samples,
            );
        }
    }
    read(msime_client_destroy(handle));
    Ok(())
}
