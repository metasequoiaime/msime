//! Measures pinyin-to-text conversion quality against the locked dictionary.
//!
//! Why this exists: neither the word lattice nor any later reranking can be shown to help or hurt
//! without a number to compare against, and the repository had none.
//!
//! Why it reports per source rather than top-1 alone: for two or more complete syllables the Engine inserts lattice sentence rows among the dictionary rows, so whether position 1 is a dictionary word or a lattice sentence depends on the input, and whether a lattice change can move top-1 depends on that. `top1_source` and `gold_source` record it per run instead of assuming either way.
//!
//! Learning is off and each case gets a fresh session, so one case cannot bias the next.
//!
//! `--scheme shuangpin` 把同样的用例编成 26 键双拼的按键再打，`--touch-mode` 在参考键位几何上合成误触和落点，像触屏宿主那样先注册几何（`Runtime::set_key_geometry`）、每键带上按下点（`Runtime::set_key_touch`）再派发，量双拼触点纠错的召回和对正确输入的影响；`--no-touch-points` 只送按到的键、不送落点，就是引擎收不到触点时的平面表。`--learn-replay` 把 noisy 集按错的两键码对直接记进评测自己的日志（会话本身仍关着学习），再重跑一遍，量双拼纠错学习的作用。报告形状和跑法见 resources/eval/README.md「双拼与触屏误触」。

#[path = "../eval_resources/mod.rs"]
mod eval_resources;
mod grid_keys;
mod metrics;
mod touch_noise;

use metrics::{
    Bucket, ChangedFirst, FirstSeat, Observation, Report, TouchBucket, TouchObservation,
    TouchReport,
};
use msime_engine::host::{Command, EngineOptions, Session};
use msime_engine::touch::{KeyGeometry, KeyTouch, TouchTuning};
use msime_engine::{KeyGrid, SchemeType, ShuangpinProfileKind};
use msime_input_runtime::{Action, Reranker, Runtime, SentenceModel};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use touch_noise::{KeyLayout, Point, TouchMode, TouchNoise};

/// `--seed` 的默认值。
const DEFAULT_SEED: u64 = 20261027;

/// `--scheme shuangpin` 的参数。
struct ShuangpinEval {
    profiles: Vec<ShuangpinProfileKind>,
    /// 只留方案用得到的几何，按 `--key-geometry` 给出的顺序；报告的 `geometry` 照它列。
    layouts: Vec<KeyLayout>,
    modes: Vec<TouchMode>,
    sigmas: Vec<f64>,
    biases: Vec<f64>,
    seed: u64,
    /// 触点模式下把落点送给引擎；`--no-touch-points` 时只送按到的键，量的是平面表。
    touch_points: bool,
    /// `--touch-tuning` 覆盖的计价参数，没给时用引擎的默认值。
    tuning: Option<TouchTuning>,
    /// `--assert-top1-unchanged`：clean 集只要有一条用例的首选被改就失败。
    assert_top1_unchanged: bool,
    /// `--assert-first-seat-gate`：clean 集首选被改的比例超过首选规则的门槛（σ=0 时为 0，总体不超过 0.1%，每个方案不超过 0.2%）就失败。纠错可以坐首选以后用它代替 `--assert-top1-unchanged`。
    assert_first_seat_gate: bool,
    /// `--jobs`：同时跑几个方案。
    jobs: usize,
    /// `--learn-replay <n>`：每组（σ、偏置）跑完之后，把这组 noisy 集里按错的每一对两键码在日志里记成接受过 n 次，再把各个触点模式重跑一遍，量双拼纠错学习的效果。
    learn_replay: Option<i64>,
}

/// 一次按键：按到的键，和送给引擎的落点（没有时按 `msime_client_character` 派发）。
type TypedKey = (u8, Option<Point>);

struct Case {
    id: String,
    input: String,
    gold: String,
    syllables: usize,
    tags: Vec<String>,
    /// Text the user committed just before this case, seeded into the session before typing.
    ///
    /// Empty for the hand-written sets, which predate the column. Ranking 会议 above 回忆 is a
    /// judgement about what came before, so a set that carries none cannot ask for one; harvested
    /// cases bring the preceding sentence from the corpus they came from.
    context: String,
    /// `--cut-last-syllable` 截短前的输入；没截时为 `None`。`input` 换成了截短后真正打的那串，九键双拼要从截短前的全拼切音节。
    full_input: Option<String>,
}

/// `--cut-last-syllable` 的两种截法：用户打到最后一个音节就停，等候选里出现要的词（吃饭 打 `chif`，进一步 打 `jinyib`）。
#[derive(Clone, Copy)]
enum Cut {
    /// 末音节只打声母：zh、ch、sh 算一个声母，没有辅音声母的音节（an、e）留第一个字母。
    Initial,
    /// 末音节只打第一个字母。只收和 `Initial` 截出来不一样的用例，也就是末音节以 zh、ch、sh 开头的（吃饭 打 `chic`），其余用例两种截法相同，不重复计入。
    Letter,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut resources: Option<PathBuf> = None;
    let mut resource_lock: Option<PathBuf> = None;
    let mut sets: Vec<PathBuf> = Vec::new();
    let mut limit: Option<usize> = None;
    let mut report_path: Option<PathBuf> = None;
    let mut baseline: Option<PathBuf> = None;
    let mut update = false;
    let mut dump: Option<PathBuf> = None;
    let mut grid: Option<KeyGrid> = None;
    let mut segmentation_filter: Option<KeyGrid> = None;
    let mut keyboard_neural = false;
    let mut cut: Option<Cut> = None;
    let mut first_page_rows: Option<usize> = None;
    let mut scheme = SchemeType::Quanpin;
    let mut autocorrect = false;
    let mut max_syllables: Option<usize> = None;
    let mut profiles: Vec<ShuangpinProfileKind> = Vec::new();
    let mut geometry: Vec<PathBuf> = Vec::new();
    let mut modes: Vec<TouchMode> = Vec::new();
    let mut sigmas: Vec<f64> = Vec::new();
    let mut biases: Vec<f64> = Vec::new();
    let mut seed: Option<u64> = None;
    let mut touch_points = true;
    let mut tuning: Option<TouchTuning> = None;
    let mut assert_top1_unchanged = false;
    let mut assert_first_seat_gate = false;
    let mut jobs = 1usize;
    let mut learn_replay: Option<i64> = None;
    let mut fuzzy_all = false;

    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut index = 0;
    while index < args.len() {
        let take = |i: &mut usize| -> Result<String, String> {
            *i += 1;
            args.get(*i)
                .cloned()
                .ok_or_else(|| format!("{} needs a value", args[*i - 1]))
        };
        match args[index].as_str() {
            "--resources" => resources = Some(PathBuf::from(take(&mut index)?)),
            "--resource-lock" => resource_lock = Some(PathBuf::from(take(&mut index)?)),
            "--set" => sets.push(PathBuf::from(take(&mut index)?)),
            "--limit" => limit = Some(take(&mut index)?.parse()?),
            "--report" => report_path = Some(PathBuf::from(take(&mut index)?)),
            "--baseline" => baseline = Some(PathBuf::from(take(&mut index)?)),
            "--update-baseline" => update = true,
            "--dump" => dump = Some(PathBuf::from(take(&mut index)?)),
            "--grid" => grid = Some(grid_keys::parse_grid(&take(&mut index)?)?),
            "--keyboard-neural" => keyboard_neural = true,
            "--first-page-rows" => first_page_rows = Some(take(&mut index)?.parse()?),
            "--cut-last-syllable" => {
                cut = Some(match take(&mut index)?.as_str() {
                    "initial" => Cut::Initial,
                    "letter" => Cut::Letter,
                    other => {
                        return Err(format!(
                            "unknown --cut-last-syllable: {other} (expected initial or letter)"
                        )
                        .into())
                    }
                })
            }
            // `--grid nine` 的旧写法，verify-local.sh 和笔记里都用过。
            "--nine-key" => grid = Some(KeyGrid::NineKey),
            // 只留全拼切成音节只有一种切法的用例，与九键双拼量的是同一批。
            "--unique-segmentation" => {
                segmentation_filter = Some(grid_keys::parse_grid("shuangpin-migala3")?)
            }
            "--scheme" => {
                scheme = match take(&mut index)?.as_str() {
                    "quanpin" => SchemeType::Quanpin,
                    "shuangpin" => SchemeType::Shuangpin,
                    other => {
                        return Err(format!(
                            "unknown scheme: {other} (expected quanpin or shuangpin)"
                        )
                        .into())
                    }
                }
            }
            // 打开「拼音纠错」的邻键和对调两项，双拼的纠错整句（平面表）只在它们开着时出现。
            "--autocorrect" => autocorrect = true,
            // 打开全部 11 条模糊音规则（#10457 报告者的设置），给模糊音路径的改动比对候选用；默认关，与产品出厂一致。只支持全拼。
            "--fuzzy-all" => fuzzy_all = true,
            "--max-syllables" => max_syllables = Some(take(&mut index)?.parse()?),
            "--shuangpin-profile" => {
                for name in take(&mut index)?.split(',') {
                    profiles.push(touch_noise::parse_profile(name)?);
                }
            }
            "--key-geometry" => geometry.push(PathBuf::from(take(&mut index)?)),
            "--touch-mode" => {
                for name in take(&mut index)?.split(',') {
                    modes.push(TouchMode::parse(name)?);
                }
            }
            "--touch-sigma" => {
                sigmas.extend(parse_list(&take(&mut index)?, touch_noise::parse_sigma)?)
            }
            "--touch-bias-y" => {
                biases.extend(parse_list(&take(&mut index)?, touch_noise::parse_bias_y)?)
            }
            "--seed" => seed = Some(take(&mut index)?.parse()?),
            "--no-touch-points" => touch_points = false,
            "--touch-tuning" => tuning = Some(parse_tuning(&take(&mut index)?)?),
            "--assert-top1-unchanged" => assert_top1_unchanged = true,
            "--assert-first-seat-gate" => assert_first_seat_gate = true,
            "--jobs" => jobs = take(&mut index)?.parse()?,
            "--learn-replay" => learn_replay = Some(take(&mut index)?.parse()?),
            other => return Err(format!("unknown option: {other}").into()),
        }
        index += 1;
    }
    let resources =
        resources.ok_or("usage: convert_eval --resources <verified-dir> --set <file.tsv> [--resource-lock <file.json>] [...]")?;
    if sets.is_empty() {
        return Err("at least one --set is required".into());
    }

