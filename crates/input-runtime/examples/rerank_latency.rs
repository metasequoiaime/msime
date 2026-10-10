//! Measures what reranking costs per keystroke, and fails when it breaches a frame budget.
//!
//! Why this exists: the decision to ship a sentence model rested on a latency number measured once,
//! by hand, from two separate runs on a machine doing other things. That is enough to choose between
//! two model sizes and not enough to keep the choice honest afterwards. A model swap, a wider
//! lattice, or a larger candidate page all move this number, and nothing would have noticed.
//!
//! Why it measures both arms in one process: the quantity that matters is not how long a keystroke
//! takes, it is how much longer it takes *because* of the reranker. Timing two separate runs
//! measures that difference plus whatever else changed between them — thermal state, page cache,
//! what else the machine was doing. Here the same keystroke is typed twice back to back, once into
//! a runtime with the reranker attached and once into one without, so everything except the
//! reranker cancels.
//!
//! Why two runtimes rather than attaching and detaching one reranker: a `Reranker` owns the cached
//! prefix for its session. Detaching it destroys that cache, so every case would pay to rebuild it
//! and the cost would land on the model arm alone. Each arm keeps its own runtime and its own
//! cache for the whole run, which is also what a running host does.
//!
//! Why the arms alternate: whichever arm runs first on a given case pays for the engine's own cold
//! caches, and that cost would otherwise land entirely on one arm. Odd-numbered cases run the model
//! first, even-numbered cases run it second.
//!
//! What it does not measure: this drives the runtime in-process. It does not include the host
//! boundary, the IPC hop, or the time the platform spends drawing the candidate panel. The budget
//! it checks is therefore a ceiling on this crate's share of a frame, not a guarantee about what a
//! user perceives.
use msime_engine::host::{Command, Session};
use msime_input_runtime::{Action, Reranker, Runtime, SentenceModel};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// One 60 Hz frame. A keystroke that costs more than this cannot be absorbed between two frames,
/// so the candidate list visibly lags the typing.
const FRAME_MS: f64 = 16.0;

/// Percentile to hold to the budget. The mean hides exactly the tail that is worth holding: a
/// reranker's cost grows with the length of the sentence being scored, so the slowest keystrokes
/// are the ones at the end of the longest input, which is where a user is already committed.
const GATE_PERCENTILE: f64 = 95.0;

struct Case {
    id: String,
    input: String,
}

/// Keystroke timings in arrival order, kept unsorted so the two arms stay paired by index.
#[derive(Default)]
struct Arm {
    samples: Vec<Duration>,
}

impl Arm {
    fn ms(&self) -> Vec<f64> {
        self.samples
            .iter()
            .map(|d| d.as_secs_f64() * 1000.0)
            .collect()
    }
}

/// Nearest-rank percentile. Exact on the sample rather than interpolated: with a few thousand
/// keystrokes the difference is noise, and an interpolated p100 would not be an observed keystroke.
fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let rank = (p / 100.0 * sorted.len() as f64).ceil().max(1.0) as usize;
    sorted[rank.min(sorted.len()) - 1]
}

fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f64>() / values.len() as f64
}

struct Summary {
    mean: f64,
    p50: f64,
    p95: f64,
    p99: f64,
    max: f64,
}

fn summarize(values: &[f64]) -> Summary {
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    Summary {
        mean: mean(values),
        p50: percentile(&sorted, 50.0),
        p95: percentile(&sorted, 95.0),
        p99: percentile(&sorted, 99.0),
        max: percentile(&sorted, 100.0),
    }
}

fn row(label: &str, s: &Summary) {
    println!(
        "  {label:<14} mean {:>7.2}  p50 {:>7.2}  p95 {:>7.2}  p99 {:>7.2}  max {:>7.2}",
        s.mean, s.p50, s.p95, s.p99, s.max
    );
}

