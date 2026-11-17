//! Per-keystroke latency of the keyboard reranker as a real host sees it: typed segments replayed through the C ABI with that host's own runtime options.
//!
//! `keystroke_latency` types fixed words and cancels them, so nothing is ever committed and the reranker never sees a context. A real session commits every segment, and the first keystroke that needs the model after a commit used to re-encode the whole committed context (64 characters for the large model), which no test with an empty context could show. This replays segments the way the macOS typing driver does: each one is committed with the highlighted candidate, followed by the driver's 143 ms pause.
//!
//! 上屏照 macOS 的空格做：按当前视图里高亮项的代次和序号调 `msime_client_select`（InputController.mm 的 Space 分支），所以上屏的就是屏幕上那一行。`first` 数的是上屏前屏幕上的首选，`committed` 数的是真正上屏的文字；每段一行 `segment` 给出两者，方便逐段比较两臂。
//!
//! 四臂：`off` 不挂模型；`on` 模型挂在按键上；`pause` 写 `pause_rerank`，每段打完停下、把停顿重解做完再上屏；`rush` 同样写 `pause_rerank`，打完立刻上屏，上屏前像 macOS 宿主那样调 `msime_client_finish_pause_rerank`，量手快的人在停顿重解没跑时上屏的首选与那一键的耗时；`rhythm` 照打字节奏回放，每键之后留 `gap-ms` 的空隙，宿主在空隙里等 `delay-ms` 再分片跑停顿重解（`idle_slices`），报用户感到的字母键与空格（等前一片的时间加这一键自己的耗时），用来定宿主起跑的延迟。
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