    let shuangpin = if scheme == SchemeType::Shuangpin {
        if fuzzy_all {
            return Err("--fuzzy-all is only supported for --scheme quanpin".into());
        }
        if cut.is_some() {
            return Err("--cut-last-syllable is only supported for --scheme quanpin".into());
        }
        if grid.is_some() || segmentation_filter.is_some() {
            return Err(
                "--scheme shuangpin types the 26-key layout; it cannot be combined with --grid"
                    .into(),
            );
        }
        if profiles.is_empty() {
            return Err("--scheme shuangpin needs --shuangpin-profile".into());
        }
        if modes.is_empty() {
            modes.push(TouchMode::None);
        }
        let touches = modes.iter().any(|mode| *mode != TouchMode::None);
        if touches && (geometry.is_empty() || sigmas.is_empty()) {
            return Err("touch modes other than none need --key-geometry and --touch-sigma".into());
        }
        if biases.is_empty() {
            biases.push(0.0);
        }
        if let Some(times) = learn_replay {
            if !(1..=1000).contains(&times) || !touches || !touch_points {
                return Err("--learn-replay needs a count from 1 to 1000, a touch mode other than none and touch points".into());
            }
        }
        let layouts = geometry
            .iter()
            .map(|path| KeyLayout::load(path))
            .collect::<Result<Vec<_>, _>>()?;
        // 同一种给了几份的在这里报错；只留方案挑中的，报告里不列没用上的几何。
        let layouts = touch_noise::select_layouts(layouts, &profiles, touches)?;
        Some(ShuangpinEval {
            profiles,
            layouts,
            modes,
            sigmas,
            biases,
            seed: seed.unwrap_or(DEFAULT_SEED),
            touch_points,
            tuning,
            assert_top1_unchanged,
            assert_first_seat_gate,
            jobs,
            learn_replay,
        })
    } else {
        if !profiles.is_empty()
            || !geometry.is_empty()
            || !modes.is_empty()
            || !sigmas.is_empty()
            || !biases.is_empty()
            || seed.is_some()
            || !touch_points
            || tuning.is_some()
            || assert_top1_unchanged
            || assert_first_seat_gate
            || jobs != 1
            || learn_replay.is_some()
        {
            return Err("--shuangpin-profile, --key-geometry and the --touch-* options need --scheme shuangpin".into());
        }
        None
    };

    let verified = eval_resources::verify(&resources, resource_lock.as_deref())?;

    let mut cases = Vec::new();
    for set in &sets {
        cases.extend(load(set)?);
    }
    // 先按音节数筛再抽等距子集，子集仍保持筛后集合的分布。
    if let Some(max) = max_syllables {
        cases.retain(|case| case.syllables <= max);
    }
    if let Some(limit) = limit {
        // Deterministic subset: every nth case, so the sample keeps the set's length distribution
        // instead of taking a prefix that would be all short words.
        let stride = cases.len().div_ceil(limit.max(1));
        cases = cases.into_iter().step_by(stride.max(1)).collect();
    }
    // 九键双拼要先把全拼切成音节，切法不唯一（xian 和 xi'an）的用例打不出确定的数字，不计入。九键也可以用 `--unique-segmentation` 只量这一部分，好和九键双拼比。
    if let Some(filter) =
        segmentation_filter.or(grid.filter(|grid| matches!(grid, KeyGrid::NineKeyShuangpin(_))))
    {
        let before = cases.len();
        cases.retain(|case| {
            grid_keys::grid_keys(filter, &case.input, Some(case.syllables)).is_some()
        });
        eprintln!(
            "{} of {before} cases have one syllable segmentation",
            cases.len()
        );
    }
    // 在切分过滤之后截：九键双拼的过滤要看打全的读法能不能唯一切开。
    if let Some(cut) = cut {
        let before = cases.len();
        cases = cases
            .into_iter()
            .filter_map(|mut case| {
                let typed = cut_last_syllable(&case.input, case.syllables, cut)?;
                case.full_input = Some(std::mem::replace(&mut case.input, typed));
                Some(case)
            })
            .collect();
        eprintln!(
            "{} of {before} cases cut the last syllable short",
            cases.len()
        );
    }
    eprintln!("{} cases from {} set(s)", cases.len(), sets.len());

    let state = tempfile::tempdir()?;
    let mut clean_top1_changed = 0;
    let mut clean_first_seat = Vec::new();
    let json = match &shuangpin {
        Some(eval) => run_shuangpin(
            &resources,
            state.path(),
            &cases,
            dump.as_deref(),
            eval,
            autocorrect,
            keyboard_neural,
            first_page_rows,
            &verified.source_commit,
            &mut clean_top1_changed,
            &mut clean_first_seat,
        )?,
        None => {
            let report = run(
                &resources,
                state.path(),
                &cases,
                dump.as_deref(),
                grid,
                keyboard_neural,
                autocorrect,
                fuzzy_all,
                first_page_rows,
                &verified.source_commit,
            )?;
            render(&report, &cases)
        }
    };

    if let Some(path) = &report_path {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, format!("{json}\n"))?;
    }
    println!("{json}");

    if shuangpin
        .as_ref()
        .is_some_and(|eval| eval.assert_top1_unchanged)
        && clean_top1_changed > 0
    {
        return Err(format!(
            "touches changed the first candidate of {clean_top1_changed} clean case(s)"
        )
        .into());
    }
    if shuangpin
        .as_ref()
        .is_some_and(|eval| eval.assert_first_seat_gate)
    {
        first_seat_gate(&clean_first_seat)?;
    }

    if let Some(path) = baseline {
        if update {
            // Trailing newline so the committed baselines are ordinary text files. The comparison
            // below trims, so a baseline written by an older build still matches.
            std::fs::write(&path, format!("{json}\n"))?;
            eprintln!("baseline written: {}", path.display());
        } else if path.exists() {
            let previous = std::fs::read_to_string(&path)?;
            if previous.trim() != json.trim() {
                eprintln!(
                    "eval differs from {}; run with --update-baseline to accept",
                    path.display()
                );
                return Err("eval baseline mismatch".into());
            }
            eprintln!("eval: at baseline");
        }
    }
    Ok(())
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
            gold: fields[2].to_string(),
            syllables: fields[3].parse()?,
            context: fields.get(5).unwrap_or(&"").to_string(),
            full_input: None,
            tags: fields
                .get(4)
                .map(|t| {
                    t.split(',')
                        .filter(|t| !t.is_empty())
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default(),
        });
    }
    Ok(cases)
}

