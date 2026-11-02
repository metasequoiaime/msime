//! 量一个导出的模型加载和打字时占多少内存（phys_footprint，iOS 按它结束超额的键盘扩展）以及每键要多久，只在苹果系统上能量。
//!
//! `footprint <模型目录> [arena|no-arena] [file|bytes]`：内存池默认关（iOS 宿主的设置），模型默认由 ONNX Runtime 自己读文件（`OnnxScorer::load_file`），`bytes` 改成先读进内存再交给它（`OnnxScorer::load`）。要开 `runtime` 特性。
//!
//! iOS 的内存分配器与 macOS 不同（释放的内存更快还给系统），在 Mac 上量的 phys_footprint 偏高；按 iOS 的口径量要编 `aarch64-apple-ios-sim`，在模拟器里跑：`xcrun simctl spawn <设备> target/aarch64-apple-ios-sim/release/examples/footprint <模型目录>`。
//!
//! 打字负载：一次组字保持同一段上文，按 4 到 27 次键，每次 9 个候选、长 1 到 6 字，前缀随按键增长、互相共享，然后上屏第一个候选接到上文后面；共 80 次组字。
#![allow(unsafe_code)]

use msime_engine::ordering::CandidateScorer;

#[cfg(target_vendor = "apple")]
fn footprint() -> (f64, f64) {
    // rusage_info_v4 按 u64 排：ri_phys_footprint 是第 9 个，ri_lifetime_max_phys_footprint 是第 30 个。
    extern "C" {
        fn proc_pid_rusage(pid: i32, flavor: i32, buffer: *mut u64) -> i32;
        fn getpid() -> i32;
    }
    let mut info = [0u64; 64];
    // SAFETY: RUSAGE_INFO_V4（4）写一个 rusage_info_v4，比 64 个 u64 小。
    let result = unsafe { proc_pid_rusage(getpid(), 4, info.as_mut_ptr()) };
    assert_eq!(result, 0, "proc_pid_rusage failed");
    let mebibytes = |bytes: u64| bytes as f64 / 1048576.0;
    (mebibytes(info[9]), mebibytes(info[30]))
}

#[cfg(not(target_vendor = "apple"))]
fn footprint() -> (f64, f64) {
    panic!("phys_footprint is only measured on Apple systems")
}

/// 一个固定种子的 xorshift，让每次运行打同样的字。
struct Sequence(u64);

impl Sequence {
    fn next(&mut self, below: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as usize % below
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let directory = std::path::Path::new(
        args.first()
            .ok_or("usage: footprint <model dir> [arena|no-arena] [file|bytes]")?,
    );
    let arena = args.iter().any(|arg| arg == "arena");
    let bytes = args.iter().any(|arg| arg == "bytes");
    let read = |name: &str| std::fs::read_to_string(directory.join(name));
    let (info, vocabulary) = (read("model.json")?, read("vocab.json")?);
    let characters: Vec<char> = {
        #[derive(serde::Deserialize)]
        struct Tokens {
            tokens: Vec<String>,
        }
        let tokens: Tokens = serde_json::from_str(&vocabulary)?;
        tokens
            .tokens
            .iter()
            .filter_map(|token| {
                let mut chars = token.chars();
                match (chars.next(), chars.next()) {
                    (Some(c), None) if ('\u{4e00}'..='\u{9fff}').contains(&c) => Some(c),
                    _ => None,
                }
            })
            .take(3000)
            .collect()
    };

    let (before, _) = footprint();
    let started = std::time::Instant::now();
    let model = directory.join("lm_step.ort");
    let mut scorer = if bytes {
        msime_onnx_rerank::OnnxScorer::load(&std::fs::read(&model)?, &info, &vocabulary, 2, arena)?
    } else {
        msime_onnx_rerank::OnnxScorer::load_file(&model, &info, &vocabulary, 2, arena)?
    };
    let load_ms = started.elapsed().as_secs_f64() * 1000.0;
    let (loaded, _) = footprint();
    let calibrated_ms = scorer.calibrate();

    let mut sequence = Sequence(0x9e37_79b9_7f4a_7c15);
    let pick = |sequence: &mut Sequence, within: usize| {
        characters[sequence.next(within.min(characters.len()))]
    };
    let mut context: String = (0..20)
        .map(|_| pick(&mut sequence, characters.len()))
        .collect();
    let mut times = Vec::new();
    let mut highest: f64 = 0.0;
    for _ in 0..80 {
        let keystrokes = 4 + sequence.next(24);
        let stem: Vec<char> = (0..8).map(|_| pick(&mut sequence, 400)).collect();
        let mut chosen = String::new();
        for keystroke in 0..keystrokes {
            let page: Vec<String> = (0..9)
                .map(|candidate| {
                    let length = 1 + (candidate + keystroke) % 6;
                    let mut text: String = stem.iter().take(1 + keystroke / 6).collect();
                    while text.chars().count() < length {
                        text.push(pick(&mut sequence, 1500));
                    }
                    text.chars().take(length).collect()
                })
                .collect();
            let texts: Vec<&str> = page.iter().map(String::as_str).collect();
            let started = std::time::Instant::now();
            scorer.log_probabilities(&context, &texts);
            times.push(started.elapsed().as_secs_f64() * 1000.0);
            highest = highest.max(footprint().0);
            chosen.clone_from(&page[0]);
        }
        context.push_str(&chosen);
        let length = context.chars().count();
        if length > 64 {
            context = context.chars().skip(length - 64).collect();
        }
    }
    times.sort_by(f64::total_cmp);
    let (typed, peak) = footprint();
    let percentile = |p: usize| times[(times.len() * p / 100).min(times.len() - 1)];
    println!(
        "arena={arena} bytes={bytes}: load +{:.1} MiB ({load_ms:.0} ms), calibrate {calibrated_ms:.2} ms, after typing +{:.1} MiB, highest while typing +{:.1} MiB, lifetime peak +{:.1} MiB; {} keystrokes p50 {:.2} ms p95 {:.2} ms",
        loaded - before,
        typed - before,
        highest - before,
        peak - before,
        times.len(),
        percentile(50),
        percentile(95),
    );
    drop(scorer);
    println!("after drop +{:.1} MiB", footprint().0 - before);
    Ok(())
}
