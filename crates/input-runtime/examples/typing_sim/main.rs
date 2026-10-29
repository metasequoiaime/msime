//! 真实打字模拟：一个用户用手机出厂配置，从头到尾按顺序打一整段文字流，量他实际感受到的准确率。
//!
//! `convert_eval` 量的是冷启动：每条用例互相独立，关学习、清上文，一串键只问一次金标准排第几。真实打字不是这样：用户在同一个会话里一直打，选过的词会往前排，部分选择暂存在组字区（`phrase_preedit`），整句没出来时点前面的词再接着打，拼写列能锁定音节。这里把这些都放进来，量的是用户点了第几个、点了几次。
//!
//! 每块怎么打、怎么选见 `user.rs`，口径见 `metrics.rs`，评测集见 `corpus.rs` 和 `resources/eval/README.md`「打字流评测」一节。
//!
//! 确定性：会话的单调时钟和本地墙钟换成虚拟时钟（`host::Session::set_clock`），按模拟的打字节奏推进，个人学习的 3 s / 8 s / 10 s 窗口和行内日期时间行不随机器快慢和运行的日子变化；切块和简拼只由种子和行号决定。同一份二进制、同一组参数两次运行的报告逐字节相同。

#[path = "../eval_resources/mod.rs"]
mod eval_resources;

mod corpus;
mod latency;
mod metrics;
mod profile;
mod user;

use corpus::{Abbreviation, Chunk, Chunking, Keyboard, Piece, Row, Split};
use metrics::{ChunkRecord, Run, Selection, Totals, Unreachable};
use msime_client_core::preferences::TouchKeyboardLayout;
use msime_engine::host::{Command, EngineOptions, Session};
use msime_engine::{Clock, LocalDateTime};
use msime_input_runtime::{Action, NineKeySpellingId, Reranker, Runtime, SentenceModel};
use profile::Profile;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use user::{Decision, Policy};

/// `--chunk-seed` 和简拼抽样的默认种子。
const DEFAULT_SEED: u64 = 1;
/// bootstrap 的种子，固定，让区间可复现。
const BOOTSTRAP_SEED: u64 = 20261028;
/// 没有拼写列的全键盘找不到时最多翻几页面板。
const FULL_KEYBOARD_PANEL_PAGES: usize = 10;
/// 学习曲线一桶多少字。
const CURVE_BAND: usize = 2000;
/// 报告里最多列多少个不是首选的选择（只列 dev）。
const SAMPLE_FAILURES: usize = 40;

/// 每个动作让虚拟时钟走多久（毫秒）。
#[derive(Clone, Copy, Debug)]
struct Timing {
    key: u64,
    select: u64,
    chunk: u64,
    literal: u64,
    turn: u64,
}

impl Default for Timing {
    fn default() -> Self {
        Self {
            key: 350,
            select: 700,
            chunk: 1000,
            literal: 500,
            turn: 20_000,
        }
    }
}

impl Timing {
    fn parse(text: &str) -> Result<Self, String> {
        let mut timing = Self::default();
        for item in text.split(',') {
            let (name, value) = item
                .split_once('=')
                .ok_or_else(|| format!("--timing expects name=ms pairs, got {item}"))?;
            let value: u64 = value
                .parse()
                .map_err(|_| format!("--timing {name}: {value} is not a whole number"))?;
            match name {
                "key" => timing.key = value,
                "select" => timing.select = value,
                "chunk" => timing.chunk = value,
                "literal" => timing.literal = value,
                "turn" => timing.turn = value,
                other => return Err(format!("--timing: unknown action {other}")),
            }
        }
        Ok(timing)
    }
}

/// 会话读的虚拟时钟：起点取一次真实的 `Instant`，之后只按模拟的动作往前走。墙钟钉在 2026-01-05（周一）10:00。
#[derive(Clone)]
struct VirtualClock {
    start: Instant,
    elapsed_ms: Arc<AtomicU64>,
}

impl VirtualClock {
    fn new() -> Self {
        Self {
            start: Instant::now(),
            elapsed_ms: Arc::new(AtomicU64::new(0)),
        }
    }

    fn clock(&self) -> Clock {
        let start = self.start;
        let elapsed = Arc::clone(&self.elapsed_ms);
        Clock {
            steady: Box::new(move || {
                start + Duration::from_millis(elapsed.load(Ordering::Relaxed))
            }),
            local: Arc::new(|| LocalDateTime {
                year: 2026,
                month: 1,
                day: 5,
                weekday: 1,
                hour: 10,
                minute: 0,
                second: 0,
            }),
        }
    }

    fn advance(&self, ms: u64) {
        self.elapsed_ms.fetch_add(ms, Ordering::Relaxed);
    }
}

struct Args {
    resources: PathBuf,
    resource_lock: Option<PathBuf>,
    set: PathBuf,
    keyboard: Keyboard,
    profile: Profile,
    learning: Option<bool>,
    model: bool,
    chunking: Chunking,
    chunk_name: String,
    abbreviate: f64,
    abbreviation: Abbreviation,
    first_page_rows: Option<usize>,
    strip: Option<usize>,
    panel_page: usize,
    panel_pages: usize,
    timing: Timing,
    seed: u64,
    order_seed: u64,
    slip: f64,
    warmup: bool,
    limit_measure_chars: Option<usize>,
    draws: usize,
    report: Option<PathBuf>,
    dump: Option<PathBuf>,
    /// `--latency`：给每次派发计时，写到这个文件。
    latency: Option<PathBuf>,
    baseline: Option<PathBuf>,
    update_baseline: bool,
    lint_readings: bool,
}

