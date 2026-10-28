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
//!
//! `--scheme shuangpin --shuangpin-profile <方案>` 把用例编成 26 键双拼的按键再打；再给 `--key-geometry` 和 `--touch-sigma` 时按 `convert_eval` 同样的规则合成误触（默认 `--touch-mode realistic`），量带着按错的键时每次按键的开销。双拼的纠错整句要加 `--autocorrect` 才会出现。
mod eval_resources;

#[path = "convert_eval/grid_keys.rs"]
mod grid_keys;
#[path = "convert_eval/touch_noise.rs"]
mod touch_noise;

use msime_engine::host::{Command, Session};
use msime_engine::{KeyGrid, SchemeType, ShuangpinProfileKind};
use msime_input_runtime::{Action, Reranker, Runtime, SentenceModel};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use touch_noise::{KeyLayout, TouchMode, TouchNoise};

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
    /// 实际送进去的键，两臂共用。
    keys: Vec<u8>,
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
            keys: Vec::new(),
        });
    }
    Ok(cases)
}

/// 从空的输入串开始打一条输入的 `keys`，逐键计时。
///
/// 先发 `Cancel` 而不是新建会话：新建会话会重新打开词库，这笔开销会盖过要量的东西，而用户也不会每按一次键都付一次。
fn type_case(
    runtime: &mut Runtime<Session>,
    keys: &[u8],
    grid: Option<KeyGrid>,
    into: &mut Arm,
) -> Result<(), Box<dyn std::error::Error>> {
    runtime.dispatch(Action::Command(Command::Cancel))?;
    for &byte in keys {
        let action = match grid {
            Some(
                KeyGrid::FourteenKey
                | KeyGrid::EighteenKey
                | KeyGrid::FourteenKeyShuangpin(_)
                | KeyGrid::EighteenKeyShuangpin(_),
            ) => Action::GridKey(byte),
            Some(KeyGrid::NineKey | KeyGrid::NineKeyShuangpin(_)) | None => Action::Character {
                value: byte,
                shift: false,
            },
        };
        let started = Instant::now();
        runtime.dispatch(action)?;
        into.samples.push(started.elapsed());
    }
    Ok(())
}

fn non_negative_finite(value: &str, option: &str) -> Result<f64, String> {
    match value.parse::<f64>() {
        Ok(value) if value.is_finite() && value >= 0.0 => Ok(value),
        _ => Err(format!(
            "{option}: {value} is not a finite non-negative number"
        )),
    }
}

