//! Per-keystroke latency through the C ABI, on the pinned dictionary.
//!
//! Every host calls `character` and then `view` for each key, so that pair is what a person feels
//! as "the keyboard kept up". This reports the distribution rather than a mean: dropped frames come
//! from the slow tail, and a mean hides it behind the many cheap keys in a word.

// Integration tests and examples are their own crates, so the exemption the
// library root carries does not reach them. Same boundary, same reason: this
// target drives the C ABI directly.
#![allow(unsafe_code)]
use msime_host_api::*;
use serde_json::{json, Value};
use std::ffi::{c_char, CString};
use std::time::Instant;

/// Committing candidate selections timed per statistics setting, enough for a stable p95.
const SELECTIONS: usize = 200;

fn read(pointer: *mut c_char) -> Value {
    // SAFETY: each fresh owned host response is consumed exactly once.
    let response = unsafe { CString::from_raw(pointer) };
    let value: Value = serde_json::from_slice(response.as_bytes()).unwrap();
    assert_eq!(value["ok"], true, "{value}");
    value["value"].clone()
}

fn report(label: &str, mut samples: Vec<f64>) {
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let at = |q: f64| samples[((samples.len() - 1) as f64 * q).round() as usize];
    let total: f64 = samples.iter().sum();
    println!(
        "{label:<22} n={:<4} mean={:6.2}ms p50={:6.2}ms p95={:6.2}ms max={:6.2}ms",
        samples.len(),
        total / samples.len() as f64,
        at(0.50),
        at(0.95),
        at(1.00)
    );
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let resources = std::env::args_os()
        .nth(1)
        .ok_or("usage: keystroke_latency <verified-resources> [scheme] [fuzzy-rule,...]")?;
    let scheme = std::env::args().nth(2).unwrap_or_else(|| "quanpin".into());
    // 第三个参数是逗号分隔的模糊音规则名（`z-zh,c-ch,s-sh`），给了就打开模糊音：每键多出模糊读法的查询和合并，要单独量。
    let fuzzy_rules: Vec<String> = std::env::args()
        .nth(3)
        .map(|rules| rules.split(',').map(str::to_owned).collect())
        .unwrap_or_default();
    let state = tempfile::tempdir()?;
    let resources = std::fs::canonicalize(resources)?;
    let request = json!({"resources": resources, "state_root": state.path()}).to_string();
    let mut options: Value =
        read(unsafe { msime_client_prepare_host(request.as_ptr(), request.len()) });
    options["preferences"]["scheme"] = json!(scheme);
    options["preferences"]["candidate_page_size"] = json!(9);
    if !fuzzy_rules.is_empty() {
        options["preferences"]["fuzzy_pinyin"] =
            json!({"enabled": true, "rules": fuzzy_rules, "seeded": true});
    }
    let options_value = options;
    let options = options_value.to_string();
    let created = read(unsafe { msime_client_create(options.as_ptr(), options.len()) });
    let handle = created["session"].as_u64().unwrap();
    read(msime_client_focus(handle, true));

    // Words a person actually types, not one long buffer: the cost of a key depends on how much
    // is already composed, so a realistic run has to keep committing and starting over.
    let words = [
        "nihao",
        "shijie",
        "zhongguo",
        "shurufa",
        "pengyou",
        "gongzuo",
        "xiexie",
        "zaijian",
        "jintian",
        "mingtian",
        "womenderizi",
        "buzhidaogaizenmeban",
    ];
    let mut character = Vec::new();
    let mut view = Vec::new();
    let mut commit = Vec::new();
    for _ in 0..8 {
        for word in words {
            for byte in word.bytes() {
                let started = Instant::now();
                read(msime_client_character(handle, byte, false));
                character.push(started.elapsed().as_secs_f64() * 1000.0);
                let started = Instant::now();
                read(msime_client_view(handle));
                view.push(started.elapsed().as_secs_f64() * 1000.0);
            }
            let started = Instant::now();
            read(msime_client_command(handle, 9));
            commit.push(started.elapsed().as_secs_f64() * 1000.0);
        }
    }

    // The same keys again, but reported by how many are already in the buffer. "Fast typing falls
    // behind" is what a cost that grows with the composition feels like, and an aggregate cannot
    // show that: the long buffers are a minority of the keys and the mean buries them.
    let mut by_position: Vec<Vec<f64>> = vec![Vec::new(); 24];
    // Every sample that took long enough for a person to see it, with where it happened. A stall
    // is not a slow average; it is one key in a hundred that costs a tenth of a second, and the
    // question is always which key and on which pass.
    let mut stalls: Vec<(usize, usize, f64)> = Vec::new();
    for pass in 0..40 {
        // 3 is Cancel. 0 is Backspace, which only removes one key - using it here left the
        // composition growing across passes until it was hundreds of characters long, which is
        // not typing and made the numbers say so.
        read(msime_client_command(handle, 3));
        let long = "buzhidaogaizenmebanzheshi";
        for (index, byte) in long.bytes().enumerate().take(24) {
            let started = Instant::now();
            read(msime_client_character(handle, byte, false));
            read(msime_client_view(handle));
            let ms = started.elapsed().as_secs_f64() * 1000.0;
            by_position[index].push(ms);
            if ms > 16.0 {
                stalls.push((pass, index + 1, ms));
            }
        }
    }

    // Picking a candidate by position - a click, or a tap on a touch keyboard - is the one keystroke that also feeds typing statistics, so it is timed on its own, with statistics on and then off. Each run ends in a focus-out, and that call is timed too because it is where a host's session hands over whatever it has not written yet.
    let user_data = options_value["user_data"].as_str().unwrap().to_owned();
    let statistics = |action: Value| {
        let request = json!({"directory": user_data, "action": action}).to_string();
        read(unsafe { msime_client_typing_statistics(request.as_ptr(), request.len()) })
    };
    let mut selections = Vec::new();
    for enabled in [true, false] {
        statistics(json!({"operation": "set_enabled", "enabled": enabled}));
        read(msime_client_focus(handle, true));
        let mut select = Vec::new();
        for round in 0..SELECTIONS {
            read(msime_client_command(handle, 3));
            let mut current = Value::Null;
            for byte in words[round % words.len()].bytes() {
                current = read(msime_client_character(handle, byte, false))["view"].clone();
            }
            // Only a selection that commits is counted, so a candidate that covers part of the word is followed by the first candidate for the rest until something commits, and only that last call is kept.
            let mut index = round % current["candidates"].as_array().unwrap().len().min(9);
            loop {
                let generation = current["generation"].as_u64().unwrap();
                let started = Instant::now();
                let result = read(msime_client_select(handle, generation, index));
                let elapsed = started.elapsed().as_secs_f64() * 1000.0;
                if !result["commit"].is_null() {
                    select.push(elapsed);
                    break;
                }
                current = result["view"].clone();
                index = 0;
            }
        }
        let started = Instant::now();
        read(msime_client_focus(handle, false));
        let blur = started.elapsed().as_secs_f64() * 1000.0;
        selections.push((enabled, select, blur));
    }
    let recorded = statistics(json!({"operation": "load"}))["selections"].clone();

    println!("scheme={scheme} fuzzy={}", fuzzy_rules.join(","));
    report("character", character);
    report("view", view);
    report("commit", commit);
    for (enabled, samples, blur) in selections {
        let state = if enabled { "on" } else { "off" };
        report(&format!("select, stats {state}"), samples);
        println!("{:<22} {blur:6.2}ms", format!("focus-out, stats {state}"));
    }
    let counted: u64 = recorded["ranks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|count| count.as_u64().unwrap())
        .sum::<u64>()
        + recorded["beyond"].as_u64().unwrap();
    println!("selections counted after focus-out: {counted} of {SELECTIONS}");
    println!("\nby position in the composition (character + view):");
    for (index, samples) in by_position.iter().enumerate() {
        if samples.is_empty() {
            continue;
        }
        let mut sorted = samples.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        println!(
            "  key {:<2} p50={:6.2}ms max={:6.2}ms",
            index + 1,
            sorted[sorted.len() / 2],
            sorted[sorted.len() - 1]
        );
    }
    println!("\nstalls over one frame (16ms): {}", stalls.len());
    for (pass, key, ms) in &stalls {
        println!("  pass {pass:<3} key {key:<2} {ms:7.2}ms");
    }
    read(msime_client_destroy(handle));
    Ok(())
}
