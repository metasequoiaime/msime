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
//! `--scheme shuangpin --shuangpin-profile <方案>` 把用例编成 26 键双拼的按键再打；再给 `--key-geometry` 和 `--touch-sigma` 时按 `convert_eval` 同样的规则合成误触和落点（默认 `--touch-mode realistic`），像触屏宿主那样注册几何、每键带上按下点，量带着按错的键时每次按键的开销，换算触点的那一步也计在这一键里。`--no-touch-points` 只送按到的键（平面表）。`--touch-arms` 把两臂换成都挂模型、一臂送落点一臂不送，配对量出触点计价本身每键多花多少；再加 `--no-touch-points` 两臂都不送（A/A），量出同一台机器上配对增量本身的噪声底，触点那一臂的增量要和它比。双拼的纠错整句要加 `--autocorrect` 才会出现。
mod eval_resources;

#[path = "convert_eval/grid_keys.rs"]
mod grid_keys;
#[path = "convert_eval/touch_noise.rs"]
mod touch_noise;

use msime_engine::host::{Command, Session};
use msime_engine::touch::KeyGeometry;
use msime_engine::{KeyGrid, SchemeType, ShuangpinProfileKind};
use msime_input_runtime::{Action, Reranker, Runtime, SentenceModel};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use touch_noise::{KeyLayout, Point, TouchMode, TouchNoise};

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
    /// 每个键的落点，与 `keys` 对齐；不送落点时为空。
    points: Vec<Point>,
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
            points: Vec::new(),
        });
    }
    Ok(cases)
}

/// 从空的输入串开始打一条输入的 `keys`，逐键计时。`points` 不为空时每键像 `msime_client_touch_character` 那样按代次 `geometry_id` 换成触点、派发前交上、派发后清掉，这几步一起计时。
///
/// 先发 `Cancel` 而不是新建会话：新建会话会重新打开词库，这笔开销会盖过要量的东西，而用户也不会每按一次键都付一次。
fn type_case(
    runtime: &mut Runtime<Session>,
    keys: &[u8],
    points: &[Point],
    geometry_id: u32,
    grid: Option<KeyGrid>,
    into: &mut Arm,
) -> Result<(), Box<dyn std::error::Error>> {
    runtime.dispatch(Action::Command(Command::Cancel))?;
    for (position, &byte) in keys.iter().enumerate() {
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
        let touched = points.get(position).map(|&(x, y)| {
            let touch = runtime.key_touch(byte, geometry_id, x as f32, y as f32);
            runtime.set_key_touch(touch);
        });
        runtime.dispatch(action)?;
        if touched.is_some() {
            runtime.set_key_touch(None);
        }
        into.samples.push(started.elapsed());
    }
    Ok(())
}

/// 算好按键的用例，和不计时的条数。
struct Typed {
    /// 要计时的用例，`keys` 已填好，送落点时 `points` 也填好。
    cases: Vec<Case>,
    /// 编不出来、不计时的条数。
    unencodable: usize,
    /// noisy 下没有一键可能按错、不计时的条数。
    unmissable: usize,
}

impl Typed {
    /// 打在 stderr 的条数说明，一种原因一行：26 键双拼下总有编不出来的那一行，网格上有编不出来的才打；noisy 下再加没有一键可能按错的那一行。
    fn notes(
        &self,
        shuangpin: Option<ShuangpinProfileKind>,
        mode: Option<TouchMode>,
    ) -> Vec<String> {
        let mut lines = Vec::new();
        let encoded = self.cases.len() + self.unmissable;
        let unencodable = |label: &str| {
            format!(
                "{label}: {} of {} cases do not encode and are not timed",
                self.unencodable,
                encoded + self.unencodable
            )
        };
        match shuangpin {
            Some(profile) => lines.push(unencodable(profile.name())),
            None if self.unencodable > 0 => lines.push(unencodable("--grid")),
            None => {}
        }
        if let (Some(profile), Some(TouchMode::Noisy)) = (shuangpin, mode) {
            lines.push(format!(
                "{}: {} of {} encoded cases have no key that can be missed in noisy mode and are not timed",
                profile.name(),
                self.unmissable,
                encoded
            ));
        }
        lines
    }
}

