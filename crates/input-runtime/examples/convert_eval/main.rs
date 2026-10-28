//! Measures pinyin-to-text conversion quality against the locked dictionary.
//!
//! Why this exists: neither the word lattice nor any later reranking can be shown to help or hurt
//! without a number to compare against, and the repository had none.
//!
//! Why it reports per source rather than top-1 alone: for two or more complete syllables the Engine inserts lattice sentence rows among the dictionary rows, so whether position 1 is a dictionary word or a lattice sentence depends on the input, and whether a lattice change can move top-1 depends on that. `top1_source` and `gold_source` record it per run instead of assuming either way.
//!
//! Learning is off and each case gets a fresh session, so one case cannot bias the next.
//!
//! `--scheme shuangpin` 把同样的用例编成 26 键双拼的按键再打，`--touch-mode` 在参考键位几何上合成误触，量双拼纠错的召回和对正确输入的影响；报告形状和跑法见 resources/eval/README.md「双拼与触屏误触」。

#[path = "../eval_resources/mod.rs"]
mod eval_resources;
mod grid_keys;
mod metrics;
mod touch_noise;

use metrics::{Bucket, Observation, Report, TouchBucket, TouchObservation, TouchReport};
use msime_engine::host::{Command, EngineOptions, Session};
use msime_engine::{KeyGrid, SchemeType, ShuangpinProfileKind};
use msime_input_runtime::{Action, Reranker, Runtime, SentenceModel};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use touch_noise::{KeyLayout, TouchMode, TouchNoise};

/// 26 键双拼的内置方案，按 `ShuangpinProfileKind` 的序号；`custom` 没有内置的表，评测不了。
const SHUANGPIN_PROFILES: [ShuangpinProfileKind; 5] = [
    ShuangpinProfileKind::Xiaohe,
    ShuangpinProfileKind::Ziranma,
    ShuangpinProfileKind::Shoudao,
    ShuangpinProfileKind::Microsoft,
    ShuangpinProfileKind::SmartAbc,
];

/// `--seed` 的默认值。
const DEFAULT_SEED: u64 = 20261027;