/// `rhythm` 臂里两键之间的那段空隙，照 macOS 宿主的做法：空隙比 `delay` 长时开始调 `msime_client_start_settled_rerank`，回 `continue` 就接着跑下一片，直到解完或空隙用完；空隙没用完的部分真的睡掉，让上屏后编码上文的后台线程照实际那样在空隙里跑。一片跑不了一半，所以返回超出空隙的毫秒数：用户的下一键要等它。
fn idle_slices(handle: u64, delay: f64, gap: f64) -> f64 {
    let mut spent = 0.0;
    if gap > delay {
        std::thread::sleep(std::time::Duration::from_secs_f64(delay / 1000.0));
        spent = delay;
        while spent < gap {
            let started = Instant::now();
            let reply = read(settled::msime_client_start_settled_rerank(handle));
            spent += started.elapsed().as_secs_f64() * 1000.0;
            if reply["status"] != "continue" {
                break;
            }
        }
    }
    if spent < gap {
        std::thread::sleep(std::time::Duration::from_secs_f64((gap - spent) / 1000.0));
    }
    (spent - gap).max(0.0)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let options_path = args.next().ok_or(
        "usage: rerank_replay <runtime-options.json> <cases.json> <models-root> <on|off|pause|rush|rhythm> [rounds] [delay-ms] [gap-ms]",
    )?;
    let cases_path = args.next().ok_or("cases")?;
    let models = args.next().ok_or("models")?;
    let arm = args.next().ok_or("arm")?;
    if !["on", "off", "pause", "rush", "rhythm"].contains(&arm.as_str()) {
        return Err(format!("unknown arm {arm}").into());
    }
    let rounds: usize = args.next().map(|r| r.parse().unwrap()).unwrap_or(1);
    // `rhythm`：宿主在两键之间的空隙（`gap`）里等 `delay` 再分片跑停顿重解，见 `idle_slices`。默认是 macOS 的 kSettledRerankDelay 和打字驱动的 143 ms。
    let delay_ms: f64 = args.next().map(|v| v.parse().unwrap()).unwrap_or(150.0);
    let gap_ms: f64 = args.next().map(|v| v.parse().unwrap()).unwrap_or(143.0);
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
    // `pause` 与 `rush` 像 macOS 输入法和鸿蒙、安卓的键盘一样在停顿时重解（HostOptions 的 `pause_rerank`）：桌面上模型一律延后，手机上跑不进逐键预算时延后。
    let pausing = arm == "pause" || arm == "rush" || arm == "rhythm";
    options["pause_rerank"] = json!(pausing);
    let preferences = &mut options["preferences"];
    preferences["scheme"] = json!("quanpin");
    preferences["learning"] = json!(false);
    preferences["sentence_association"]["neural_keyboard"] = json!(arm != "off");
    preferences["keyboard_reranker_migrated"] = json!(true);
    let options = options.to_string();
    let created = read(unsafe { msime_client_create(options.as_ptr(), options.len()) });
    let handle = created["session"].as_u64().unwrap();
    read(msime_client_focus(handle, true));

    let cases: Value = serde_json::from_slice(&std::fs::read(cases_path)?)?;
    let mut character = Vec::new();
    let mut view = Vec::new();
    // 仅 `pause`：每段之后的重解调用及随后的首键耗时；模型挂上再摘下时，引擎的整句缓存已清空。
    let mut settle = Vec::new();
    let mut after_settle = Vec::new();
    // 一次停顿的调用次数及平均时长；停顿期间到达的按键最多等当前这一片。
    let mut slices = Vec::new();
    let mut slice = Vec::new();
    let mut by_length: Vec<Vec<f64>> = vec![Vec::new(); 8];
    let (mut segments, mut correct) = (0, 0);
    let (mut committed_correct, mut shown_not_landed) = (0, 0);
    let mut commit_key = Vec::new();
    // 仅 `rhythm`：用户感到的每个字母键和空格，等前一片的时间加上这一键自己的耗时。
    let mut felt_letter = Vec::new();
    let mut felt_space = Vec::new();
    let mut waiting = 0.0;
    for _ in 0..rounds {
        for case in cases.as_array().unwrap() {
            for segment in case["segments"].as_array().unwrap() {
                let (expected, keys) = (segment[0].as_str().unwrap(), segment[1].as_str().unwrap());
                for (index, byte) in keys.bytes().enumerate() {
                    let started = Instant::now();
                    read(msime_client_character(handle, byte, false));
                    let ms = started.elapsed().as_secs_f64() * 1000.0;
                    character.push(ms);
                    if index == 0 && !settle.is_empty() {
                        after_settle.push(ms);
                    }
                    by_length[(index / 6).min(7)].push(ms);
                    let started = Instant::now();
                    read(msime_client_view(handle));
                    let drawn = started.elapsed().as_secs_f64() * 1000.0;
                    view.push(drawn);
                    if arm == "rhythm" {
                        felt_letter.push(waiting + ms + drawn);
                        waiting = idle_slices(handle, delay_ms, gap_ms);
                    }
                }
                if arm == "pause" {
                    // 模拟宿主停顿；工作已转到后台时，像真实宿主一样轮询到完成。
                    let started = Instant::now();
                    let mut reply = read(settled::msime_client_start_settled_rerank(handle));
                    let mut calls = 1;
                    loop {
                        if reply["status"] == "pending" {
                            std::thread::sleep(std::time::Duration::from_micros(200));
                            let token = reply["token"].as_u64().unwrap();
                            reply = read(settled::msime_client_poll_settled_rerank(
                                handle, token, true,
                            ));
                        } else if reply["status"] == "continue" {
                            // 延后模型耗尽本片预算；真实宿主在这里处理排队输入，回放中没有待处理输入。
                            reply = read(settled::msime_client_start_settled_rerank(handle));
                            calls += 1;
                        } else {
                            break;
                        }
                    }
                    slice.push(started.elapsed().as_secs_f64() * 1000.0 / f64::from(calls));
                    slices.push(f64::from(calls));
                    settle.push(started.elapsed().as_secs_f64() * 1000.0);
                }
                segments += 1;
                // Space commits what is highlighted, as the typing driver does, so the committed text becomes the context the next segment is ranked against. 像 macOS 的空格那样按屏幕上高亮项的身份选词；`rush` 先调 `msime_client_finish_pause_rerank`，与宿主在空格上做的一样，那一键的耗时算进 `commit key`。
                let mut landed = String::new();
                let mut first = None;
                for _ in 0..12 {
                    let started = Instant::now();
                    if arm == "rush" || arm == "rhythm" {
                        read(settled::msime_client_finish_pause_rerank(handle));
                    }
                    let shown = read(msime_client_view(handle));
                    first.get_or_insert_with(|| {
                        shown["candidates"][0]["text"]
                            .as_str()
                            .unwrap_or_default()
                            .to_owned()
                    });
                    let generation = shown["generation"].as_u64().unwrap_or(0);
                    let highlighted = shown["highlighted"].as_u64().unwrap_or(0) as usize;
                    let reply = read(msime_client_select(handle, generation, highlighted));
                    let ms = started.elapsed().as_secs_f64() * 1000.0;
                    commit_key.push(ms);
                    if arm == "rhythm" {
                        // 只有第一次空格前有一段打字的空隙；同一段里接着按的空格（分几次上屏的长句）不再等。
                        felt_space.push(std::mem::take(&mut waiting) + ms);
                    }
                    if let Some(text) = reply["commit"].as_str() {
                        landed.push_str(text);
                    }
                    if read(msime_client_view(handle))["editing_text"]
                        .as_str()
                        .unwrap_or_default()
                        .is_empty()
                    {
                        break;
                    }
                }
                let first = first.unwrap_or_default();
                if first == expected {
                    correct += 1;
                }
                if landed == expected {
                    committed_correct += 1;
                }
                if first == expected && landed != expected {
                    shown_not_landed += 1;
                }
                println!("segment\t{segments}\t{expected}\t{first}\t{landed}");
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
    println!(
        "committed={committed_correct} ({:.1}%) shown_but_not_landed={shown_not_landed}",
        100.0 * committed_correct as f64 / segments as f64
    );
    quantiles("commit key", commit_key);
    if arm == "rhythm" {
        println!("rhythm delay={delay_ms} ms gap={gap_ms} ms");
        quantiles("felt letter", felt_letter);
        quantiles("felt space", felt_space);
    }
    let total: Vec<f64> = character.iter().zip(&view).map(|(c, v)| c + v).collect();
    quantiles("character", character);
    quantiles("view", view);
    quantiles("character+view", total);
    if !settle.is_empty() {
        quantiles("settle (pause)", settle);
        quantiles("settle slice", slice);
        let mut counts = slices;
        counts.sort_by(f64::total_cmp);
        println!(
            "{:<28} p50={} p95={} max={}",
            "slices per pause",
            counts[counts.len() / 2],
            counts[((counts.len() - 1) as f64 * 0.95).round() as usize],
            counts[counts.len() - 1]
        );
        quantiles("first key after settle", after_settle);
    }
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