fn load(path: &Path) -> Result<Vec<Case>, Box<dyn std::error::Error>> {
    let text = std::fs::read_to_string(path)?;
    let mut cases = Vec::new();
    for line in text.lines() {
        if line.starts_with('#') || line.is_empty() || line.starts_with("id\t") {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() < 4 {
            return Err(format!("{}: malformed row: {line}", path.display()).into());
        }
        cases.push(Case {
            id: fields[0].to_string(),
            input: fields[1].to_string(),
        });
    }
    Ok(cases)
}

/// 从空的输入串开始打一条输入，逐键计时。
///
/// 先发 `Cancel` 而不是新建会话：新建会话会重新打开词库，这笔开销会盖过要量的东西，而用户也不会每按一次键都付一次。
///
/// `nine_key` 时把字母换成九宫格上印着它的数字再输入，与 `convert_eval --nine-key` 相同。
fn type_case(
    runtime: &mut Runtime<Session>,
    input: &str,
    nine_key: bool,
    into: &mut Arm,
) -> Result<(), Box<dyn std::error::Error>> {
    runtime.dispatch(Action::Command(Command::Cancel))?;
    let keys: Vec<u8> = if nine_key {
        input.bytes().filter_map(keypad_digit).collect()
    } else {
        input.bytes().collect()
    };
    for byte in keys {
        let started = Instant::now();
        runtime.dispatch(Action::Character {
            value: byte,
            shift: false,
        })?;
        into.samples.push(started.elapsed());
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut resources: Option<PathBuf> = None;
    let mut sets: Vec<PathBuf> = Vec::new();
    let mut budget_ms = FRAME_MS;
    let mut warmup = 5usize;
    let mut limit: Option<usize> = None;
    let mut nine_key = false;
    let mut index = 0;
    while index < args.len() {
        let take = |index: &mut usize| -> Result<String, String> {
            *index += 1;
            args.get(*index)
                .cloned()
                .ok_or_else(|| format!("{} needs a value", args[*index - 1]))
        };
        match args[index].as_str() {
            "--resources" => resources = Some(PathBuf::from(take(&mut index)?)),
            "--set" => sets.push(PathBuf::from(take(&mut index)?)),
            "--budget-ms" => budget_ms = take(&mut index)?.parse()?,
            "--warmup" => warmup = take(&mut index)?.parse()?,
            "--limit" => limit = Some(take(&mut index)?.parse()?),
            "--nine-key" => nine_key = true,
            other => return Err(format!("unknown argument: {other}").into()),
        }
        index += 1;
    }
    let resources = resources.ok_or(
        "usage: rerank_latency --resources <verified-dir> --set <file.tsv> [--budget-ms 16] [--warmup 5] [--limit N] [--nine-key]",
    )?;
    if sets.is_empty() {
        return Err("at least one --set is required".into());
    }

    let mut cases = Vec::new();
    for set in &sets {
        cases.extend(load(set)?);
    }
    if let Some(limit) = limit {
        // Stride rather than truncate: the sets are ordered, and a prefix would be all short inputs
        // — exactly the keystrokes that cost the least.
        let stride = cases.len().div_ceil(limit.max(1));
        cases = cases.into_iter().step_by(stride.max(1)).collect();
    }

    let state = tempfile::tempdir()?;
    let generation = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources/desktop-dictionary.lock.json"),
    )?;
    let generation: serde_json::Value = serde_json::from_str(&generation)?;
    let generation = generation["source_commit"]
        .as_str()
        .ok_or("lock has no source_commit")?;
    let mut options = msime_engine::host::prepare_options(
        resources.to_str().ok_or("non-UTF-8 resource path")?,
        state
            .path()
            .join("user")
            .to_str()
            .ok_or("non-UTF-8 state")?,
        state
            .path()
            .join("cache")
            .to_str()
            .ok_or("non-UTF-8 state")?,
        generation,
    )?;
    options.scheme = 0;
    options.learning = false;
    options.frequency_mode = "disabled".into();
    options.sentence_alternatives = true;

    let model_path = resources.join("sentence-model.safetensors");
    let bytes = std::fs::read(&model_path).map_err(|error| {
        format!(
            "{}: {error}. This measures what the reranker costs, so it needs one to measure.",
            model_path.display()
        )
    })?;
    let model = std::sync::Arc::new(SentenceModel::load(&bytes)?);

    let mut plain = Runtime::new(Session::new(&options)?, 9)?;
    plain.focus(true)?;
    let mut ranked = Runtime::new(Session::new(&options)?, 9)?;
    ranked.focus(true)?;
    ranked.set_reranker(Some(Reranker::new(model)));
    // 九键的长串数字每次按键都要重新解一遍整句，量它的按键延迟要在九宫格上打（#6059）。
    if nine_key {
        plain.set_nine_key_enabled(true)?;
        ranked.set_nine_key_enabled(true)?;
    }

    // The first keystrokes of a process pay for lazily-opened dictionaries and a cold allocator.
    // Those costs are real but they are paid once at launch, not per keystroke, so they belong
    // outside the samples.
    let mut discard = Arm::default();
    for case in cases.iter().take(warmup) {
        type_case(&mut ranked, &case.input, nine_key, &mut discard)?;
        type_case(&mut plain, &case.input, nine_key, &mut discard)?;
    }

    let mut off = Arm::default();
    let mut on = Arm::default();
    // Per-case worst keystroke with the model attached, to name the slowest input rather than
    // report a percentile with nothing behind it.
    let mut worst: Vec<(f64, String)> = Vec::new();
    for (position, case) in cases.iter().enumerate() {
        let before = on.samples.len();
        if position % 2 == 0 {
            type_case(&mut plain, &case.input, nine_key, &mut off)?;
            type_case(&mut ranked, &case.input, nine_key, &mut on)?;
        } else {
            type_case(&mut ranked, &case.input, nine_key, &mut on)?;
            type_case(&mut plain, &case.input, nine_key, &mut off)?;
        }
        let case_max = on.samples[before..]
            .iter()
            .map(|d| d.as_secs_f64() * 1000.0)
            .fold(0.0f64, f64::max);
        worst.push((case_max, format!("{} ({})", case.id, case.input)));
    }

    let off_ms = off.ms();
    let on_ms = on.ms();
    if off_ms.len() != on_ms.len() {
        return Err("arms are not paired; a case typed a different number of keystrokes".into());
    }
    // Paired per keystroke, not a difference of aggregates. Subtracting two p95 values would answer
    // "how do the tails compare", which is a different question from "what does the reranker add".
    let delta: Vec<f64> = on_ms
        .iter()
        .zip(off_ms.iter())
        .map(|(a, b)| a - b)
        .collect();

    let off_summary = summarize(&off_ms);
    let on_summary = summarize(&on_ms);
    let delta_summary = summarize(&delta);

    println!(
        "{} cases, {} keystrokes per arm, {} warm-up cases discarded",
        cases.len(),
        on_ms.len(),
        warmup.min(cases.len())
    );
    println!("milliseconds per keystroke:");
    row("engine only", &off_summary);
    row("with model", &on_summary);
    row("reranker adds", &delta_summary);

    // The share matters as much as the percentile. Reranking is skipped entirely for most
    // keystrokes — the lattice does not run below three syllables and has nothing to choose from
    // when it finds one reading — so a mean spreads a cost that is really paid by a minority of
    // keystrokes across all of them, and a percentile says how bad the tail is without saying how
    // often a user reaches it.
    let over = on_ms.iter().filter(|ms| **ms > budget_ms).count();
    println!(
        "over the {:.2}ms budget: {over} of {} keystrokes ({:.1}%)",
        budget_ms,
        on_ms.len(),
        100.0 * over as f64 / on_ms.len().max(1) as f64
    );

    worst.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
    println!("slowest keystroke by case:");
    for (ms, label) in worst.iter().take(5) {
        println!("  {ms:>7.2}  {label}");
    }

    let observed = percentile(
        &{
            let mut sorted = on_ms.clone();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
            sorted
        },
        GATE_PERCENTILE,
    );
    if observed > budget_ms {
        return Err(format!(
            "p{GATE_PERCENTILE:.0} with the model is {observed:.2}ms, over the {budget_ms:.2}ms budget"
        )
        .into());
    }
    println!(
        "p{GATE_PERCENTILE:.0} with the model is {observed:.2}ms, within the {budget_ms:.2}ms budget"
    );
    Ok(())
}

/// 小写字母在九宫格上对应的数字；其他字符（少数用例里的 `'`）不输入。与 `convert_eval` 的同名函数相同。
fn keypad_digit(letter: u8) -> Option<u8> {
    const KEYPAD: &[u8; 26] = b"22233344455566677778889999";
    letter
        .is_ascii_lowercase()
        .then(|| KEYPAD[usize::from(letter - b'a')])
}