/// clean 集里一组配置的首选变化：方案、σ、用例数、首选被改的条数。
type CleanFirstSeat = (&'static str, f64, usize, usize);

/// 首选规则的门槛（方案第八节 PR3）：σ=0 时 clean 集的首选一条都不能变（落点在键心，纠错不能抢首选），所有 clean 组合起来不超过 0.1%，每个方案不超过 0.2%。
fn first_seat_gate(runs: &[CleanFirstSeat]) -> Result<(), String> {
    if runs.is_empty() {
        return Err("--assert-first-seat-gate needs clean runs".into());
    }
    let mut failures = Vec::new();
    for (profile, sigma, cases, changed) in runs {
        if *sigma == 0.0 && *changed > 0 {
            failures.push(format!("{profile} sigma=0: {changed} of {cases}"));
        }
    }
    let total = |runs: &mut dyn Iterator<Item = &CleanFirstSeat>| {
        runs.fold((0, 0), |(cases, changed), run| {
            (cases + run.2, changed + run.3)
        })
    };
    let (cases, changed) = total(&mut runs.iter());
    if changed as f64 > cases as f64 * 0.001 {
        failures.push(format!("overall: {changed} of {cases}"));
    }
    let mut profiles: Vec<&str> = runs.iter().map(|run| run.0).collect();
    profiles.dedup();
    for profile in profiles {
        let (cases, changed) = total(&mut runs.iter().filter(|run| run.0 == profile));
        if changed as f64 > cases as f64 * 0.002 {
            failures.push(format!("{profile}: {changed} of {cases}"));
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "touches changed the first candidate of too many clean cases: {}",
            failures.join("; ")
        ))
    }
}

/// 逗号分隔的一列数，每一项按 `parse` 解析和校验。
fn parse_list(text: &str, parse: fn(&str) -> Result<f64, String>) -> Result<Vec<f64>, String> {
    text.split(',').map(parse).collect()
}

/// `--touch-tuning base=12,scale=8,...`：只覆盖给出的几项，其余用引擎的默认值。
fn parse_tuning(text: &str) -> Result<TouchTuning, String> {
    let mut tuning = TouchTuning::default();
    for item in text.split(',') {
        let (name, value) = item
            .split_once('=')
            .ok_or_else(|| format!("--touch-tuning: {item} is not name=value"))?;
        let value: f64 = value
            .parse()
            .ok()
            .filter(|value: &f64| value.is_finite())
            .ok_or_else(|| format!("--touch-tuning: {item} is not a finite number"))?;
        let slot = match name {
            "base" => &mut tuning.base,
            "scale" => &mut tuning.scale,
            "sigma_x" => &mut tuning.sigma_x,
            "sigma_y" => &mut tuning.sigma_y,
            "bias_y" => &mut tuning.bias_y,
            "min" => &mut tuning.min_penalty,
            "max" => &mut tuning.max_penalty,
            "cut" => &mut tuning.cut,
            "show" => &mut tuning.show_margin,
            "show_pen" => &mut tuning.show_penalty,
            "lead" => &mut tuning.lead_margin,
            "lead_pen" => &mut tuning.lead_penalty,
            other => {
                return Err(format!(
                    "--touch-tuning: unknown {other} (expected base, scale, sigma_x, sigma_y, bias_y, min, max, cut, show, show_pen, lead or lead_pen)"
                ))
            }
        };
        *slot = value;
    }
    if tuning.sigma_x <= 0.0 || tuning.sigma_y <= 0.0 {
        return Err("--touch-tuning: sigma_x and sigma_y must be positive".into());
    }
    if tuning.min_penalty > tuning.max_penalty {
        return Err("--touch-tuning: min must not exceed max".into());
    }
    Ok(tuning)
}

/// 资源里有的两个重排模型，整次运行只读一次，各个 Runtime 共用：模型只是权重，每个 `Reranker` 自己持有会话的前缀缓存，共用模型不会让一个方案的运行影响另一个。
struct Models {
    /// 停顿后重排用的 settled 模型（`sentence-model-desktop.safetensors`）。
    settled: Option<Arc<SentenceModel>>,
    /// 每次按键重排用的模型（`sentence-model.safetensors`）。
    keystroke: Option<Arc<SentenceModel>>,
}

impl Models {
    // Measuring the shipped path means measuring it with whatever the resource set contains. The model is optional there, so its absence has to leave these numbers exactly as they were.
    fn load(resources: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        // The settled model, when the resource set ships one. Attaching it is what lets the eval measure the shipped pair rather than either model alone: the fast one ranks every keystroke and this one ranks once at the end, which is exactly what a host does with its settle timer.
        let settled_path = resources.join("sentence-model-desktop.safetensors");
        let settled = match std::fs::read(&settled_path) {
            Ok(bytes) => {
                let model = SentenceModel::load(&bytes)?;
                eprintln!("settling with {}", settled_path.display());
                Some(Arc::new(model))
            }
            Err(_) => {
                eprintln!("no settled model at {}", settled_path.display());
                None
            }
        };
        let model_path = resources.join("sentence-model.safetensors");
        let keystroke = match std::fs::read(&model_path) {
            Ok(bytes) => {
                let model = SentenceModel::load(&bytes)?;
                eprintln!("reranking with {}", model_path.display());
                Some(Arc::new(model))
            }
            Err(_) => {
                eprintln!(
                    "no model at {}, measuring the engine alone",
                    model_path.display()
                );
                None
            }
        };
        Ok(Self { settled, keystroke })
    }
}

/// 建一个和宿主同样配置的 Runtime：关学习、关调频、关混输和本地模式，每页候选数同时交给引擎和 runtime，挂上资源里有的重排模型。`configure` 在建会话前改方案相关的选项，`tuning` 覆盖触点计价参数。
#[allow(clippy::too_many_arguments)]
fn open_runtime(
    resources: &Path,
    state: &Path,
    models: &Models,
    keyboard_neural: bool,
    autocorrect: bool,
    first_page_rows: Option<usize>,
    source_commit: &str,
    tuning: Option<TouchTuning>,
    configure: impl FnOnce(&mut EngineOptions),
) -> Result<Runtime<Session>, Box<dyn std::error::Error>> {
    // 入口已核对资源清单；这里准备可写词库代次，不能把包目录直接当用户词库。
    let mut options = msime_engine::host::prepare_options(
        resources.to_str().ok_or("non-UTF-8 resource path")?,
        state.join("user").to_str().ok_or("non-UTF-8 state path")?,
        state.join("cache").to_str().ok_or("non-UTF-8 cache path")?,
        source_commit,
    )?;
    options.scheme = 0;
    // Off so a case cannot promote its own answer for the cases that follow.
    options.learning = false;
    options.frequency_mode = "disabled".into();
    options.autocorrect_transposition = autocorrect;
    options.autocorrect_neighbor = autocorrect;
    options.fuzzy_pinyin_rules = 0;
    options.helpcode = false;
    options.show_helpcode = false;
    options.mixed_english = false;
    options.mixed_emoji = false;
    options.mixed_kaomoji = false;
    options.mixed_symbols = false;
    options.local_unicode = false;
    options.local_date_time = false;
    options.local_quick_phrase = false;
    options.local_emoji = false;
    options.local_kaomoji = false;
    options.local_super_jianpin = false;
    options.local_temporary_english = false;
    options.local_temporary_japanese = false;
    // 其余本地模式也关掉，和上面一样量的是转换本身。行内日期时间和「📅日期」菜单是产品默认开的，不关时 `riqi`、`shijian`、`jintian` 这类输入第 2 格起是运行当天的日期时间，词级集的结果随运行日期变化。
    options.local_date_time_inline = false;
    options.local_date_time_menu = false;
    options.local_expression = false;
    options.local_command = false;
    options.local_mention = false;
    options.local_chaizi = false;
    options.local_symbol = false;
    options.local_shift_letter_entry = false;
    // Measure the shipped path: the host asks for every reading and crops the list itself.
    options.sentence_alternatives = true;
    options.sentence_association.neural_keyboard = keyboard_neural;
    // 每页候选数和宿主一样同时是引擎的首屏门槛和 runtime 的分页（`Runtime::new` 会把它交给引擎，这里先写进选项，建会话时就是同一个数）。不给 `--first-page-rows` 时取共享偏好的出厂值，桌面三端、Android 和鸿蒙都用它；iOS 和网页引擎每页 9 个，用 `--first-page-rows 9` 量。
    let page_size = first_page_rows.unwrap_or(usize::from(
        msime_client_core::preferences::Preferences::default().candidate_page_size,
    ));
    let page_size = u8::try_from(page_size)
        .ok()
        .filter(|size| (1..=msime_input_runtime::MAX_CANDIDATE_PAGE_SIZE).contains(size))
        .ok_or_else(|| {
            format!(
                "--first-page-rows must be 1 to {}",
                msime_input_runtime::MAX_CANDIDATE_PAGE_SIZE
            )
        })?;
    options.first_page_rows = usize::from(page_size);
    configure(&mut options);

    let mut engine = Session::new(&options)?;
    if let Some(tuning) = tuning {
        engine.set_touch_tuning(tuning);
    }
    let mut runtime = Runtime::new(engine, page_size)?;
    // 每个 Runtime 各建自己的 `Reranker`（前缀缓存跟着会话走），模型本身共用。
    if let Some(model) = &models.settled {
        runtime.set_settled_reranker(Some(Reranker::new(Arc::clone(model))));
    }
    if let Some(model) = &models.keystroke {
        runtime.set_reranker(Some(Reranker::new(Arc::clone(model))));
    }
    // dispatch() drops every action while unfocused, which yields an empty candidate list rather
    // than an error.
    runtime.focus(true)?;
    Ok(runtime)
}