fn parse_args() -> Result<Args, Box<dyn std::error::Error>> {
    let mut resources = None;
    let mut resource_lock = None;
    let mut set = None;
    let mut keyboard = Keyboard::Nine;
    let mut profile = Profile::ProductMobile;
    let mut learning = None;
    let mut model = false;
    let mut chunk = "sampled".to_owned();
    let mut chunk_mean = 3.5;
    // 主配置：三成的块停在末音节声母上（见 resources/eval/README.md「打字流评测」）。
    let mut abbreviate = 0.3;
    let mut abbreviation = Abbreviation::Last;
    let mut first_page_rows = None;
    let mut strip = None;
    let mut panel_page = 24;
    let mut panel_pages = 2;
    let mut timing = Timing::default();
    let mut seed = DEFAULT_SEED;
    let mut order_seed = 0;
    let mut slip = 0.0;
    let mut warmup = true;
    let mut limit_measure_chars = None;
    let mut draws = 1000;
    let mut report = None;
    let mut dump = None;
    let mut latency = None;
    let mut baseline = None;
    let mut update_baseline = false;
    let mut lint_readings = false;

    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut index = 0;
    while index < args.len() {
        let take = |i: &mut usize| -> Result<String, String> {
            *i += 1;
            args.get(*i)
                .cloned()
                .ok_or_else(|| format!("{} needs a value", args[*i - 1]))
        };
        let share = |text: String, option: &str| -> Result<f64, String> {
            match text.parse::<f64>() {
                Ok(value) if (0.0..=1.0).contains(&value) => Ok(value),
                _ => Err(format!("{option}: {text} is not a share between 0 and 1")),
            }
        };
        match args[index].as_str() {
            "--resources" => resources = Some(PathBuf::from(take(&mut index)?)),
            "--resource-lock" => resource_lock = Some(PathBuf::from(take(&mut index)?)),
            "--set" => set = Some(PathBuf::from(take(&mut index)?)),
            "--keyboard" => {
                keyboard = match take(&mut index)?.as_str() {
                    "nine" => Keyboard::Nine,
                    "fourteen" => Keyboard::Fourteen,
                    "full" => Keyboard::Full,
                    other => {
                        return Err(format!(
                            "unknown keyboard {other} (expected nine, fourteen or full)"
                        )
                        .into())
                    }
                }
            }
            "--profile" => {
                profile = match take(&mut index)?.as_str() {
                    "product-mobile" => Profile::ProductMobile,
                    "convert-eval" => Profile::ConvertEval,
                    other => {
                        return Err(format!(
                            "unknown profile {other} (expected product-mobile or convert-eval)"
                        )
                        .into())
                    }
                }
            }
            "--learning" => learning = Some(on_off(&take(&mut index)?, "--learning")?),
            "--model" => {
                model = match take(&mut index)?.as_str() {
                    "off" => false,
                    "keyboard" => true,
                    other => {
                        return Err(
                            format!("unknown model {other} (expected off or keyboard)").into()
                        )
                    }
                }
            }
            "--chunk" => chunk = take(&mut index)?,
            "--chunk-mean" => {
                chunk_mean = take(&mut index)?.parse()?;
                if !(1.0..=8.0).contains(&chunk_mean) {
                    return Err("--chunk-mean must be between 1 and 8".into());
                }
            }
            "--abbreviate" => abbreviate = share(take(&mut index)?, "--abbreviate")?,
            "--abbreviate-mode" => {
                abbreviation = match take(&mut index)?.as_str() {
                    "last" => Abbreviation::Last,
                    "all" => Abbreviation::All,
                    other => {
                        return Err(format!(
                            "unknown abbreviation mode {other} (expected last or all)"
                        )
                        .into())
                    }
                }
            }
            "--first-page-rows" => first_page_rows = Some(take(&mut index)?.parse()?),
            "--strip" => strip = Some(take(&mut index)?.parse()?),
            "--panel-page" => panel_page = take(&mut index)?.parse()?,
            "--panel-pages" => panel_pages = take(&mut index)?.parse()?,
            "--timing" => timing = Timing::parse(&take(&mut index)?)?,
            "--seed" => seed = take(&mut index)?.parse()?,
            "--order-seed" => order_seed = take(&mut index)?.parse()?,
            "--slip" => slip = share(take(&mut index)?, "--slip")?,
            "--warmup" => warmup = on_off(&take(&mut index)?, "--warmup")?,
            "--limit-measure-chars" => limit_measure_chars = Some(take(&mut index)?.parse()?),
            "--bootstrap" => draws = take(&mut index)?.parse()?,
            "--report" => report = Some(PathBuf::from(take(&mut index)?)),
            "--dump" => dump = Some(PathBuf::from(take(&mut index)?)),
            "--latency" => latency = Some(PathBuf::from(take(&mut index)?)),
            "--baseline" => baseline = Some(PathBuf::from(take(&mut index)?)),
            "--update-baseline" => update_baseline = true,
            "--lint-readings" => lint_readings = true,
            other => return Err(format!("unknown option: {other}").into()),
        }
        index += 1;
    }
    let chunking = match chunk.as_str() {
        "sampled" => Chunking::Sampled {
            mean: chunk_mean,
            seed,
        },
        "clause" => Chunking::Clause,
        "word" => Chunking::Word,
        other => {
            return Err(
                format!("unknown chunking {other} (expected sampled, clause or word)").into(),
            )
        }
    };
    let chunk_name = match chunking {
        Chunking::Sampled { mean, .. } => format!("sampled-{mean}"),
        _ => chunk,
    };
    Ok(Args {
        resources: resources.ok_or("usage: typing_sim --resources <verified-dir> --set <typing-*.tsv> [--resource-lock <lock.json>] [...]")?,
        resource_lock,
        set: set.ok_or("--set is required")?,
        keyboard,
        profile,
        learning,
        model,
        chunking,
        chunk_name,
        abbreviate,
        abbreviation,
        first_page_rows,
        strip,
        panel_page,
        panel_pages,
        timing,
        seed,
        order_seed,
        slip,
        warmup,
        limit_measure_chars,
        draws,
        report,
        dump,
        latency,
        baseline,
        update_baseline,
        lint_readings,
    })
}