/// 两臂打同样的键，所以按键在开始计时前算好。给了 `grid` 时把字母换成它所在键的组码再输入（`'` 不输入），与 `convert_eval --grid` 相同；九键双拼按音节最少的切法编码。
///
/// 合成误触时每键同时抽出落点，`touch_points` 为真才记进 `points`；按到的键与不送落点时逐个相同。
///
/// 编不出来的用例（九键双拼切不开；26 键双拼切不开，或有音节在这个方案里编不出来）不计时，否则空串白占一条预热和一行用例数。noisy 下没有一键可能按错的用例（σ 太小，按错的概率在积分里为 0）也不计时：退回想按的键会让 noisy 的数字混进没按错的用例。两者都计数。随机数流按抽子集后的序号取，去掉一条用例不会挪动其余用例的流。
fn encode_cases(
    cases: Vec<Case>,
    shuangpin: Option<ShuangpinProfileKind>,
    grid: Option<KeyGrid>,
    touch: Option<(&TouchNoise, TouchMode)>,
    touch_points: bool,
    seed: u64,
) -> Result<Typed, String> {
    let mut typed = Typed {
        cases: Vec::with_capacity(cases.len()),
        unencodable: 0,
        unmissable: 0,
    };
    for (position, mut case) in cases.into_iter().enumerate() {
        case.keys = match (shuangpin, grid) {
            (Some(profile), _) => {
                let Some(intended) = grid_keys::shuangpin_keys(profile.name(), &case.input, None)
                else {
                    typed.unencodable += 1;
                    continue;
                };
                let intended = intended.into_bytes();
                match touch {
                    Some((noise, mode)) => {
                        let mut rng = touch_noise::case_rng(seed, &[position as u64]);
                        match noise.synthesize_touches(mode, &intended, &mut rng)? {
                            Some(touches) => {
                                if touch_points {
                                    case.points = touches.iter().map(|(_, point)| *point).collect();
                                }
                                touches.into_iter().map(|(key, _)| key).collect()
                            }
                            None => {
                                typed.unmissable += 1;
                                continue;
                            }
                        }
                    }
                    None => intended,
                }
            }
            (None, Some(grid)) => {
                let Some(keys) = grid_keys::grid_keys(grid, &case.input, None) else {
                    typed.unencodable += 1;
                    continue;
                };
                keys.into_bytes()
            }
            (None, None) => case.input.bytes().collect(),
        };
        typed.cases.push(case);
    }
    Ok(typed)
}

#[cfg(feature = "onnx-rerank")]
fn onnx_scorer(
    directory: &Path,
    threads: usize,
) -> Result<Box<dyn msime_input_runtime::CandidateScorer>, Box<dyn std::error::Error>> {
    let read = |name: &str| std::fs::read_to_string(directory.join(name));
    let model = std::fs::read(directory.join("lm_step.ort"))?;
    Ok(Box::new(msime_onnx_rerank::OnnxScorer::load(
        &model,
        &read("model.json")?,
        &read("vocab.json")?,
        threads,
    )?))
}