/// 整张候选表（文字和来源），以及首选是不是按首选规则坐上去的纠错行。
type TypedCandidates = (Vec<(String, u8)>, bool);

/// 在清空的组字和上文里打一条用例，返回停顿重排之后的整张候选表（文字和来源），以及首选是不是按首选规则坐上去的纠错行（视图的 `confident_correction`）。带触点的键像 `msime_client_touch_character` 那样，派发前交上触点、派发后清掉。
fn type_case(
    runtime: &mut Runtime<Session>,
    context: &str,
    keys: impl IntoIterator<Item = (Action, Option<KeyTouch>)>,
) -> Result<TypedCandidates, Box<dyn std::error::Error>> {
    runtime.dispatch(Action::Command(Command::Cancel))?;
    // Cancel keeps the committed context and seeding appends to it, so without this every case would be ranked against the tail of the contexts of all the cases before it.
    runtime.clear_context();
    runtime.seed_context(context);
    for (key, touch) in keys {
        if touch.is_some() {
            runtime.set_key_touch(touch);
        }
        runtime.dispatch(key)?;
        if touch.is_some() {
            runtime.set_key_touch(None);
        }
    }
    // The user has stopped typing by the time a case is read, which is when a host fires its settle timer.
    runtime.rerank_settled();
    let snapshot = runtime.all_candidates();
    let candidates = snapshot
        .candidates
        .iter()
        .map(|c| (c.text.clone(), c.source))
        .collect();
    let confident = snapshot
        .candidates
        .first()
        .is_some_and(|candidate| candidate.confident_correction);
    runtime.dispatch(Action::Command(Command::Cancel))?;
    Ok((candidates, confident))
}

#[allow(clippy::too_many_arguments)]
fn run(
    resources: &Path,
    state: &Path,
    cases: &[Case],
    dump: Option<&Path>,
    grid: Option<KeyGrid>,
    keyboard_neural: bool,
    autocorrect: bool,
    fuzzy_all: bool,
    first_page_rows: Option<usize>,
    source_commit: &str,
) -> Result<Report, Box<dyn std::error::Error>> {
    // Optional JSONL of the real candidate lists, for offline experiments that must not be able to change what the harness measures.
    let mut dumped = String::new();
    let models = Models::load(resources)?;
    let mut runtime = open_runtime(
        resources,
        state,
        &models,
        keyboard_neural,
        autocorrect,
        first_page_rows,
        source_commit,
        None,
        |options| {
            if fuzzy_all {
                options.fuzzy_pinyin_rules = msime_engine::fuzzy_rule::ALL;
            }
        },
    )?;
    // `--grid` 在触屏网格上打每条用例：字母换成它所在键的组码（九键是旁边印的数字，14 键是这一组的首字母），同一个金标准要从组码串里打出来，而 西安 和 一按 在九键上是同一串输入。
    if grid.is_some() {
        runtime.set_key_grid(grid)?;
    }
    let mut report = Report::default();

    for case in cases {
        // 组码只取字母，少数用例带的 `'` 不输入；九键双拼切不出唯一一种音节切法的用例在 `main` 里已经去掉了。
        let input: Vec<u8> = match grid {
            Some(grid) => typed_keys(grid, case).unwrap_or_default().into_bytes(),
            None => case.input.bytes().collect(),
        };
        let (candidates, _) = type_case(
            &mut runtime,
            &case.context,
            input.into_iter().map(|byte| (key_action(grid, byte), None)),
        )?;
        if dump.is_some() {
            let rows: Vec<serde_json::Value> = candidates
                .iter()
                .map(|(text, source)| serde_json::json!({"text": text, "source": source}))
                .collect();
            dumped.push_str(
                &serde_json::json!({
                    "id": case.id, "input": case.input, "gold": case.gold,
                    "context": case.context, "candidates": rows,
                })
                .to_string(),
            );
            dumped.push('\n');
        }
        report.observe(Observation {
            id: &case.id,
            input: &case.input,
            gold: &case.gold,
            tags: &case.tags,
            syllables: case.syllables,
            candidates: &candidates,
            keep_failure: report.failures.len() < 40,
        });
    }
    if let Some(path) = dump {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, dumped)?;
    }
    Ok(report)
}

/// `--scheme shuangpin`：每个方案建一个双拼 Runtime，把用例编成这个方案的按键；先按想按的键原样打一遍作对照，再按每组（触点模式、σ、偏置）合成实际按到的键和落点去打。返回整份报告的 JSON；clean 集里首选被改的用例数累加进 `clean_top1_changed`。`jobs` 大于 1 时几个方案各开一个线程同时跑，各方案的结果互不相关，按方案的次序拼起来，输出与一个一个跑逐字节相同。
#[allow(clippy::too_many_arguments)]
fn run_shuangpin(
    resources: &Path,
    state: &Path,
    cases: &[Case],
    dump: Option<&Path>,
    eval: &ShuangpinEval,
    autocorrect: bool,
    keyboard_neural: bool,
    first_page_rows: Option<usize>,
    source_commit: &str,
    clean_top1_changed: &mut usize,
    clean_first_seat: &mut Vec<CleanFirstSeat>,
) -> Result<String, Box<dyn std::error::Error>> {
    let models = Models::load(resources)?;
    // 落点分布只取决于几何、σ 和偏置，各方案、各模式共用同一份几何时不必重算；在开线程前一次算好，各方案只读。只有 noisy、realistic 和要送落点的 clean 要抽落点。
    let needs_noise = eval.modes.iter().any(|mode| match mode {
        TouchMode::None => false,
        TouchMode::Clean => eval.touch_points,
        TouchMode::Noisy | TouchMode::Realistic => true,
    });
    let mut noises = Noises::new();
    if needs_noise {
        for (layout, geometry) in eval.layouts.iter().enumerate() {
            for &sigma in &eval.sigmas {
                for &bias in &eval.biases {
                    noises.insert(
                        (layout, sigma.to_bits(), bias.to_bits()),
                        TouchNoise::new(geometry, sigma, bias),
                    );
                }
            }
        }
    }
    let run = |profile: ShuangpinProfileKind| {
        run_profile(
            resources,
            // 每个方案各用一份可写目录，同时跑的会话不争同一个用户词库。
            &state.join(profile.name()),
            &models,
            &noises,
            cases,
            dump.is_some(),
            eval,
            autocorrect,
            keyboard_neural,
            first_page_rows,
            source_commit,
            profile,
        )
        .map_err(|error| error.to_string())
    };
    let mut results = Vec::with_capacity(eval.profiles.len());
    for batch in eval.profiles.chunks(eval.jobs.max(1)) {
        if batch.len() == 1 {
            results.push(run(batch[0])?);
            continue;
        }
        let finished: Vec<Result<ProfileRun, String>> = std::thread::scope(|scope| {
            let handles: Vec<_> = batch
                .iter()
                .map(|&profile| {
                    let run = &run;
                    scope.spawn(move || run(profile))
                })
                .collect();
            handles
                .into_iter()
                .map(|handle| {
                    handle
                        .join()
                        .unwrap_or_else(|_| Err("a profile thread panicked".to_owned()))
                })
                .collect()
        });
        for result in finished {
            results.push(result?);
        }
    }
    let mut encoding = Vec::with_capacity(results.len());
    let mut runs = Vec::new();
    let mut dumped = String::new();
    for result in results {
        encoding.push(result.encoding);
        runs.extend(result.runs);
        dumped.push_str(&result.dump.unwrap_or_default());
        *clean_top1_changed += result.clean_top1_changed;
        clean_first_seat.extend(result.clean_first_seat);
    }
    if let Some(path) = dump {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, dumped)?;
    }
    Ok(render_shuangpin(eval, autocorrect, &encoding, &runs))
}