fn on_off(text: &str, option: &str) -> Result<bool, String> {
    match text {
        "on" => Ok(true),
        "off" => Ok(false),
        other => Err(format!("{option}: expected on or off, got {other}")),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = parse_args()?;
    let verified = eval_resources::verify(&args.resources, args.resource_lock.as_deref())?;
    let (rows, set_hash) = corpus::load(&args.set)?;
    let state = tempfile::tempdir()?;
    let mut options = msime_engine::host::prepare_options(
        args.resources.to_str().ok_or("non-UTF-8 resource path")?,
        state
            .path()
            .join("user")
            .to_str()
            .ok_or("non-UTF-8 state path")?,
        state
            .path()
            .join("cache")
            .to_str()
            .ok_or("non-UTF-8 state path")?,
        &verified.source_commit,
    )?;
    let page_size = args
        .first_page_rows
        .unwrap_or_else(|| profile::page_size(args.profile))
        .clamp(1, 9);
    profile::apply(args.profile, &mut options, page_size);
    if let Some(learning) = args.learning {
        options.learning = learning;
    }
    // 「增强」只挂 runtime 逐键重排器（下面的 `set_reranker`），引擎内的键盘模型层固定关，与 host-api 的 `keyboard_reranker_enabled` 和 `engine_sentence_association` 一致（#10197）。要量引擎层用 `convert_eval --keyboard-neural`。
    options.sentence_association.neural_keyboard = false;

    if args.lint_readings {
        return lint_readings(&options, &rows);
    }

    let clock = VirtualClock::new();
    let mut engine = Session::new(&options)?;
    engine.set_clock(clock.clock());
    let layout = match args.keyboard {
        Keyboard::Full => TouchKeyboardLayout::TwentySixKey,
        Keyboard::Nine => TouchKeyboardLayout::NineKey,
        Keyboard::Fourteen => TouchKeyboardLayout::FourteenKey,
    };
    let mut runtime = Runtime::new_with_touch_layout(engine, u8::try_from(page_size)?, layout)?;
    runtime.set_english_first_page_count(options.english_first_page_count);
    // 三个移动宿主都请求暂存部分选择（Android `MSIMEInputService`、iOS `MetasequoiaInputSessionBridge`、鸿蒙 `KeyboardSession`）；`convert_eval` 不请求。
    let phrase_preedit = args.profile == Profile::ProductMobile;
    runtime.set_phrase_preedit(phrase_preedit);
    if args.model {
        let path = args.resources.join("sentence-model.safetensors");
        let bytes = std::fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))?;
        runtime.set_reranker(Some(Reranker::new(Arc::new(SentenceModel::load(&bytes)?))));
        eprintln!("reranking each keystroke with {}", path.display());
    } else {
        eprintln!("no keyboard model: measuring the engine alone");
    }
    if let Some(grid) = args.keyboard.grid() {
        runtime.set_key_grid(Some(grid))?;
    }
    runtime.focus(true)?;

    let strip = args.strip.unwrap_or(page_size);
    let policy = Policy {
        strip,
        panel_page: args.panel_page,
        panel_pages: if args.keyboard.grid().is_some() {
            args.panel_pages
        } else {
            args.panel_pages.max(FULL_KEYBOARD_PANEL_PAGES)
        },
    };
    let ordered = stream_order(&rows, args.order_seed);
    let learning = options.learning;
    // 学习关着时预热什么也留不下，不打。
    let type_warmup = args.warmup && learning;
    let facts = SetFacts::new(&rows, args.chunking, args.keyboard);

    let mut simulator = Simulator {
        runtime: &mut runtime,
        clock: &clock,
        keyboard: args.keyboard,
        timing: args.timing,
        policy,
        abbreviation: args.abbreviation,
        dump: args.dump.as_ref().map(|_| String::new()),
        failures: Vec::new(),
        latency: args.latency.as_ref().map(|_| latency::Latency::default()),
        chunk_keys: 0,
    };
    let mut run = Run::default();
    let mut seen: HashSet<String> = HashSet::new();
    let mut warmup_keys: HashSet<String> = HashSet::new();
    let mut measured = 0usize;
    let mut warmup_chars = 0usize;
    let mut warmup_rows = 0usize;
    let started = Instant::now();
    'rows: for row in ordered {
        if row.split == Split::Warmup && !type_warmup {
            continue;
        }
        if let Some(limit) = args.limit_measure_chars {
            if row.split != Split::Warmup && measured >= limit {
                break 'rows;
            }
        }
        for (ordinal, piece) in corpus::pieces(row, args.chunking).into_iter().enumerate() {
            match piece {
                Piece::Literal(text) => {
                    simulator.runtime.seed_context(&text);
                    clock.advance(args.timing.literal);
                }
                Piece::Chunk(chunk) => {
                    let ordinal = ordinal as u64;
                    let abbreviated =
                        corpus::abbreviates(args.seed, &row.id, ordinal, args.abbreviate);
                    let slipped = args.slip > 0.0
                        && corpus::unit(args.seed ^ 0x5119, &row.id, ordinal) < args.slip;
                    let full_keys = args.keyboard.keys(&chunk.syllables.concat());
                    let mut record = simulator.type_chunk(row, &chunk, abbreviated, slipped)?;
                    record.first_occurrence = seen.insert(full_keys.clone());
                    record.seen_in_warmup = warmup_keys.contains(&full_keys);
                    record.ambiguous = facts.ambiguous.contains(&full_keys);
                    record.common = facts.common.contains(&chunk.text);
                    if row.split == Split::Warmup {
                        warmup_chars += record.chars;
                        warmup_keys.insert(full_keys);
                    } else {
                        record.offset = measured;
                        measured += record.chars;
                        run.records.push((row.split, record));
                    }
                    clock.advance(args.timing.chunk);
                }
            }
        }
        if row.split == Split::Warmup {
            warmup_rows += 1;
        }
        // 发出消息：输入框清空，上文和上屏词链随失焦清掉，学到的东西留着。
        simulator.runtime.focus(false)?;
        simulator.runtime.focus(true)?;
        clock.advance(args.timing.turn);
    }
    let elapsed = started.elapsed();
    eprintln!(
        "typed {warmup_rows} warmup rows ({warmup_chars} chars) and {measured} measured chars in {:.1} s",
        elapsed.as_secs_f64()
    );

    let config = serde_json::json!({
        "set": args.set.file_name().map(|name| name.to_string_lossy().into_owned()),
        "set_fnv64": format!("{set_hash:016x}"),
        "resources": {
            "source_commit": verified.source_commit,
            "generation": verified.generation()?,
        },
        "keyboard": args.keyboard.name(),
        "profile": args.profile.name(),
        "learning": learning,
        "model": if args.model { "keyboard" } else { "off" },
        "warmup": type_warmup,
        "chunking": args.chunk_name,
        "abbreviate": args.abbreviate,
        "abbreviate_mode": match args.abbreviation { Abbreviation::Last => "last", Abbreviation::All => "all" },
        "seed": args.seed,
        "order_seed": args.order_seed,
        "slip": args.slip,
        "page_size": page_size,
        "phrase_preedit": phrase_preedit,
        "policy": {"strip": policy.strip, "panel_page": policy.panel_page, "panel_pages": policy.panel_pages},
        "timing_ms": {"key": args.timing.key, "select": args.timing.select, "chunk": args.timing.chunk, "literal": args.timing.literal, "turn": args.timing.turn},
        "limit_measure_chars": args.limit_measure_chars,
        "bootstrap": {"draws": args.draws, "seed": BOOTSTRAP_SEED, "unit": "doc"},
        "engine": profile::describe(&options),
    });
    let json = render(
        &run,
        config,
        (warmup_rows, warmup_chars),
        args.draws,
        &simulator.failures,
    );

    if let (Some(path), Some(dump)) = (&args.dump, &simulator.dump) {
        write_file(path, dump)?;
    }
    if let (Some(path), Some(latency)) = (&args.latency, &simulator.latency) {
        // 前 20 次派发是进程启动：词库第一次打开、模型第一次跑上文，单独列出不计入分布。
        let mut value = latency.to_json(20, 16.0);
        value["set"] = serde_json::json!(args
            .set
            .file_name()
            .map(|name| name.to_string_lossy().into_owned()));
        value["keyboard"] = serde_json::json!(args.keyboard.name());
        value["model"] = serde_json::json!(if args.model { "keyboard" } else { "off" });
        value["wall_seconds"] = serde_json::json!(elapsed.as_secs_f64());
        write_file(
            path,
            &format!("{}\n", serde_json::to_string_pretty(&value)?),
        )?;
    }
    if let Some(path) = &args.report {
        write_file(path, &format!("{json}\n"))?;
    }
    println!("{json}");
    if let Some(path) = &args.baseline {
        if args.update_baseline {
            write_file(path, &format!("{json}\n"))?;
            eprintln!("baseline written: {}", path.display());
        } else if path.exists() {
            if std::fs::read_to_string(path)?.trim() != json.trim() {
                eprintln!(
                    "typing simulation differs from {}; run with --update-baseline to accept",
                    path.display()
                );
                return Err("typing simulation baseline mismatch".into());
            }
            eprintln!("typing simulation: at baseline");
        }
    }
    Ok(())
}