#[cfg(not(feature = "onnx-rerank"))]
fn onnx_scorer(
    _: &Path,
    _: usize,
) -> Result<Box<dyn msime_input_runtime::CandidateScorer>, Box<dyn std::error::Error>> {
    Err("--onnx-model needs the onnx-rerank feature (and ORT_LIB_LOCATION pointing at a minimal ONNX Runtime build)".into())
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
    // `--onnx-model <目录>`：模型臂挂 ONNX Runtime 后端（`export_step.py` 导出的 `lm_step.ort`、`model.json`、`vocab.json`），不挂随包的句子模型；`--onnx-threads` 是它一次图运行用几个线程。
    let mut onnx_model: Option<PathBuf> = None;
    let mut onnx_threads = 2usize;
    let mut seed = 20261027;
    let mut touch_points = true;
    let mut touch_arms = false;
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
                profile = Some(touch_noise::parse_profile(&take(&mut index)?)?)
            }
            "--autocorrect" => autocorrect = true,
            "--key-geometry" => geometry.push(PathBuf::from(take(&mut index)?)),
            "--touch-mode" => touch_mode = Some(TouchMode::parse(&take(&mut index)?)?),
            "--touch-sigma" => sigma = Some(touch_noise::parse_sigma(&take(&mut index)?)?),
            "--touch-bias-y" => bias = touch_noise::parse_bias_y(&take(&mut index)?)?,
            "--seed" => seed = take(&mut index)?.parse()?,
            "--no-touch-points" => touch_points = false,
            "--touch-arms" => touch_arms = true,
            "--onnx-model" => onnx_model = Some(PathBuf::from(take(&mut index)?)),
            "--onnx-threads" => onnx_threads = take(&mut index)?.parse()?,
            other => return Err(format!("unknown argument: {other}").into()),
        }
        index += 1;
    }
    let resources = resources.ok_or(
        "usage: rerank_latency --resources <verified-dir> --set <file.tsv> [--resource-lock <file.json>] [--budget-ms 16] [--warmup 5] [--limit N] [--grid nine|fourteen|shuangpin-<profile>] [--scheme shuangpin --shuangpin-profile <profile> [--autocorrect] [--key-geometry <file.json>... --touch-sigma <σ> [--touch-bias-y <b>] [--touch-mode realistic] [--seed N] [--no-touch-points] [--touch-arms]]] [--onnx-model <dir> [--onnx-threads 2]]",
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
    // 双拼加误触时用哪份几何、哪种模式；按方案用不用 `;` 键挑几何，同 `convert_eval`。
    let touch = match (shuangpin, sigma) {
        (Some(profile), Some(sigma)) => {
            let layouts = geometry
                .iter()
                .map(|path| KeyLayout::load(path))
                .collect::<Result<Vec<_>, _>>()?;
            let layouts = touch_noise::select_layouts(layouts, &[profile], true)?;
            // 只选了一个方案且要合成误触，留下的恰好一份。
            let layout = &layouts[0];
            let mode = touch_mode.unwrap_or(TouchMode::Realistic);
            eprintln!(
                "typing {} keys with {} touches on {} at sigma {sigma}, bias {bias}",
                profile.name(),
                mode.name(),
                layout.name
            );
            let geometry = KeyGeometry::from_json(&layout.json)?
                .ok_or("a reference geometry needs a non-zero id")?;
            Some((TouchNoise::new(layout, sigma, bias), mode, geometry))
        }
        (Some(_), None) if touch_mode.is_some() || !geometry.is_empty() => {
            return Err("--key-geometry and --touch-mode need --touch-sigma".into())
        }
        _ if touch_arms || !touch_points => {
            return Err("--touch-arms and --no-touch-points need --touch-sigma".into())
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
    let typed = encode_cases(
        cases,
        shuangpin,
        grid,
        touch.as_ref().map(|(noise, mode, _)| (noise, *mode)),
        touch_points,
        seed,
    )?;
    for line in typed.notes(shuangpin, touch.as_ref().map(|(_, mode, _)| *mode)) {
        eprintln!("{line}");
    }
    // 全部不计时就没有可比的样本，不能当作过了预算。
    if typed.cases.is_empty() {
        return Err("no eligible keystrokes to measure".into());
    }
    let cases = typed.cases;

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
    match &onnx_model {
        Some(directory) => ranked.set_candidate_scorer(Some(onnx_scorer(directory, onnx_threads)?)),
        None => ranked.set_reranker(Some(Reranker::new(model.clone()))),
    }
    // `--touch-arms`：两臂都挂模型，只差送不送落点。
    if touch_arms {
        plain.set_reranker(Some(Reranker::new(model)));
    }
    // 送落点时两臂都注册同一份几何；不送落点的那一臂注册了也用不上。
    let geometry_id = match &touch {
        Some((_, _, geometry)) if touch_points => {
            plain.set_key_geometry(Some(geometry.clone()));
            ranked.set_key_geometry(Some(geometry.clone()));
            geometry.id()
        }
        _ => 0,
    };
    // 不送落点的那一臂（`--touch-arms` 的 plain）打同样的键、不带落点。
    let plain_points = |case: &Case| -> Vec<Point> {
        if touch_arms {
            Vec::new()
        } else {
            case.points.clone()
        }
    };
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
        type_case(
            &mut ranked,
            &case.keys,
            &case.points,
            geometry_id,
            grid,
            &mut discard,
        )?;
        type_case(
            &mut plain,
            &case.keys,
            &plain_points(case),
            geometry_id,
            grid,
            &mut discard,
        )?;
    }

    let mut off = Arm::default();
    let mut on = Arm::default();
    // Per-case worst keystroke with the model attached, to name the slowest input rather than
    // report a percentile with nothing behind it.
    let mut worst: Vec<(f64, String)> = Vec::new();
    for (position, case) in cases.iter().enumerate() {
        let before = on.samples.len();
        let flat = plain_points(case);
        if position % 2 == 0 {
            type_case(&mut plain, &case.keys, &flat, geometry_id, grid, &mut off)?;
            type_case(
                &mut ranked,
                &case.keys,
                &case.points,
                geometry_id,
                grid,
                &mut on,
            )?;
        } else {
            type_case(
                &mut ranked,
                &case.keys,
                &case.points,
                geometry_id,
                grid,
                &mut on,
            )?;
            type_case(&mut plain, &case.keys, &flat, geometry_id, grid, &mut off)?;
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
    if touch_arms && !touch_points {
        // A/A：两臂做完全相同的事，增量只有计时噪声。
        row("flat table", &off_summary);
        row("flat again", &on_summary);
        row("noise", &delta_summary);
    } else if touch_arms {
        row("flat table", &off_summary);
        row("with touches", &on_summary);
        row("touches add", &delta_summary);
    } else {
        row("engine only", &off_summary);
        row("with model", &on_summary);
        row("reranker adds", &delta_summary);
    }

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

    fn case(input: &str) -> Case {
        Case {
            id: input.to_owned(),
            input: input.to_owned(),
            keys: Vec::new(),
            points: Vec::new(),
        }
    }

    fn android_portrait() -> KeyLayout {
        KeyLayout::load(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../resources/eval/key-geometry/android-portrait.json"),
        )
        .unwrap()
    }

    fn intended(input: &str) -> Vec<u8> {
        grid_keys::shuangpin_keys("xiaohe", input, None)
            .unwrap()
            .into_bytes()
    }

    fn inputs(typed: &Typed) -> Vec<&str> {
        typed.cases.iter().map(|case| case.input.as_str()).collect()
    }

    const XIAOHE: Option<ShuangpinProfileKind> = Some(ShuangpinProfileKind::Xiaohe);

    #[test]
    fn shuangpin_cases_that_do_not_encode_are_skipped_and_counted() {
        let typed = encode_cases(
            vec![case("nihao"), case("xyz"), case("zhongguo")],
            XIAOHE,
            None,
            None,
            true,
            1,
        )
        .unwrap();
        assert_eq!(typed.unencodable, 1);
        assert_eq!(typed.unmissable, 0);
        assert_eq!(inputs(&typed), ["nihao", "zhongguo"]);
        assert_eq!(typed.cases[0].keys, intended("nihao"));
        assert_eq!(typed.cases[1].keys, intended("zhongguo"));
        assert_eq!(
            typed.notes(XIAOHE, None),
            ["xiaohe: 1 of 3 cases do not encode and are not timed"]
        );
    }

    #[test]
    fn a_skipped_case_does_not_move_the_streams_of_the_others() {
        let noise = TouchNoise::new(&android_portrait(), 0.35, 0.1);
        let seed = 20261027;
        let encode = |touch_points| {
            encode_cases(
                vec![case("nihao"), case("xyz"), case("zhongguo")],
                XIAOHE,
                None,
                Some((&noise, TouchMode::Realistic)),
                touch_points,
                seed,
            )
            .unwrap()
        };
        let typed = encode(true);
        assert_eq!(inputs(&typed), ["nihao", "zhongguo"]);
        // 第三条用例仍取序号 2 的流，不因为第二条没打而改取序号 1 的。
        for (typed, position) in typed.cases.iter().zip([0u64, 2]) {
            let expected = noise
                .synthesize_touches(
                    TouchMode::Realistic,
                    &intended(&typed.input),
                    &mut touch_noise::case_rng(seed, &[position]),
                )
                .unwrap()
                .unwrap();
            let keys: Vec<u8> = expected.iter().map(|(key, _)| *key).collect();
            let points: Vec<Point> = expected.iter().map(|(_, point)| *point).collect();
            assert_eq!(typed.keys, keys, "{}", typed.input);
            assert_eq!(typed.points, points, "{}", typed.input);
        }
        // 不送落点时按到的键逐个相同，只是不记落点。
        let flat = encode(false);
        for (flat, typed) in flat.cases.iter().zip(&typed.cases) {
            assert_eq!(flat.keys, typed.keys, "{}", typed.input);
            assert!(flat.points.is_empty(), "{}", typed.input);
        }
        // realistic 不加条件，不因没有错键剔除。
        assert_eq!(typed.unmissable, 0);
        assert_eq!(
            typed.notes(XIAOHE, Some(TouchMode::Realistic)),
            ["xiaohe: 1 of 3 cases do not encode and are not timed"]
        );
    }

    #[test]
    fn impossible_noisy_cases_do_not_fall_back_to_clean_keys() {
        let cases = || vec![case("nihao"), case("zhongguo"), case("xyz")];
        let still = TouchNoise::new(&android_portrait(), 0.0, 0.0);
        let typed = encode_cases(
            cases(),
            XIAOHE,
            None,
            Some((&still, TouchMode::Noisy)),
            true,
            1,
        )
        .unwrap();
        // σ 为 0 时没有一键可能按错，这两条不退回想按的键，不计时、计数；`main` 见到一条不剩就报错。
        assert!(typed.cases.is_empty());
        assert_eq!(typed.unmissable, 2);
        assert_eq!(typed.unencodable, 1);
        assert_eq!(
            typed.notes(XIAOHE, Some(TouchMode::Noisy)),
            [
                "xiaohe: 1 of 3 cases do not encode and are not timed",
                "xiaohe: 2 of 2 encoded cases have no key that can be missed in noisy mode and are not timed",
            ]
        );
        // σ 大到每键都可能按错时，noisy 的每条都至少按错一键，一条不剔除。
        let shaky = TouchNoise::new(&android_portrait(), 0.35, 0.0);
        let typed = encode_cases(
            cases(),
            XIAOHE,
            None,
            Some((&shaky, TouchMode::Noisy)),
            true,
            1,
        )
        .unwrap();
        assert_eq!(typed.unmissable, 0);
        assert_eq!(inputs(&typed), ["nihao", "zhongguo"]);
        for case in &typed.cases {
            assert_ne!(case.keys, intended(&case.input), "{}", case.input);
        }
        assert_eq!(
            typed.notes(XIAOHE, Some(TouchMode::Noisy))[1],
            "xiaohe: 0 of 2 encoded cases have no key that can be missed in noisy mode and are not timed"
        );
    }

    #[test]
    fn grid_cases_that_do_not_encode_are_skipped_and_counted() {
        let grid = grid_keys::parse_grid("shuangpin-migala2").unwrap();
        let typed = encode_cases(
            vec![case("nihao"), case("xyz")],
            None,
            Some(grid),
            None,
            true,
            1,
        )
        .unwrap();
        assert_eq!(inputs(&typed), ["nihao"]);
        assert_eq!(typed.unencodable, 1);
        assert_eq!(
            typed.notes(None, None),
            ["--grid: 1 of 2 cases do not encode and are not timed"]
        );
        // 全拼九键每条都编得出来，不打条数。
        let typed = encode_cases(
            vec![case("nihao"), case("xyz")],
            None,
            Some(KeyGrid::NineKey),
            None,
            true,
            1,
        )
        .unwrap();
        assert_eq!(inputs(&typed), ["nihao", "xyz"]);
        assert!(typed.notes(None, None).is_empty());
    }

    #[test]
    fn quanpin_cases_are_all_timed_and_print_no_notes() {
        let typed =
            encode_cases(vec![case("nihao"), case("xyz")], None, None, None, true, 1).unwrap();
        assert_eq!(inputs(&typed), ["nihao", "xyz"]);
        assert_eq!(typed.cases[1].keys, b"xyz");
        assert!(typed.notes(None, None).is_empty());
    }
}