/// 一个方案的结果：编码统计、每组配置渲染好的报告行、`--dump` 的行、clean 集首选被改的条数，以及每组 clean 配置的首选变化（`--assert-first-seat-gate` 用）。
#[derive(Default)]
struct ProfileRun {
    encoding: (&'static str, usize, usize),
    runs: Vec<String>,
    dump: Option<String>,
    clean_top1_changed: usize,
    clean_first_seat: Vec<CleanFirstSeat>,
}

/// `run_shuangpin` 里一个方案的那一份。
#[allow(clippy::too_many_arguments)]
fn run_profile(
    resources: &Path,
    state: &Path,
    models: &Models,
    noises: &Noises,
    cases: &[Case],
    dump: bool,
    eval: &ShuangpinEval,
    autocorrect: bool,
    keyboard_neural: bool,
    first_page_rows: Option<usize>,
    source_commit: &str,
    profile: ShuangpinProfileKind,
) -> Result<ProfileRun, Box<dyn std::error::Error>> {
    let mut result = ProfileRun {
        dump: dump.then(String::new),
        ..ProfileRun::default()
    };
    let touches = eval.modes.iter().any(|mode| *mode != TouchMode::None);
    let mut runtime = open_runtime(
        resources,
        state,
        models,
        keyboard_neural,
        autocorrect,
        first_page_rows,
        source_commit,
        eval.tuning,
        |options| {
            options.scheme = SchemeType::Shuangpin as u8;
            options.shuangpin_profile = profile as u8;
        },
    )?;
    // 像触屏宿主一样先注册这个方案用的那份几何；none 模式不送落点，注册与否结果都一样。
    let geometry_id = if touches && eval.touch_points {
        let geometry = KeyGeometry::from_json(
            &eval.layouts[touch_noise::layout_for(&eval.layouts, profile)?].json,
        )?
        .ok_or("a reference geometry needs a non-zero id")?;
        let id = geometry.id();
        runtime.set_key_geometry(Some(geometry));
        id
    } else {
        0
    };
    // 切法有歧义或有音节在这个方案里编不出来的用例不打，编出来的比例写进报告。
    let intended: Vec<Option<Vec<u8>>> = cases
        .iter()
        .map(|case| {
            grid_keys::shuangpin_keys(profile.name(), &case.input, Some(case.syllables))
                .map(String::into_bytes)
        })
        .collect();
    let encoded = intended.iter().flatten().count();
    eprintln!(
        "{}: {encoded} of {} cases encode",
        profile.name(),
        cases.len()
    );
    result.encoding = (profile.name(), cases.len(), encoded);
    // 一条用例的候选表只取决于它的上文、送进去的按键和引擎收到的（量化后的）触点，所以同一串按键带同样的触点在各组配置里只打一次。
    let mut ranks: Ranks = HashMap::new();
    let mut reference = vec![CandidateResult::default(); cases.len()];
    for (index, keys) in intended.iter().enumerate() {
        if let Some(keys) = keys {
            let typed: Vec<TypedKey> = keys.iter().map(|&key| (key, None)).collect();
            reference[index] = gold_rank(
                &mut runtime,
                &mut ranks,
                (index, &cases[index]),
                &typed,
                geometry_id,
                profile,
                result.dump.as_mut(),
            )?;
        }
    }
    for &mode in &eval.modes {
        for setting in touch_settings(eval, mode) {
            let (report, skipped) = measure_setting(
                &mut runtime,
                &mut ranks,
                noises,
                (cases, &intended, &reference),
                eval,
                profile,
                mode,
                setting,
                geometry_id,
                Some(&mut result),
            )?;
            let layout = match setting {
                Some(_) => Some(touch_noise::layout_for(&eval.layouts, profile)?),
                None => None,
            };
            let (sigma, bias) = setting.unzip();
            if let (TouchMode::Clean, Some(sigma)) = (mode, sigma) {
                result.clean_first_seat.push((
                    profile.name(),
                    sigma,
                    report.overall.cases,
                    report.first_seat.first_changed,
                ));
            }
            result.runs.push(render_touch_run(
                profile,
                layout.map(|layout| eval.layouts[layout].name.as_str()),
                mode,
                sigma,
                bias,
                skipped,
                &report,
                None,
            ));
        }
    }
    if let Some(times) = eval.learn_replay {
        // 学习回放：每组（σ、偏置）单独记、单独量，量完清掉，组与组互不影响。
        let journal = state.join("user").join(msime_engine::assets::USER_JOURNAL);
        let layout = touch_noise::layout_for(&eval.layouts, profile)?;
        for &sigma in &eval.sigmas {
            for &bias in &eval.biases {
                let noise = &noises[&(layout, sigma.to_bits(), bias.to_bits())];
                let pairs = noisy_pairs(noise, &intended, eval, profile, sigma, bias)?;
                write_learned_pairs(&journal, profile, &pairs, times)?;
                // 学过之后同一串按键的名次变了，记忆表重新来。
                let mut learned_ranks: Ranks = HashMap::new();
                for &mode in eval.modes.iter().filter(|mode| **mode != TouchMode::None) {
                    let (report, skipped) = measure_setting(
                        &mut runtime,
                        &mut learned_ranks,
                        noises,
                        (cases, &intended, &reference),
                        eval,
                        profile,
                        mode,
                        Some((sigma, bias)),
                        geometry_id,
                        None,
                    )?;
                    result.runs.push(render_touch_run(
                        profile,
                        Some(eval.layouts[layout].name.as_str()),
                        mode,
                        Some(sigma),
                        Some(bias),
                        skipped,
                        &report,
                        Some((times, pairs.len())),
                    ));
                }
                write_learned_pairs(&journal, profile, &[], 0)?;
            }
        }
    }
    Ok(result)
}

/// 一个触点模式要跑的各组（σ、偏置）；none 模式只有一组、不带落点。
fn touch_settings(eval: &ShuangpinEval, mode: TouchMode) -> Vec<Option<(f64, f64)>> {
    if mode == TouchMode::None {
        vec![None]
    } else {
        eval.sigmas
            .iter()
            .flat_map(|&sigma| eval.biases.iter().map(move |&bias| Some((sigma, bias))))
            .collect()
    }
}

/// 第 `index` 条用例在这组配置下实际按到的键和落点；noisy 集里一键都没按错（σ 为 0）时为 `None`。随机流只由种子、方案、模式、σ、偏置和用例序号决定，学习回放取到的和正式那一遍逐键相同。
#[allow(clippy::too_many_arguments)]
fn touch_typed_keys(
    noise: Option<&TouchNoise>,
    eval: &ShuangpinEval,
    profile: ShuangpinProfileKind,
    mode: TouchMode,
    (sigma, bias): (Option<f64>, Option<f64>),
    index: usize,
    intended: &[u8],
) -> Result<Option<Vec<TypedKey>>, String> {
    let Some(noise) = noise else {
        return Ok(Some(intended.iter().map(|&key| (key, None)).collect()));
    };
    let mut rng = touch_noise::case_rng(
        eval.seed,
        &[
            touch_noise::profile_label(profile),
            mode.label(),
            sigma.unwrap_or_default().to_bits(),
            bias.unwrap_or_default().to_bits(),
            index as u64,
        ],
    );
    Ok(noise
        .synthesize_touches(mode, intended, &mut rng)?
        .map(|touches| {
            touches
                .into_iter()
                .map(|(key, point)| (key, eval.touch_points.then_some(point)))
                .collect()
        }))
}

/// 用例、每条用例想按的键（编不出来的为 `None`）和不带触点原样打出的结果，三者按用例序号对齐。
type CaseKeys<'a> = (&'a [Case], &'a [Option<Vec<u8>>], &'a [CandidateResult]);