fn write_file(path: &Path, text: &str) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, text)?;
    Ok(())
}

/// 打的顺序：预热按文件顺序在前；计入的部分按文档整篇搬动，`order_seed` 不为 0 时打乱文档的先后，文档内部的顺序不变。
fn stream_order(rows: &[Row], order_seed: u64) -> Vec<&Row> {
    let mut ordered: Vec<&Row> = rows
        .iter()
        .filter(|row| row.split == Split::Warmup)
        .collect();
    let mut documents: Vec<Vec<&Row>> = Vec::new();
    let mut index: HashMap<&str, usize> = HashMap::new();
    for row in rows.iter().filter(|row| row.split != Split::Warmup) {
        let at = *index.entry(row.doc.as_str()).or_insert_with(|| {
            documents.push(Vec::new());
            documents.len() - 1
        });
        documents[at].push(row);
    }
    if order_seed != 0 {
        use rand::{RngExt, SeedableRng};
        // Fisher-Yates，种子固定。
        let mut rng = rand_pcg::Pcg64::seed_from_u64(order_seed);
        for at in (1..documents.len()).rev() {
            documents.swap(at, rng.random_range(0..=at));
        }
    }
    ordered.extend(documents.into_iter().flatten());
    ordered
}

/// 整份集合上算一次的事实：哪些键串对应不止一种金标准，哪些块的文字最常见。
struct SetFacts {
    ambiguous: HashSet<String>,
    common: HashSet<String>,
}