/// `--scheme shuangpin` 的参数。
struct ShuangpinEval {
    profiles: Vec<ShuangpinProfileKind>,
    layouts: Vec<KeyLayout>,
    modes: Vec<TouchMode>,
    sigmas: Vec<f64>,
    biases: Vec<f64>,
    seed: u64,
}

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
            "--max-syllables" => max_syllables = Some(take(&mut index)?.parse()?),
            "--shuangpin-profile" => {
                for name in take(&mut index)?.split(',') {
                    profiles.push(
                        SHUANGPIN_PROFILES
                            .into_iter()
                            .find(|kind| kind.name() == name)
                            .ok_or_else(|| format!("unknown shuangpin profile: {name} (expected xiaohe, ziranma, shoudao, microsoft or smart_abc)"))?,
                    );
                }
            }
            "--key-geometry" => geometry.push(PathBuf::from(take(&mut index)?)),
            "--touch-mode" => {
                for name in take(&mut index)?.split(',') {
                    modes.push(TouchMode::parse(name)?);
                }
            }
            "--touch-sigma" => sigmas.extend(parse_list(&take(&mut index)?, "--touch-sigma")?),
            "--touch-bias-y" => biases.extend(parse_list(&take(&mut index)?, "--touch-bias-y")?),
            "--seed" => seed = Some(take(&mut index)?.parse()?),
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
        let layouts = geometry
            .iter()
            .map(|path| KeyLayout::load(path))
            .collect::<Result<Vec<_>, _>>()?;
        if touches {
            for profile in &profiles {
                layout_for(&layouts, *profile)?;
            }
        }
        Some(ShuangpinEval {
            profiles,
            layouts,
            modes,
            sigmas,
            biases,
            seed: seed.unwrap_or(DEFAULT_SEED),
        })
    } else {
        if !profiles.is_empty()
            || !geometry.is_empty()
            || !modes.is_empty()
            || !sigmas.is_empty()
            || !biases.is_empty()
            || seed.is_some()
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

/// `--key-geometry` 给的几何里，`profile` 该用的那一份：宿主只在微软双拼下显示 `;` 键，第二行因此多一个键，所以微软双拼取带 `;` 的那份，其余方案取不带的。
fn layout_for(layouts: &[KeyLayout], profile: ShuangpinProfileKind) -> Result<&KeyLayout, String> {
    let semicolon = profile == ShuangpinProfileKind::Microsoft;
    layouts
        .iter()
        .find(|layout| layout.has_semicolon() == semicolon)
        .ok_or_else(|| {
            format!(
                "{} needs a --key-geometry {} a ; key",
                profile.name(),
                if semicolon { "with" } else { "without" }
            )
        })
}

/// 逗号分隔的一列有限、非负的数。
fn parse_list(text: &str, option: &str) -> Result<Vec<f64>, String> {
    text.split(',')
        .map(|item| match item.parse::<f64>() {
            Ok(value) if value.is_finite() && value >= 0.0 => Ok(value),
            _ => Err(format!(
                "{option}: {item} is not a finite non-negative number"
            )),
        })
        .collect()
}

/// 建一个和宿主同样配置的 Runtime：关学习、关调频、关混输和本地模式，挂上资源里有的重排模型。`configure` 在建会话前改方案相关的选项。
fn open_runtime(
    resources: &Path,
    state: &Path,
    keyboard_neural: bool,
    autocorrect: bool,
    first_page_rows: Option<usize>,
    source_commit: &str,
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
    // Measure the shipped path: the host asks for every reading and crops the list itself.
    options.sentence_alternatives = true;
    options.sentence_association.neural_keyboard = keyboard_neural;
    // 首页行数同时交给引擎排序和运行时分页，未指定时保留各自的默认值。
    if let Some(rows) = first_page_rows {
        options.first_page_rows = rows;
    }
    configure(&mut options);

    let engine = Session::new(&options)?;
    // page_size is capped at 9 by the runtime, but all_candidates() is not paginated.
    let page_size = first_page_rows.map_or(9, |rows| rows.clamp(1, 9));
    let mut runtime = Runtime::new(engine, u8::try_from(page_size)?)?;
    // Measuring the shipped path means measuring it with whatever the resource set contains. The
    // model is optional there, so its absence has to leave these numbers exactly as they were.
    // The settled model, when the resource set ships one. Attaching it here is what lets the eval
    // measure the shipped pair rather than either model alone: the fast one ranks every keystroke
    // and this one ranks once at the end, which is exactly what a host does with its settle timer.
    let settled_path = resources.join("sentence-model-desktop.safetensors");
    match std::fs::read(&settled_path) {
        Ok(bytes) => {
            runtime.set_settled_reranker(Some(Reranker::new(std::sync::Arc::new(
                SentenceModel::load(&bytes)?,
            ))));
            eprintln!("settling with {}", settled_path.display());
        }
        Err(_) => eprintln!("no settled model at {}", settled_path.display()),
    }
    let model_path = resources.join("sentence-model.safetensors");
    match std::fs::read(&model_path) {
        Ok(bytes) => {
            let model = SentenceModel::load(&bytes)?;
            eprintln!("reranking with {}", model_path.display());
            runtime.set_reranker(Some(Reranker::new(std::sync::Arc::new(model))));
        }
        Err(_) => eprintln!(
            "no model at {}, measuring the engine alone",
            model_path.display()
        ),
    }
    // dispatch() drops every action while unfocused, which yields an empty candidate list rather
    // than an error.
    runtime.focus(true)?;
    Ok(runtime)
}

/// 在清空的组字和上文里打一条用例，返回停顿重排之后的整张候选表（文字和来源）。
fn type_case(
    runtime: &mut Runtime<Session>,
    context: &str,
    keys: impl IntoIterator<Item = Action>,
) -> Result<Vec<(String, u8)>, Box<dyn std::error::Error>> {
    runtime.dispatch(Action::Command(Command::Cancel))?;
    // Cancel keeps the committed context and seeding appends to it, so without this every case would be ranked against the tail of the contexts of all the cases before it.
    runtime.clear_context();
    runtime.seed_context(context);
    for key in keys {
        runtime.dispatch(key)?;
    }
    // The user has stopped typing by the time a case is read, which is when a host fires its
    // settle timer.
    runtime.rerank_settled();
    let snapshot = runtime.all_candidates();
    let candidates = snapshot
        .candidates
        .iter()
        .map(|c| (c.text.clone(), c.source))
        .collect();
    runtime.dispatch(Action::Command(Command::Cancel))?;
    Ok(candidates)
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
    first_page_rows: Option<usize>,
    source_commit: &str,
) -> Result<Report, Box<dyn std::error::Error>> {
    // Optional JSONL of the real candidate lists, for offline experiments that must not be able
    // to change what the harness measures.
    let mut dumped = String::new();
    let mut runtime = open_runtime(
        resources,
        state,
        keyboard_neural,
        autocorrect,
        first_page_rows,
        source_commit,
        |_| {},
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
        let candidates = type_case(
            &mut runtime,
            &case.context,
            input.into_iter().map(|byte| key_action(grid, byte)),
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

/// `--scheme shuangpin`：每个方案建一个双拼 Runtime，把用例编成这个方案的按键；先按想按的键原样打一遍作对照，再按每组（触点模式、σ、偏置）合成实际按到的键去打。返回整份报告的 JSON。
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
) -> Result<String, Box<dyn std::error::Error>> {
    let mut dumped = dump.map(|_| String::new());
    let mut encoding = Vec::new();
    let mut runs = Vec::new();
    for &profile in &eval.profiles {
        let mut runtime = open_runtime(
            resources,
            state,
            keyboard_neural,
            autocorrect,
            first_page_rows,
            source_commit,
            |options| {
                options.scheme = SchemeType::Shuangpin as u8;
                options.shuangpin_profile = profile as u8;
            },
        )?;
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
        encoding.push((profile.name(), cases.len(), encoded));
        // 一条用例的候选表只取决于它的上文和送进去的按键，所以同一串按键在各组配置里只打一次。
        let mut ranks: HashMap<(usize, Vec<u8>), Option<usize>> = HashMap::new();
        let mut reference = vec![None; cases.len()];
        for (index, keys) in intended.iter().enumerate() {
            if let Some(keys) = keys {
                reference[index] = gold_rank(
                    &mut runtime,
                    &mut ranks,
                    (index, &cases[index]),
                    keys,
                    profile,
                    dumped.as_mut(),
                )?;
            }
        }
        for &mode in &eval.modes {
            let settings: Vec<Option<(f64, f64)>> = if mode == TouchMode::None {
                vec![None]
            } else {
                eval.sigmas
                    .iter()
                    .flat_map(|&sigma| eval.biases.iter().map(move |&bias| Some((sigma, bias))))
                    .collect()
            };
            for setting in settings {
                let layout = match setting {
                    Some(_) => Some(layout_for(&eval.layouts, profile)?),
                    None => None,
                };
                // 每键都按在本键上的 clean 不用抽样，只有 noisy 和 realistic 要按落点分布换键。
                let noise = match (layout, setting, mode) {
                    (
                        Some(layout),
                        Some((sigma, bias)),
                        TouchMode::Noisy | TouchMode::Realistic,
                    ) => Some(TouchNoise::new(layout, sigma, bias)),
                    _ => None,
                };
                let (sigma, bias) = setting.unzip();
                let mut report = TouchReport::default();
                let mut skipped = 0;
                for (index, case) in cases.iter().enumerate() {
                    let Some(intended) = &intended[index] else {
                        continue;
                    };
                    let typed = match &noise {
                        Some(noise) => {
                            let mut rng = touch_noise::case_rng(
                                eval.seed,
                                &[
                                    profile as u64,
                                    mode as u64,
                                    sigma.unwrap_or_default().to_bits(),
                                    bias.unwrap_or_default().to_bits(),
                                    index as u64,
                                ],
                            );
                            noise.synthesize(mode, intended, &mut rng)?
                        }
                        None => Some(intended.clone()),
                    };
                    // σ 为 0 时没有一键会按错，这条用例不进 noisy 集。
                    let Some(typed) = typed else {
                        skipped += 1;
                        continue;
                    };
                    let errors = typed.iter().zip(intended).filter(|(a, b)| a != b).count();
                    let rank = gold_rank(
                        &mut runtime,
                        &mut ranks,
                        (index, case),
                        &typed,
                        profile,
                        dumped.as_mut(),
                    )?;
                    report.observe(TouchObservation {
                        syllables: case.syllables,
                        errors,
                        rank,
                        reference: reference[index],
                    });
                }
                runs.push(render_touch_run(
                    profile,
                    layout.map(|layout| layout.name.as_str()),
                    mode,
                    sigma,
                    bias,
                    skipped,
                    &report,
                ));
            }
        }
    }
    if let (Some(path), Some(dumped)) = (dump, dumped) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, dumped)?;
    }
    Ok(render_shuangpin(eval, autocorrect, &encoding, &runs))
}

/// 金标准在 `keys` 打出的候选表里的名次（从 1 起）。同一条用例的同一串按键只打一次；`dumped` 给了时把这次打出的整张表追加一行 JSONL。
fn gold_rank(
    runtime: &mut Runtime<Session>,
    ranks: &mut HashMap<(usize, Vec<u8>), Option<usize>>,
    (index, case): (usize, &Case),
    keys: &[u8],
    profile: ShuangpinProfileKind,
    dumped: Option<&mut String>,
) -> Result<Option<usize>, Box<dyn std::error::Error>> {
    if let Some(rank) = ranks.get(&(index, keys.to_vec())) {
        return Ok(*rank);
    }
    let candidates = type_case(
        runtime,
        &case.context,
        keys.iter().map(|&value| Action::Character {
            value,
            shift: false,
        }),
    )?;
    let rank = candidates
        .iter()
        .position(|(text, _)| *text == case.gold)
        .map(|position| position + 1);
    if let Some(dumped) = dumped {
        let rows: Vec<serde_json::Value> = candidates
            .iter()
            .map(|(text, source)| serde_json::json!({"text": text, "source": source}))
            .collect();
        dumped.push_str(
            &serde_json::json!({
                "profile": profile.name(), "id": case.id, "input": case.input,
                "keys": String::from_utf8_lossy(keys), "gold": case.gold,
                "context": case.context, "candidates": rows,
            })
            .to_string(),
        );
        dumped.push('\n');
    }
    ranks.insert((index, keys.to_vec()), rank);
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
fn render_touch_run(
    profile: ShuangpinProfileKind,
    geometry: Option<&str>,
    mode: TouchMode,
    sigma: Option<f64>,
    bias: Option<f64>,
    skipped: usize,
    report: &TouchReport,
) -> String {
    let json = |value: serde_json::Value| value.to_string();
    let buckets = |map: &BTreeMap<&'static str, TouchBucket>| {
        json(serde_json::Value::Object(
            map.iter()
                .map(|(band, bucket)| (band.to_string(), touch_bucket(bucket)))
                .collect(),
        ))
    };
    format!(
        "    {{\"profile\": {}, \"geometry\": {}, \"mode\": {}, \"sigma\": {}, \"bias_y\": {}, \"skipped\": {},\n      \"overall\": {},\n      \"by_syllables\": {},\n      \"by_errors\": {}}}",
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
    format!(
        "{{\n  \"scheme\": \"shuangpin\",\n  \"autocorrect\": {autocorrect},\n  \"seed\": {},\n  \"geometry\": {},\n  \"encoding\": {},\n  \"runs\": [\n{}\n  ]\n}}",
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