/// 两臂共用预先算好的按键；无法产生错键的 noisy 用例留空，稍后一起剔除。
fn case_keys(
    input: &str,
    shuangpin: Option<ShuangpinProfileKind>,
    grid: Option<KeyGrid>,
    touch: Option<&(TouchNoise, TouchMode)>,
    seed: u64,
    position: usize,
) -> Result<Vec<u8>, String> {
    match (shuangpin, grid) {
        (Some(profile), _) => {
            let intended = grid_keys::shuangpin_keys(profile.name(), input, None)
                .unwrap_or_default()
                .into_bytes();
            match touch {
                Some((noise, mode)) => {
                    let mut rng = touch_noise::case_rng(seed, &[position as u64]);
                    Ok(noise
                        .synthesize(*mode, &intended, &mut rng)?
                        .unwrap_or_default())
                }
                None => Ok(intended),
            }
        }
        (None, Some(grid)) => Ok(grid_keys::grid_keys(grid, input, None)
            .unwrap_or_default()
            .into_bytes()),
        (None, None) => Ok(input.bytes().collect()),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut resources: Option<PathBuf> = None;
    let mut resource_lock: Option<PathBuf> = None;
    let mut sets: Vec<PathBuf> = Vec::new();
    let mut budget_ms = FRAME_MS;
    let mut warmup = 5usize;
    let mut limit: Option<usize> = None;
    let mut grid: Option<KeyGrid> = None;
    let mut scheme = SchemeType::Quanpin;
    let mut profile: Option<ShuangpinProfileKind> = None;
    let mut autocorrect = false;
    let mut geometry: Vec<PathBuf> = Vec::new();
    let mut touch_mode: Option<TouchMode> = None;
    let mut sigma: Option<f64> = None;
    let mut bias = 0.0;
    let mut seed = 20261027;
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
            "--resource-lock" => resource_lock = Some(PathBuf::from(take(&mut index)?)),
            "--set" => sets.push(PathBuf::from(take(&mut index)?)),
            "--budget-ms" => budget_ms = take(&mut index)?.parse()?,
            "--warmup" => warmup = take(&mut index)?.parse()?,
            "--limit" => limit = Some(take(&mut index)?.parse()?),
            "--grid" => grid = Some(grid_keys::parse_grid(&take(&mut index)?)?),
            // `--grid nine` 的旧写法。
            "--nine-key" => grid = Some(KeyGrid::NineKey),
            "--scheme" => {
                scheme = match take(&mut index)?.as_str() {
                    "quanpin" => SchemeType::Quanpin,
                    "shuangpin" => SchemeType::Shuangpin,
                    other => return Err(format!("unknown scheme: {other}").into()),
                }
            }
            "--shuangpin-profile" => {
                let name = take(&mut index)?;
                profile = Some(
                    ShuangpinProfileKind::from_name(&name)
                        .filter(|kind| *kind != ShuangpinProfileKind::Custom)
                        .ok_or_else(|| format!("unknown shuangpin profile: {name}"))?,
                );
            }
            "--autocorrect" => autocorrect = true,
            "--key-geometry" => geometry.push(PathBuf::from(take(&mut index)?)),
            "--touch-mode" => touch_mode = Some(TouchMode::parse(&take(&mut index)?)?),
            "--touch-sigma" => {
                sigma = Some(non_negative_finite(&take(&mut index)?, "--touch-sigma")?)
            }
            "--touch-bias-y" => bias = non_negative_finite(&take(&mut index)?, "--touch-bias-y")?,
            "--seed" => seed = take(&mut index)?.parse()?,
            other => return Err(format!("unknown argument: {other}").into()),
        }
        index += 1;
    }
    let resources = resources.ok_or(
        "usage: rerank_latency --resources <verified-dir> --set <file.tsv> [--resource-lock <file.json>] [--budget-ms 16] [--warmup 5] [--limit N] [--grid nine|fourteen|shuangpin-<profile>] [--scheme shuangpin --shuangpin-profile <profile> [--autocorrect] [--key-geometry <file.json>... --touch-sigma <σ> [--touch-bias-y <b>] [--touch-mode realistic] [--seed N]]]",
    )?;
    if sets.is_empty() {
        return Err("at least one --set is required".into());
    }
    let shuangpin = match (scheme, profile) {
        (SchemeType::Shuangpin, Some(profile)) if grid.is_none() => Some(profile),
        (SchemeType::Shuangpin, None) => {
            return Err("--scheme shuangpin needs --shuangpin-profile".into())
        }
        (SchemeType::Shuangpin, Some(_)) => {
            return Err(
                "--scheme shuangpin types the 26-key layout; it cannot be combined with --grid"
                    .into(),
            )
        }
        _ if profile.is_some()
            || sigma.is_some()
            || touch_mode.is_some()
            || !geometry.is_empty() =>
        {
            return Err(
                "--shuangpin-profile and the --touch-* options need --scheme shuangpin".into(),
            )
        }
        _ => None,
    };
    // 双拼加误触时用哪份几何、哪种模式；只有带 `;` 的那份配微软双拼，同 `convert_eval`。
    let touch = match (shuangpin, sigma) {
        (Some(profile), Some(sigma)) => {
            let semicolon = profile == ShuangpinProfileKind::Microsoft;
            let layout = geometry
                .iter()
                .map(|path| KeyLayout::load(path))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .find(|layout| layout.has_semicolon() == semicolon)
                .ok_or("--touch-sigma needs a --key-geometry that matches the profile's ; key")?;
            let mode = touch_mode.unwrap_or(TouchMode::Realistic);
            eprintln!(
                "typing {} keys with {} touches on {} at sigma {sigma}, bias {bias}",
                profile.name(),
                mode.name(),
                layout.name
            );
            Some((TouchNoise::new(&layout, sigma, bias), mode))
        }
        (Some(_), None) if touch_mode.is_some() || !geometry.is_empty() => {
            return Err("--key-geometry and --touch-mode need --touch-sigma".into())
        }
        _ => None,
    };

    let verified = eval_resources::verify(&resources, resource_lock.as_deref())?;

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
    // 两臂打同样的键，所以按键在开始计时前算好；切不开或无法满足 noisy 条件的用例不参与计时。
    for (position, case) in cases.iter_mut().enumerate() {
        case.keys = case_keys(&case.input, shuangpin, grid, touch.as_ref(), seed, position)?;
    }
    let before = cases.len();
    cases.retain(|case| !case.keys.is_empty());
    if cases.len() != before {
        eprintln!(
            "skipped {} cases without eligible keystrokes",
            before - cases.len()
        );
    }
    if cases.is_empty() {
        return Err("no eligible keystrokes to measure".into());
    }

    let state = tempfile::tempdir()?;
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
        &verified.source_commit,
    )?;
    options.scheme = 0;
    options.learning = false;
    options.frequency_mode = "disabled".into();
    options.sentence_alternatives = true;
    if let Some(profile) = shuangpin {
        options.scheme = SchemeType::Shuangpin as u8;
        options.shuangpin_profile = profile as u8;
    }
    options.autocorrect_transposition = autocorrect;
    options.autocorrect_neighbor = autocorrect;

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
    // 九键和 14 键的长串组码每次按键都要重新解一遍整句，量它们的按键延迟要在网格上打（#6059）。
    if grid.is_some() {
        plain.set_key_grid(grid)?;
        ranked.set_key_grid(grid)?;
    }

    // The first keystrokes of a process pay for lazily-opened dictionaries and a cold allocator.
    // Those costs are real but they are paid once at launch, not per keystroke, so they belong
    // outside the samples.
    let mut discard = Arm::default();
    for case in cases.iter().take(warmup) {
        type_case(&mut ranked, &case.keys, grid, &mut discard)?;
        type_case(&mut plain, &case.keys, grid, &mut discard)?;
    }

    let mut off = Arm::default();
    let mut on = Arm::default();
    // Per-case worst keystroke with the model attached, to name the slowest input rather than
    // report a percentile with nothing behind it.
    let mut worst: Vec<(f64, String)> = Vec::new();
    for (position, case) in cases.iter().enumerate() {
        let before = on.samples.len();
        if position % 2 == 0 {
            type_case(&mut plain, &case.keys, grid, &mut off)?;
            type_case(&mut ranked, &case.keys, grid, &mut on)?;
        } else {
            type_case(&mut ranked, &case.keys, grid, &mut on)?;
            type_case(&mut plain, &case.keys, grid, &mut off)?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn touch_parameters_reject_non_finite_and_negative_values() {
        for value in ["NaN", "inf", "-inf", "-0.1"] {
            assert!(non_negative_finite(value, "--touch-sigma").is_err());
            assert!(non_negative_finite(value, "--touch-bias-y").is_err());
        }
        assert_eq!(non_negative_finite("0", "--touch-sigma").unwrap(), 0.0);
        assert_eq!(non_negative_finite("0.25", "--touch-sigma").unwrap(), 0.25);
    }

    #[test]
    fn impossible_noisy_cases_do_not_fall_back_to_clean_keys() {
        let layout = KeyLayout::load(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../resources/eval/key-geometry/android-portrait.json"),
        )
        .unwrap();
        let touch = (TouchNoise::new(&layout, 0.0, 0.0), TouchMode::Noisy);
        let keys = |touch| {
            case_keys(
                "nihao",
                Some(ShuangpinProfileKind::Xiaohe),
                None,
                touch,
                7,
                0,
            )
            .unwrap()
        };
        assert!(!keys(None).is_empty());
        assert!(keys(Some(&touch)).is_empty());
        let touch = (TouchNoise::new(&layout, 0.35, 0.0), TouchMode::Noisy);
        assert_ne!(keys(Some(&touch)), keys(None));
    }
}