impl SetFacts {
    fn new(rows: &[Row], chunking: Chunking, keyboard: Keyboard) -> Self {
        let mut golds: HashMap<String, BTreeSet<String>> = HashMap::new();
        let mut counts: HashMap<String, usize> = HashMap::new();
        for row in rows {
            for piece in corpus::pieces(row, chunking) {
                if let Piece::Chunk(chunk) = piece {
                    golds
                        .entry(keyboard.keys(&chunk.syllables.concat()))
                        .or_default()
                        .insert(chunk.text.clone());
                    *counts.entry(chunk.text).or_default() += 1;
                }
            }
        }
        let mut ranked: Vec<(String, usize)> = counts.into_iter().collect();
        ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        Self {
            ambiguous: golds
                .into_iter()
                .filter(|(_, texts)| texts.len() > 1)
                .map(|(keys, _)| keys)
                .collect(),
            common: ranked.into_iter().take(200).map(|(text, _)| text).collect(),
        }
    }
}

/// 一个还没打出来的音节：金标准的拼写和实际打的字母（简拼时是声母）。
#[derive(Clone)]
struct Pending {
    gold: String,
    typed: String,
}

/// 报告里列出的一次不是首选的选择（只列 dev）。
struct Failure {
    row: String,
    keys: String,
    gold: String,
    seat: usize,
    strip: Vec<String>,
}

struct Simulator<'a> {
    runtime: &'a mut Runtime<Session>,
    clock: &'a VirtualClock,
    keyboard: Keyboard,
    timing: Timing,
    policy: Policy,
    abbreviation: Abbreviation,
    dump: Option<String>,
    failures: Vec<Failure>,
    /// `--latency` 时每次派发的耗时。
    latency: Option<latency::Latency>,
    /// 这一块里已经派发过的键，`--latency` 用它标出每块的第一键和长串的尾巴。
    chunk_keys: usize,
}