/// 一对两键码：按到的、想按的。
type CodePair = ([u8; 2], [u8; 2]);

/// 一个方案在一组配置下的结果和 noisy 集跳过的条数。`ranks` 是这一遍的记忆表。`record` 给了时（正式那一遍）把打出的表追加进它的 `dump`，并把 clean 集实际首选的变化计进它的 `clean_top1_changed`；学习回放不给，不碰这两项。
#[allow(clippy::too_many_arguments)]
fn measure_setting(
    runtime: &mut Runtime<Session>,
    ranks: &mut Ranks,
    noises: &Noises,
    (cases, intended, reference): CaseKeys<'_>,
    eval: &ShuangpinEval,
    profile: ShuangpinProfileKind,
    mode: TouchMode,
    setting: Option<(f64, f64)>,
    geometry_id: u32,
    mut record: Option<&mut ProfileRun>,
) -> Result<(TouchReport, usize), Box<dyn std::error::Error>> {
    let layout = match setting {
        Some(_) => Some(touch_noise::layout_for(&eval.layouts, profile)?),
        None => None,
    };
    // noisy 和 realistic 按落点分布换键；clean 每键都按在本键上，只在要送落点时才抽落点，不送落点时它就是原样的按键。
    let noise = match (layout, setting, mode) {
        (Some(layout), Some((sigma, bias)), TouchMode::Noisy | TouchMode::Realistic) => {
            Some(&noises[&(layout, sigma.to_bits(), bias.to_bits())])
        }
        (Some(layout), Some((sigma, bias)), TouchMode::Clean) if eval.touch_points => {
            Some(&noises[&(layout, sigma.to_bits(), bias.to_bits())])
        }
        _ => None,
    };
    let setting = setting.unzip();
    let mut report = TouchReport::default();
    let mut skipped = 0;
    for (index, case) in cases.iter().enumerate() {
        let Some(intended) = &intended[index] else {
            continue;
        };
        // σ 为 0 时没有一键会按错，这条用例不进 noisy 集。
        let Some(typed) = touch_typed_keys(noise, eval, profile, mode, setting, index, intended)?
        else {
            skipped += 1;
            continue;
        };
        let errors = typed
            .iter()
            .zip(intended)
            .filter(|((typed, _), intended)| typed != *intended)
            .count();
        let rank = gold_rank(
            runtime,
            ranks,
            (index, case),
            &typed,
            geometry_id,
            profile,
            record.as_mut().and_then(|record| record.dump.as_mut()),
        )?;
        report.observe(TouchObservation {
            syllables: case.syllables,
            errors,
            rank: rank.rank,
            reference: reference[index].rank,
        });
        let was = &reference[index];
        report.first_seat.observe(
            was.first.as_deref(),
            rank.first.as_deref(),
            rank.confident,
            rank.rank == Some(1),
            (mode == TouchMode::Clean).then_some(|| ChangedFirst {
                id: case.id.clone(),
                keys: typed.iter().map(|(key, _)| char::from(*key)).collect(),
                gold: case.gold.clone(),
                was: was.first.clone(),
                now: rank.first.clone(),
                confident: rank.confident,
            }),
        );
        if let Some(record) = record.as_mut() {
            record.observe_top1(mode, &reference[index], &rank);
        }
    }
    Ok((report, skipped))
}

/// 这组配置的 noisy 集里按错的每一对两键码（去重，按出现的先后）：一个音节的两键里只错了一键，或两键对调，正是触点纠错会产出、用户选中纠错行时会记下的那种改动。两键都错的音节纠错改不回来，不记。
fn noisy_pairs(
    noise: &TouchNoise,
    intended: &[Option<Vec<u8>>],
    eval: &ShuangpinEval,
    profile: ShuangpinProfileKind,
    sigma: f64,
    bias: f64,
) -> Result<Vec<CodePair>, String> {
    let mut pairs = Vec::new();
    for (index, intended) in intended.iter().enumerate() {
        let Some(intended) = intended else {
            continue;
        };
        let Some(typed) = touch_typed_keys(
            Some(noise),
            eval,
            profile,
            TouchMode::Noisy,
            (Some(sigma), Some(bias)),
            index,
            intended,
        )?
        else {
            continue;
        };
        for (typed, aimed) in typed.chunks_exact(2).zip(intended.chunks_exact(2)) {
            let typed = [typed[0].0, typed[1].0];
            let aimed = [aimed[0], aimed[1]];
            let changed = typed.iter().zip(&aimed).filter(|(a, b)| a != b).count();
            let one_edit = changed == 1 || (changed == 2 && typed == [aimed[1], aimed[0]]);
            if one_edit && !pairs.contains(&(typed, aimed)) {
                pairs.push((typed, aimed));
            }
        }
    }
    Ok(pairs)
}

/// 把方案 `profile` 下的双拼纠错计数换成「`pairs` 里每一对都接受过 `times` 次」；`pairs` 为空时清空。直接写评测自己那份用户目录里的日志，等于用户在别处接受过这么多次同一对：接受路径本身（选中纠错行时记哪一对）由引擎的会话测试钉住，这里只量计数对排序的作用，不让选中整句顺带记下的整句词条和上下文混进来。建表语句与 `msime-engine` 的 `journal.rs` 相同，引擎还没建出日志时由这里建。写完把日志的修改时间往后挪，引擎的纠错档案按（修改时间、大小）判断要不要重读，同一秒里写两次可能看不出来。
fn write_learned_pairs(
    journal: &Path,
    profile: ShuangpinProfileKind,
    pairs: &[CodePair],
    times: i64,
) -> Result<(), Box<dyn std::error::Error>> {
    static STAMP: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let mut connection = rusqlite::Connection::open(journal)?;
    connection.busy_timeout(std::time::Duration::from_secs(5))?;
    let transaction = connection.transaction()?;
    transaction.execute_batch(
        "CREATE TABLE IF NOT EXISTS shuangpin_typo_counts(\
profile TEXT NOT NULL,typed TEXT NOT NULL,intended TEXT NOT NULL,\
accepted INTEGER NOT NULL DEFAULT 0 CHECK(accepted BETWEEN 0 AND 1000),\
rejected INTEGER NOT NULL DEFAULT 0 CHECK(rejected BETWEEN 0 AND 1000),\
updated_at INTEGER NOT NULL DEFAULT(unixepoch()),\
PRIMARY KEY(profile,typed,intended));",
    )?;
    transaction.execute(
        "DELETE FROM shuangpin_typo_counts WHERE profile=?1",
        [profile.name()],
    )?;
    for (typed, intended) in pairs {
        transaction.execute(
            "INSERT INTO shuangpin_typo_counts(profile,typed,intended,accepted) VALUES(?1,?2,?3,?4)",
            rusqlite::params![
                profile.name(),
                std::str::from_utf8(typed)?,
                std::str::from_utf8(intended)?,
                times
            ],
        )?;
    }
    transaction.commit()?;
    drop(connection);
    let shift = STAMP.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    std::fs::File::options()
        .write(true)
        .open(journal)?
        .set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(10 * shift))?;
    Ok(())
}

#[derive(Clone, Default, Debug)]
struct CandidateResult {
    rank: Option<usize>,
    first: Option<String>,
    /// 首选是按首选规则坐上去的纠错行。
    confident: bool,
}

impl CandidateResult {
    fn from_candidates(candidates: &[(String, u8)], gold: &str) -> Self {
        Self {
            rank: candidates
                .iter()
                .position(|(text, _)| text == gold)
                .map(|position| position + 1),
            first: candidates.first().map(|(text, _)| text.clone()),
            confident: false,
        }
    }
}