impl Simulator<'_> {
    /// 派发一个动作；`--latency` 时给它计时。
    fn send(
        &mut self,
        kind: latency::Kind,
        action: Action,
    ) -> Result<msime_input_runtime::Transition, Box<dyn std::error::Error>> {
        let started = latency::Latency::start();
        let transition = self.runtime.dispatch(action)?;
        let elapsed = started.elapsed();
        let depth = self.chunk_keys;
        if matches!(kind, latency::Kind::Key | latency::Kind::Backspace) {
            self.chunk_keys += 1;
        }
        if let Some(latency) = self.latency.as_mut() {
            latency.record(
                kind,
                depth == 0 && kind == latency::Kind::Key,
                depth,
                elapsed,
            );
        }
        Ok(transition)
    }

    fn key(&mut self, byte: u8) -> Result<(), Box<dyn std::error::Error>> {
        let action = match self.keyboard {
            Keyboard::Fourteen => Action::GridKey(byte),
            Keyboard::Nine | Keyboard::Full => Action::Character {
                value: byte,
                shift: false,
            },
        };
        self.send(latency::Kind::Key, action)?;
        self.clock.advance(self.timing.key);
        Ok(())
    }

    fn type_letters(&mut self, letters: &str) -> Result<usize, Box<dyn std::error::Error>> {
        let keys = self.keyboard.keys(letters);
        for byte in keys.bytes() {
            self.key(byte)?;
        }
        Ok(keys.len())
    }

    /// 打一块：先把整块的键打完，再按 `user::decide` 一步步选，直到整块上屏或者走失败路径。
    fn type_chunk(
        &mut self,
        row: &Row,
        chunk: &Chunk,
        abbreviated: bool,
        slipped: bool,
    ) -> Result<ChunkRecord, Box<dyn std::error::Error>> {
        if !self.runtime.is_idle() {
            return Err(format!("{}: composition left over before a chunk", row.id).into());
        }
        self.chunk_keys = 0;
        let typed = corpus::typed_letters(chunk, abbreviated.then_some(self.abbreviation))
            .unwrap_or_else(|| chunk.syllables.clone());
        let mut record = ChunkRecord {
            doc: row.doc.clone(),
            chars: chunk.syllables.len(),
            keys_spelled: self.keyboard.keys(&chunk.syllables.concat()).len(),
            abbreviated: typed != chunk.syllables,
            ..ChunkRecord::default()
        };
        let mut pending: Vec<Pending> = chunk
            .syllables
            .iter()
            .zip(&typed)
            .map(|(gold, typed)| Pending {
                gold: gold.clone(),
                typed: typed.clone(),
            })
            .collect();
        let keys: String = typed.concat();
        record.keys_typed += self.type_letters(&keys)?;
        let measured = row.split != Split::Warmup;
        let mut dumped_steps: Vec<serde_json::Value> = Vec::new();

        if slipped {
            let snapshot = self.runtime.all_candidates();
            let texts: Vec<&str> = snapshot
                .candidates
                .iter()
                .map(|c| c.text.as_str())
                .collect();
            if let Some(seat) = user::slip(&texts, &chunk.text, self.policy) {
                let id = snapshot.candidates[seat].id;
                let transition =
                    self.send(latency::Kind::Select, Action::SelectAnyCandidate(id))?;
                self.clock.advance(self.timing.select);
                record.selections.push(Selection {
                    seat,
                    chars: 0,
                    panel: false,
                    committed: transition.commit.is_some(),
                    after_correction: false,
                });
                record.slipped = true;
                // 发现点错了：丢掉还在组字的部分，整块重打。点错上屏的字留在上文里，学到的也留着。
                if !self.runtime.is_idle() {
                    self.send(latency::Kind::Cancel, Action::Command(Command::Cancel))?;
                    self.chunk_keys = 0;
                }
                record.keys_typed += self.type_letters(&keys)?;
            }
        }

        let mut remaining = chunk.text.clone();
        let mut locked = 0usize;
        // 补完过简拼或者锁过拼写之后，这一块剩下的点选都不再算首选（见 metrics.rs 的口径）。
        let mut corrected = false;
        let mut first = true;
        loop {
            let snapshot = self.runtime.all_candidates();
            let texts: Vec<&str> = snapshot
                .candidates
                .iter()
                .map(|c| c.text.as_str())
                .collect();
            if first {
                record.top1_exact = texts.first() == Some(&chunk.text.as_str());
                first = false;
            }
            let abbreviation_pending = pending.iter().any(|p| p.typed != p.gold);
            // 停在声母上的用户只看候选栏，没有就把拼写打完，不去翻面板。
            let policy = if abbreviation_pending {
                Policy {
                    panel_pages: 0,
                    ..self.policy
                }
            } else {
                self.policy
            };
            let decision = user::decide(&texts, &remaining, policy);
            if self.dump.is_some() {
                let mut step = serde_json::json!({
                    "decision": format!("{decision:?}"),
                    "pending": pending.iter().map(|p| p.typed.as_str()).collect::<Vec<_>>().join("'"),
                });
                // test 部分只记位置不记文字，免得拿它逐条调参。
                if row.split != Split::Test {
                    step["strip"] =
                        serde_json::json!(texts.iter().take(self.policy.strip).collect::<Vec<_>>());
                }
                dumped_steps.push(step);
            }
            let (seat, chars, panel) = match decision {
                Decision::Strip { seat, chars } => (seat, chars, false),
                Decision::Panel { seat, chars, page } => {
                    record.panel_opens += 1;
                    record.page_turns += page;
                    (seat, chars, true)
                }
                Decision::NotFound => {
                    if abbreviation_pending {
                        let pressed = self.complete_spelling(&mut pending)?;
                        record.add_completion_keys(pressed);
                        corrected = true;
                        continue;
                    }
                    if self.keyboard.grid().is_some() && locked < pending.len() {
                        let view = self.runtime.view();
                        let target = &pending[locked].gold;
                        if let Some(index) =
                            view.nine_key_spellings.iter().position(|s| s == target)
                        {
                            self.send(
                                latency::Kind::Lock,
                                Action::ChooseNineKeySpelling(NineKeySpellingId {
                                    session: view.session,
                                    generation: view.generation,
                                    index,
                                }),
                            )?;
                            self.clock.advance(self.timing.select);
                            record.locks += 1;
                            corrected = true;
                            pending[locked].typed = pending[locked].gold.clone();
                            locked += 1;
                            continue;
                        }
                        self.fail(&mut record, Unreachable::NoSpelling, &remaining)?;
                    } else {
                        let kind = if self.keyboard.grid().is_some() {
                            Unreachable::NoSpelling
                        } else {
                            Unreachable::NotFound
                        };
                        self.fail(&mut record, kind, &remaining)?;
                    }
                    break;
                }
            };
            if seat != 0
                && measured
                && row.split == Split::Dev
                && self.failures.len() < SAMPLE_FAILURES
            {
                self.failures.push(Failure {
                    row: row.id.clone(),
                    keys: self
                        .keyboard
                        .keys(&pending.iter().map(|p| p.typed.as_str()).collect::<String>()),
                    gold: remaining.clone(),
                    seat,
                    strip: texts
                        .iter()
                        .take(self.policy.strip)
                        .map(|t| t.to_string())
                        .collect(),
                });
            }
            let id = snapshot.candidates[seat].id;
            let transition = self.send(latency::Kind::Select, Action::SelectAnyCandidate(id))?;
            self.clock.advance(self.timing.select);
            record.selections.push(Selection {
                seat,
                chars,
                panel,
                committed: transition.commit.is_some(),
                after_correction: corrected,
            });
            remaining = remaining.chars().skip(chars).collect();
            pending.drain(..chars);
            locked = locked.saturating_sub(chars);
            if remaining.is_empty() {
                if !self.runtime.is_idle() {
                    // 金标准都出来了，组字里却还剩键：所选的候选少用了键。剩下的丢掉。
                    self.fail(&mut record, Unreachable::Misaligned, "")?;
                }
                break;
            }
            let expected: String = pending
                .iter()
                .map(|p| self.keyboard.keys(&p.typed))
                .collect();
            let actual = self.runtime.view().editing_text.replace('\'', "");
            if actual != expected {
                self.fail(&mut record, Unreachable::Misaligned, &remaining)?;
                break;
            }
        }
        if let Some(dump) = self.dump.as_mut() {
            let mut line = serde_json::json!({
                "row": row.id,
                "split": row.split.name(),
                "chars": record.chars,
                "abbreviated": record.abbreviated,
                "top1_exact": record.top1_exact,
                "seats": record.selections.iter().map(|s| s.seat).collect::<Vec<_>>(),
                // 严口径计入首选的字数，两臂按块配对比较时用。
                "first_choice_chars": record.first_choice_chars(),
                "after_correction": record.selections.iter().map(|s| s.after_correction).collect::<Vec<_>>(),
                "panel_opens": record.panel_opens,
                "locks": record.locks,
                "completion_keys": record.completion_keys,
                "unreachable": record.unreachable.map(Unreachable::name),
                "steps": dumped_steps,
            });
            if row.split != Split::Test {
                line["gold"] = serde_json::json!(chunk.text);
                line["keys"] = serde_json::json!(self.keyboard.keys(&keys));
            }
            dump.push_str(&line.to_string());
            dump.push('\n');
        }
        Ok(record)
    }

    /// 简拼没打出要的词：把停在声母上的音节打全。只有末音节简拼时接着往后打；每个音节都简拼时退格删掉还没选的键，整段重打。返回多按的键数（退格也算）。
    fn complete_spelling(
        &mut self,
        pending: &mut [Pending],
    ) -> Result<usize, Box<dyn std::error::Error>> {
        let mut pressed = 0;
        let only_last = pending[..pending.len() - 1]
            .iter()
            .all(|p| p.typed == p.gold);
        if only_last {
            let last = pending.last_mut().expect("a pending syllable");
            let rest = last.gold[last.typed.len()..].to_owned();
            pressed += self.type_letters(&rest)?;
            last.typed = last.gold.clone();
            return Ok(pressed);
        }
        let typed: String = pending
            .iter()
            .map(|p| self.keyboard.keys(&p.typed))
            .collect();
        for _ in 0..typed.len() {
            self.send(
                latency::Kind::Backspace,
                Action::Command(Command::Backspace),
            )?;
            self.clock.advance(self.timing.key);
            pressed += 1;
        }
        for p in pending.iter_mut() {
            p.typed = p.gold.clone();
        }
        let full: String = pending.iter().map(|p| p.gold.as_str()).collect();
        pressed += self.type_letters(&full)?;
        Ok(pressed)
    }

    /// 金标准打不出来：丢掉组字（暂存的那段一起），把这块该有的文字当作用别的办法打出来，写进上文，接着打下一块。
    fn fail(
        &mut self,
        record: &mut ChunkRecord,
        kind: Unreachable,
        remaining: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        record.unreachable = Some(kind);
        record.unreachable_chars = remaining.chars().count();
        let held = self.runtime.view().phrase_prefix;
        if !self.runtime.is_idle() {
            self.send(latency::Kind::Cancel, Action::Command(Command::Cancel))?;
        }
        self.runtime.seed_context(&format!("{held}{remaining}"));
        Ok(())
    }
}

/// 列出词表读音和引擎自己的读音（`hanzi_to_pinyin`）不一致的词，供人工核对；不改文件。
fn lint_readings(options: &EngineOptions, rows: &[Row]) -> Result<(), Box<dyn std::error::Error>> {
    let mut disagreements = 0;
    let mut words = 0;
    println!("id\ttoken\tword\tfrozen\tengine");
    for row in rows {
        for (index, token) in row.tokens.iter().enumerate() {
            let corpus::Token::Word(word) = token else {
                continue;
            };
            words += 1;
            let frozen = word.syllables.join("'");
            let engine = msime_engine::host::hanzi_to_pinyin(options, &word.text);
            if engine.replace('\'', "") != frozen.replace('\'', "") {
                disagreements += 1;
                println!("{}\t{index}\t{}\t{frozen}\t{engine}", row.id, word.text);
            }
        }
    }
    eprintln!("{disagreements} of {words} words read differently by the engine");
    Ok(())
}

fn render(
    run: &Run,
    config: serde_json::Value,
    (warmup_rows, warmup_chars): (usize, usize),
    draws: usize,
    failures: &[Failure],
) -> String {
    let with_intervals = |split: Option<Split>| {
        let totals = run.totals(split);
        let mut value = totals.to_json();
        for (name, metric) in [
            ("cfa", Totals::cfa as fn(&Totals) -> f64),
            ("cfa_lenient", Totals::cfa_lenient),
            ("cca", Totals::cca),
            ("cpr", Totals::cpr),
            ("zcr", Totals::zcr),
        ] {
            let (low, high) = run.interval(split, metric, draws, BOOTSTRAP_SEED);
            value[format!("{name}_ci95")] = serde_json::json!([low, high]);
        }
        value
    };
    let buckets = |key: fn(&ChunkRecord) -> String| {
        run.buckets(None, key)
            .into_iter()
            .map(|(name, totals)| (name, totals.to_json()))
            .collect::<BTreeMap<_, _>>()
    };
    let measured = run.totals(None);
    let seen_in_warmup = run
        .records
        .iter()
        .filter(|(_, record)| record.seen_in_warmup)
        .count();
    let value = serde_json::json!({
        "tool": "typing_sim",
        "config": config,
        "warmup": {"rows": warmup_rows, "chars": warmup_chars},
        "diagnostics": {
            "measured_chunks": measured.chunks,
            "chunks_seen_in_warmup": metrics::round(metrics::ratio(seen_in_warmup, measured.chunks)),
        },
        "overall": {
            "measure": with_intervals(None),
            "dev": with_intervals(Some(Split::Dev)),
            "test": with_intervals(Some(Split::Test)),
        },
        "by_chunk_chars": buckets(|r| metrics::length_band(r.chars).to_owned()),
        "by_abbreviated": buckets(|r| r.abbreviated.to_string()),
        "by_first_occurrence": buckets(|r| r.first_occurrence.to_string()),
        "by_seen_in_warmup": buckets(|r| r.seen_in_warmup.to_string()),
        "by_ambiguous_keys": buckets(|r| r.ambiguous.to_string()),
        "by_common_chunk": buckets(|r| r.common.to_string()),
        "learning_curve": run
            .buckets(None, |r| format!("{:06}", r.offset / CURVE_BAND * CURVE_BAND))
            .into_iter()
            .map(|(band, totals)| serde_json::json!({"from_char": band.parse::<usize>().unwrap_or(0), "chars": totals.chars, "cfa": metrics::round(totals.cfa()), "cfa_lenient": metrics::round(totals.cfa_lenient()), "cca": metrics::round(totals.cca())}))
            .collect::<Vec<_>>(),
        "sample_failures": failures.iter().map(|f| serde_json::json!({
            "row": f.row, "keys": f.keys, "gold": f.gold, "seat": f.seat, "strip": f.strip,
        })).collect::<Vec<_>>(),
    });
    serde_json::to_string_pretty(&value).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timing_overrides_only_the_named_actions() {
        let timing = Timing::parse("key=250,turn=4000").unwrap();
        assert_eq!((timing.key, timing.select, timing.turn), (250, 700, 4000));
        assert!(Timing::parse("key").is_err());
        assert!(Timing::parse("blink=3").is_err());
    }

    #[test]
    fn the_virtual_clock_moves_only_when_told() {
        let clock = VirtualClock::new();
        let reading = clock.clock();
        let before = (reading.steady)();
        assert_eq!((reading.steady)(), before);
        clock.advance(10_500);
        assert_eq!((reading.steady)() - before, Duration::from_millis(10_500));
        assert_eq!((reading.local)().hour, 10);
    }

    #[test]
    fn warmup_stays_first_and_documents_move_whole() {
        let line = |id: &str, doc: &str, split: &str| format!("{id}\t{doc}\t{split}\t-\t我=wo");
        let text = [
            line("w1", "w", "warmup"),
            line("a1", "a", "test"),
            line("b1", "b", "dev"),
            line("a2", "a", "test"),
            line("c1", "c", "test"),
        ]
        .join("\n");
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("set.tsv");
        std::fs::write(&path, text).unwrap();
        let (rows, _) = corpus::load(&path).unwrap();
        let ids = |order: Vec<&Row>| {
            order
                .into_iter()
                .map(|r| r.id.as_str())
                .collect::<Vec<_>>()
                .join(",")
        };
        assert_eq!(ids(stream_order(&rows, 0)), "w1,a1,a2,b1,c1");
        for seed in 1..20 {
            let order = ids(stream_order(&rows, seed));
            assert!(order.starts_with("w1,"));
            let a1 = order.find("a1").unwrap();
            assert_eq!(&order[a1..a1 + 5], "a1,a2");
        }
    }
}