impl ProfileRun {
    fn observe_top1(
        &mut self,
        mode: TouchMode,
        reference: &CandidateResult,
        actual: &CandidateResult,
    ) {
        if mode == TouchMode::Clean && reference.first != actual.first {
            self.clean_top1_changed += 1;
        }
    }
}

/// 各组（几何下标，σ 的位，偏置的位）的落点分布，`run_shuangpin` 开线程前算好。
type Noises = HashMap<(usize, u64, u64), TouchNoise>;

/// `gold_rank` 的记忆表：（用例序号，按键，引擎收到的触点）到金标准名次及实际首选。
type Ranks = HashMap<(usize, Vec<u8>, Vec<Option<KeyTouch>>), CandidateResult>;

/// 金标准在 `typed` 打出的候选表里的名次（从 1 起）及实际首选。带落点的键先按代次 `geometry_id` 换成引擎收到的触点；同一条用例的同一串按键带同样的触点只打一次。`dumped` 给了时把这次打出的整张表追加一行 JSONL。
fn gold_rank(
    runtime: &mut Runtime<Session>,
    ranks: &mut Ranks,
    (index, case): (usize, &Case),
    typed: &[TypedKey],
    geometry_id: u32,
    profile: ShuangpinProfileKind,
    dumped: Option<&mut String>,
) -> Result<CandidateResult, Box<dyn std::error::Error>> {
    let keys: Vec<u8> = typed.iter().map(|(key, _)| *key).collect();
    let touches: Vec<Option<KeyTouch>> = typed
        .iter()
        .map(|&(key, point)| {
            point.and_then(|(x, y)| runtime.key_touch(key, geometry_id, x as f32, y as f32))
        })
        .collect();
    let memo = (index, keys, touches);
    if let Some(rank) = ranks.get(&memo) {
        return Ok(rank.clone());
    }
    let (_, keys, touches) = &memo;
    let (candidates, confident) = type_case(
        runtime,
        &case.context,
        keys.iter().zip(touches).map(|(&value, &touch)| {
            (
                Action::Character {
                    value,
                    shift: false,
                },
                touch,
            )
        }),
    )?;
    let rank = CandidateResult {
        confident,
        ..CandidateResult::from_candidates(&candidates, &case.gold)
    };
    if let Some(dumped) = dumped {
        let rows: Vec<serde_json::Value> = candidates
            .iter()
            .map(|(text, source)| serde_json::json!({"text": text, "source": source}))
            .collect();
        let mut line = serde_json::json!({
            "profile": profile.name(), "id": case.id, "input": case.input,
            "keys": String::from_utf8_lossy(keys), "gold": case.gold,
            "context": case.context, "candidates": rows,
        });
        // 只在首选是按首选规则坐上去的纠错行时写，没有这种行的 dump 与以前逐字节相同。
        if confident {
            line["confident"] = true.into();
        }
        // 只在送了触点时写：引擎收到的量化偏移（1/20 键）和第几个实例。
        if touches.iter().any(Option::is_some) {
            line["touches"] = touches
                .iter()
                .map(|touch| {
                    touch.map_or(serde_json::Value::Null, |touch| {
                        serde_json::json!([touch.dx, touch.dy, touch.copy])
                    })
                })
                .collect();
        }
        dumped.push_str(&line.to_string());
        dumped.push('\n');
    }
    ranks.insert(memo, rank.clone());
    Ok(rank)
}

fn touch_bucket(b: &TouchBucket) -> serde_json::Value {
    let rate = |hits: usize| round(Bucket::rate(hits, b.cases));
    serde_json::json!({
        "cases": b.cases,
        "top1": rate(b.top1),
        "top3": rate(b.top3),
        "top9": rate(b.top9),
        "found": rate(b.found),
        "mrr": round(if b.cases == 0 { 0.0 } else { b.reciprocal_rank / b.cases as f64 }),
        // 计数而不是比例：首选被改这类事件要逐条看，千分之一的取整会把它抹掉。
        "top1_changed": b.top1_changed,
        "gold_demoted": b.gold_demoted,
    })
}

/// 一组配置的结果，占四行：配置本身、总体、按音节档、按错键数档。整份报告有上百组，每组摊开成几十行会让基线的改动没法读。
#[allow(clippy::too_many_arguments)]
fn render_touch_run(
    profile: ShuangpinProfileKind,
    geometry: Option<&str>,
    mode: TouchMode,
    sigma: Option<f64>,
    bias: Option<f64>,
    skipped: usize,
    report: &TouchReport,
    learned: Option<(i64, usize)>,
) -> String {
    let json = |value: serde_json::Value| value.to_string();
    // 学习回放的行多两项：每一对记成接受过几次、记了多少对。正式那一遍没有这两项，报告与以前逐字节相同。
    let learned = learned.map_or(String::new(), |(times, pairs)| {
        format!(", \"learned\": {times}, \"learned_pairs\": {pairs}")
    });
    let buckets = |map: &BTreeMap<&'static str, TouchBucket>| {
        json(serde_json::Value::Object(
            map.iter()
                .map(|(band, bucket)| (band.to_string(), touch_bucket(bucket)))
                .collect(),
        ))
    };
    // 送了触点的模式多一行首选规则的统计；none 模式就是对照，没有这一行，与以前逐字节相同。
    let first_seat = if mode == TouchMode::None {
        String::new()
    } else {
        format!(
            ",\n      \"first_seat\": {}",
            json(first_seat_json(&report.first_seat))
        )
    };
    format!(
        "    {{\"profile\": {}, \"geometry\": {}, \"mode\": {}, \"sigma\": {}, \"bias_y\": {}, \"skipped\": {}{learned},\n      \"overall\": {},\n      \"by_syllables\": {},\n      \"by_errors\": {}{first_seat}}}",
        json(profile.name().into()),
        json(geometry.into()),
        json(mode.name().into()),
        json(sigma.into()),
        json(bias.into()),
        skipped,
        json(touch_bucket(&report.overall)),
        buckets(&report.by_syllables),
        buckets(&report.by_errors),
    )
}

/// 首选规则的计数；clean 集另外逐条列出首选被改的用例（原样输入的首选、这次的首选、是不是纠错行坐上去的），供人工复核。
fn first_seat_json(seat: &FirstSeat) -> serde_json::Value {
    let mut value = serde_json::json!({
        "first_changed": seat.first_changed,
        "confident": seat.confident,
        "confident_gold": seat.confident_gold,
    });
    if !seat.changed.is_empty() {
        value["changed"] = seat
            .changed
            .iter()
            .map(|case| {
                serde_json::json!({
                    "id": case.id, "keys": case.keys, "gold": case.gold,
                    "was": case.was, "now": case.now, "confident": case.confident,
                })
            })
            .collect();
    }
    value
}

fn render_shuangpin(
    eval: &ShuangpinEval,
    autocorrect: bool,
    encoding: &[(&str, usize, usize)],
    runs: &[String],
) -> String {
    let encoding: serde_json::Map<String, serde_json::Value> = encoding
        .iter()
        .map(|(profile, cases, encoded)| {
            (
                profile.to_string(),
                serde_json::json!({"cases": cases, "encoded": encoded}),
            )
        })
        .collect();
    let geometry: Vec<&str> = eval
        .layouts
        .iter()
        .map(|layout| layout.name.as_str())
        .collect();
    // 只在 `--touch-tuning` 给了时写，默认参数写在引擎里。
    let tuning = eval.tuning.map_or(String::new(), |tuning| {
        format!(
            "  \"touch_tuning\": {},\n",
            serde_json::json!({
                "base": tuning.base, "scale": tuning.scale,
                "sigma_x": tuning.sigma_x, "sigma_y": tuning.sigma_y, "bias_y": tuning.bias_y,
                "min": tuning.min_penalty, "max": tuning.max_penalty, "cut": tuning.cut,
                "show": tuning.show_margin, "show_pen": tuning.show_penalty,
                "lead": tuning.lead_margin, "lead_pen": tuning.lead_penalty,
            })
        )
    });
    format!(
        "{{\n  \"scheme\": \"shuangpin\",\n  \"autocorrect\": {autocorrect},\n  \"seed\": {},\n  \"geometry\": {},\n{tuning}  \"encoding\": {},\n  \"runs\": [\n{}\n  ]\n}}",
        eval.seed,
        serde_json::Value::from(geometry),
        serde_json::Value::Object(encoding),
        runs.join(",\n"),
    )
}

fn render(report: &Report, cases: &[Case]) -> String {
    let bucket = |b: &Bucket| {
        serde_json::json!({
            "cases": b.cases,
            "top1": round(Bucket::rate(b.top1, b.cases)),
            "top5": round(Bucket::rate(b.top5, b.cases)),
            "top9": round(Bucket::rate(b.top9, b.cases)),
            "found": round(Bucket::rate(b.found, b.cases)),
            "mrr": round(if b.cases == 0 { 0.0 } else { b.reciprocal_rank / b.cases as f64 }),
            "top1_char_prefix": round(if b.cases == 0 { 0.0 } else { b.top1_char_prefix / b.cases as f64 }),
        })
    };
    let by_syllables: BTreeMap<String, _> = report
        .by_syllables
        .iter()
        .map(|(k, v)| (k.to_string(), bucket(v)))
        .collect();
    let by_tag: BTreeMap<String, _> = report
        .by_tag
        .iter()
        .map(|(k, v)| (k.clone(), bucket(v)))
        .collect();

    let value = serde_json::json!({
        "cases": cases.len(),
        "overall": bucket(&report.overall),
        "by_syllables": by_syllables,
        "by_tag": by_tag,
        "gold_source": report.gold_source,
        "top1_source": report.top1_source,
        "sample_failures": report.failures.iter().take(40).map(|f| serde_json::json!({
            "id": f.id, "input": f.input, "gold": f.gold, "top1": f.got, "rank": f.rank,
        })).collect::<Vec<_>>(),
    });
    serde_json::to_string_pretty(&value).unwrap_or_default()
}

/// Three decimals: enough to see a real move, coarse enough that a one-case difference in a large
/// set does not rewrite the whole baseline file.
fn round(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}

/// 用例在网格上打的组码。截短的用例在九键双拼上按打全的读法切音节、去掉最后一键：一个音节两键，停在末音节的声母上就是只打了它的第一键，两种截法在这里是同一串。
fn typed_keys(grid: KeyGrid, case: &Case) -> Option<String> {
    match (grid, &case.full_input) {
        (KeyGrid::NineKeyShuangpin(_), Some(full)) => {
            grid_keys::grid_keys(grid, full, Some(case.syllables)).map(|mut keys| {
                keys.pop();
                keys
            })
        }
        _ => grid_keys::grid_keys(grid, &case.input, Some(case.syllables)),
    }
}

/// 截短后的输入。音节按引擎自己的 `normalize_full_pinyin` 切，切成金标准那么多个音节（`build_eval_set` 收用例时用的也是它），末音节按 `cut` 截短；输入里原有的 `'` 照样保留。切不开，或者截了跟没截一样（末音节只有一个字母，`Letter` 时末音节不以 zh、ch、sh 开头）时为 `None`，这条用例不计入。
fn cut_last_syllable(input: &str, syllables: usize, cut: Cut) -> Option<String> {
    let normalized = msime_engine::host::normalize_full_pinyin(input, syllables);
    let (head, last) = normalized
        .rsplit_once('\'')
        .unwrap_or(("", normalized.as_str()));
    let initial = if ["zh", "ch", "sh"]
        .iter()
        .any(|retroflex| last.starts_with(retroflex))
    {
        2
    } else {
        1
    };
    let kept = match cut {
        Cut::Initial => initial,
        Cut::Letter if initial > 1 => 1,
        Cut::Letter => return None,
    };
    if last.len() <= kept {
        return None;
    }
    let letters = head.bytes().filter(|&byte| byte != b'\'').count() + kept;
    let mut typed = String::with_capacity(input.len());
    let mut counted = 0;
    for character in input.chars() {
        if counted == letters {
            break;
        }
        if character != '\'' {
            counted += 1;
        }
        typed.push(character);
    }
    Some(typed)
}

/// 一键怎么送：九键和九键双拼的数字、全键盘的字母走 `Character`，字母网格的组码走宿主触屏键面用的 `GridKey`。
fn key_action(grid: Option<KeyGrid>, byte: u8) -> Action {
    match grid {
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
    }
}

#[cfg(test)]
mod top1_tests {
    use super::*;

    fn result(texts: &[&str], gold: &str) -> CandidateResult {
        CandidateResult::from_candidates(
            &texts
                .iter()
                .map(|text| ((*text).to_string(), 0))
                .collect::<Vec<_>>(),
            gold,
        )
    }

    #[test]
    fn clean_gate_compares_actual_top1_even_when_gold_rank_is_unchanged() {
        for gold in ["金标准", "未出现"] {
            let reference = result(&["候选甲", "候选乙", "金标准"], gold);
            let actual = result(&["候选乙", "候选甲", "金标准"], gold);
            assert_eq!(reference.rank, actual.rank);
            let mut run = ProfileRun::default();
            run.observe_top1(TouchMode::Clean, &reference, &actual);
            assert_eq!(run.clean_top1_changed, 1);
        }
    }

    #[test]
    fn clean_gate_counts_empty_transitions_and_ignores_lower_rank_changes() {
        let reference = result(&["候选甲", "金标准", "候选乙"], "金标准");
        let same_first = result(&["候选甲", "候选乙", "金标准"], "金标准");
        let empty = result(&[], "金标准");
        let mut run = ProfileRun::default();
        run.observe_top1(TouchMode::Clean, &reference, &same_first);
        run.observe_top1(TouchMode::Clean, &empty, &empty);
        run.observe_top1(TouchMode::Noisy, &reference, &empty);
        assert_eq!(run.clean_top1_changed, 0);
        run.observe_top1(TouchMode::Clean, &reference, &empty);
        run.observe_top1(TouchMode::Clean, &empty, &reference);
        assert_eq!(run.clean_top1_changed, 2);
    }
}

#[cfg(test)]
mod tuning_tests {
    use super::*;

    #[test]
    fn reversed_penalty_range_is_rejected_before_evaluation() {
        assert!(parse_tuning("min=20,max=10").is_err());
        assert!(parse_tuning("min=10,max=10").is_ok());
    }

    #[test]
    fn the_first_seat_rule_is_tuned_by_name() {
        let tuning = parse_tuning("lead=3.5,lead_pen=9").unwrap();
        assert_eq!((tuning.lead_margin, tuning.lead_penalty), (3.5, 9.0));
        assert_eq!(
            TouchTuning {
                lead_margin: TouchTuning::default().lead_margin,
                lead_penalty: TouchTuning::default().lead_penalty,
                ..tuning
            },
            TouchTuning::default()
        );
        assert!(parse_tuning("lead=inf").is_err());
    }

    #[test]
    fn the_first_seat_gate_follows_the_plan() {
        let run = |profile, sigma, changed| (profile, sigma, 1000, changed);
        // 每组 1000 条：σ=0 一条都不能变；总体 0.1%、每个方案 0.2%。
        assert!(first_seat_gate(&[run("xiaohe", 0.0, 0), run("xiaohe", 0.25, 1)]).is_ok());
        assert!(first_seat_gate(&[run("xiaohe", 0.0, 1), run("xiaohe", 0.25, 0)]).is_err());
        // 小鹤 4/2000 刚好 0.2%，总体 4/4000 刚好 0.1%：都在门槛上，通过。
        let edge = [
            run("xiaohe", 0.0, 0),
            run("xiaohe", 0.25, 4),
            run("microsoft", 0.0, 0),
            run("microsoft", 0.25, 0),
        ];
        assert!(first_seat_gate(&edge).is_ok());
        // 小鹤 5/2000 超过 0.2%，总体 5/4000 也超过 0.1%。
        let over = [
            run("xiaohe", 0.0, 0),
            run("xiaohe", 0.25, 5),
            run("microsoft", 0.0, 0),
            run("microsoft", 0.25, 0),
        ];
        let error = first_seat_gate(&over).unwrap_err();
        assert!(error.contains("xiaohe: 5 of 2000"), "{error}");
        assert!(error.contains("overall: 5 of 4000"), "{error}");
        assert!(first_seat_gate(&[]).is_err());
    }
}
