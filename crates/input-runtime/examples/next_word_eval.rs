//! 上屏后联想（next-word prediction）的离线评测：词库续写按模型排序之后，用户接下来真正写的那一段是不是更靠前，以及每次上屏要多花多少时间。
//!
//! 用例来自 Tatoeba 的中文句子（`cmn_sentences_detailed.tsv`，CC-BY 2.0 FR，不在 chinese-ime-lm 的训练语料里；语料不进仓库，用 `--corpus` 给路径）。每句取开头连续的汉字，按词库做正向最长匹配切词，随机挑一个不在句末的词：它前面的文字是上文，它本身是这次上屏的词，它后面的文字是答案。联想的一行是答案的前缀就算命中。
//!
//! 每条用例在每一臂里都走真实的运行时和引擎：清掉上文再放进这条的上文（`seed_context`），打这个词的拼音，选中文字等于它的那一行上屏，读上屏后显示的联想行。「今天」一臂不挂打分器，就是不排序的联想；其余各臂挂一个打分器（随包的 `model-v1`，以及每个 `--onnx-model`），引擎因此多取 32 条词库续写、排序后显示 8 条。产品里只有报 `scores_prediction_pool_within_a_frame` 的打分器（ONNX 后端）才排联想，随包模型不排；这里给每一臂都套上 [`Pooled`]，量的是排了会怎样，随包模型那一臂的延迟就是它在产品里不排的理由。`--unranked` 再给每个打分器加一臂「挂着但不排联想」，就是产品里随包模型的样子。排序规则（总对数概率、32 条候选池、个人习惯行在前）在看评测数字之前就定了，这里不调任何参数。
//!
//! 计时的是上屏那一次 `dispatch`（引擎选词、查续写、打分、取快照），以及上屏之后接着打答案里第一个词的读音（逐键重排读的是同一段新上文），各臂轮流先跑，配对的差就是打分器多花的时间。上屏时排联想要为新的上文编码前缀，不排时这一份落在之后第一次真正调到模型的按键上（首行是可信的词典命中时按键不调模型），所以上屏的差是上屏那一帧的代价；「上屏加下一个词」与 `--unranked` 那一臂的差才是排联想真正新增的代价。不学习（`learning = false`），所以没有个人习惯行，量的只是词库续写的排序。
//!
//! 用法：`next_word_eval --resources <verified-dir> --corpus cmn_sentences_detailed.tsv [--cases 3000] [--longest 2] [--seed 7] [--warmup 20] [--no-bundled] [--onnx-model <dir>]... [--onnx-threads 2] [--unranked] [--dump cases.jsonl]`
mod eval_resources;

use msime_engine::host::{Command, Session};
use msime_input_runtime::{
    Action, CandidateFacts, CandidateScorer, Reranker, Runtime, SentenceModel,
};
use rand::seq::SliceRandom;
use rand::{RngExt, SeedableRng};
use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

struct Case {
    context: String,
    word: String,
    pinyin: String,
    gold: String,
    /// 答案里第一个词的读音：上屏之后接着打它，量逐键重排在新上文上的那几键。
    next_pinyin: String,
}

fn is_han(character: char) -> bool {
    matches!(character as u32, 0x4E00..=0x9FFF | 0x3400..=0x4DBF | 0x20000..=0x2FA1F)
}

fn all_han(text: &str) -> bool {
    !text.is_empty() && text.chars().all(is_han)
}

/// 一到四个字的词条：词 -> 权重最高的那个读音。与可行性评测的 Python 脚本同一个口径。
fn readings(database: &Path) -> Result<HashMap<String, (String, i64)>, Box<dyn std::error::Error>> {
    let connection = rusqlite::Connection::open_with_flags(
        database,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )?;
    let mut tables: Vec<String> = connection
        .prepare("SELECT name FROM sqlite_master WHERE type='table'")?
        .query_map([], |row| row.get(0))?
        .collect::<Result<_, _>>()?;
    tables.sort();
    let mut readings: HashMap<String, (String, i64)> = HashMap::new();
    for size in 1..=4usize {
        let prefix = format!("tbl_{size}_");
        for table in tables.iter().filter(|name| name.starts_with(&prefix)) {
            let mut statement = connection.prepare(&format!(
                "SELECT key, value, MAX(weight) FROM \"{table}\" GROUP BY value"
            ))?;
            let rows = statement.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            })?;
            for row in rows {
                let (key, value, weight) = row?;
                if !all_han(&value) || value.chars().count() != size {
                    continue;
                }
                match readings.get(&value) {
                    Some((_, old)) if *old >= weight => {}
                    _ => {
                        readings.insert(value, (key, weight));
                    }
                }
            }
        }
    }
    Ok(readings)
}

/// 正向最长匹配；有切不出来的字就整句不要。
fn segment(
    text: &[char],
    readings: &HashMap<String, (String, i64)>,
    longest: usize,
) -> Option<Vec<(usize, String)>> {
    let mut words = Vec::new();
    let mut start = 0;
    while start < text.len() {
        let found = (1..=longest.min(text.len() - start))
            .rev()
            .find_map(|length| {
                let word: String = text[start..start + length].iter().collect();
                readings.contains_key(&word).then_some(word)
            })?;
        let length = found.chars().count();
        words.push((start, found));
        start += length;
    }
    Some(words)
}

fn build_cases(
    corpus: &Path,
    readings: &HashMap<String, (String, i64)>,
    count: usize,
    seed: u64,
    longest: usize,
) -> Result<Vec<Case>, Box<dyn std::error::Error>> {
    let mut sentences = Vec::new();
    for line in std::io::BufReader::new(std::fs::File::open(corpus)?).lines() {
        let line = line?;
        let mut fields = line.split('\t');
        let (_, language, text) = (fields.next(), fields.next(), fields.next());
        if language == Some("cmn") {
            if let Some(text) = text {
                sentences.push(text.to_owned());
            }
        }
    }
    let mut random = rand_pcg::Pcg64::seed_from_u64(seed);
    sentences.shuffle(&mut random);
    let mut cases = Vec::new();
    for sentence in sentences {
        // 只要开头那一段连续的汉字：标点结束这一分句。
        let run: Vec<char> = sentence.chars().take_while(|c| is_han(*c)).collect();
        if run.len() < 4 {
            continue;
        }
        let Some(words) = segment(&run, readings, longest) else {
            continue;
        };
        let bounds: Vec<&(usize, String)> = words
            .iter()
            .filter(|(start, word)| start + word.chars().count() < run.len())
            .collect();
        if bounds.is_empty() {
            continue;
        }
        let (start, word) = bounds[random.random_range(0..bounds.len())];
        let end = start + word.chars().count();
        // 切词首尾相接，`end` 不在句末，所以总有一个词从这里开始。
        let next = words
            .iter()
            .find(|(next, _)| *next == end)
            .map_or("", |(_, next)| next.as_str());
        cases.push(Case {
            context: run[..*start].iter().collect(),
            word: word.clone(),
            pinyin: readings[word].0.clone(),
            gold: run[end..].iter().collect(),
            next_pinyin: readings
                .get(next)
                .map_or_else(String::new, |(key, _)| key.clone()),
        });
        if cases.len() == count {
            break;
        }
    }
    Ok(cases)
}

#[cfg(feature = "onnx-rerank")]
fn onnx_scorer(
    directory: &Path,
    threads: usize,
) -> Result<Box<dyn CandidateScorer>, Box<dyn std::error::Error>> {
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
fn onnx_scorer(_: &Path, _: usize) -> Result<Box<dyn CandidateScorer>, Box<dyn std::error::Error>> {
    Err("--onnx-model needs the onnx-rerank feature (and ORT_LIB_LOCATION pointing at a minimal ONNX Runtime build)".into())
}

/// 不论打分器自己报不报 `scores_prediction_pool_within_a_frame`，都按第二个字段决定 runtime 排不排联想；其余全部照转，逐键重排与不套时相同。
struct Pooled(Box<dyn CandidateScorer>, bool);

impl CandidateScorer for Pooled {
    fn context_length(&self) -> usize {
        self.0.context_length()
    }

    fn log_probabilities(&mut self, context: &str, candidates: &[&str]) -> Vec<f32> {
        self.0.log_probabilities(context, candidates)
    }

    fn scores_prediction_pool_within_a_frame(&self) -> bool {
        self.1
    }

    fn best_where(
        &mut self,
        context: &str,
        texts: &[&str],
        facts: &dyn Fn(usize) -> CandidateFacts,
    ) -> Option<usize> {
        self.0.best_where(context, texts, facts)
    }
}

/// 一臂的名字、打分器（「今天」那一臂没有）、排不排联想。
type PlannedArm = (String, Option<Box<dyn CandidateScorer>>, bool);

struct Arm {
    label: String,
    runtime: Runtime<Session>,
    _state: tempfile::TempDir,
    /// 每条用例上屏后显示的联想行；没走到上屏的用例为 `None`。
    rows: Vec<Option<Vec<String>>>,
    /// 上屏那一次 `dispatch` 和接着打下一个词的读音的耗时，按用例下标，预热的用例不计。
    timings: Vec<Option<(Duration, Duration)>>,
}

/// 上屏后显示的联想行，上屏那一次 `dispatch` 和接着打下一个词的读音的耗时。
type Committed = (Vec<String>, Duration, Duration);

/// 打这条用例的拼音，选中文字等于这个词的那一行上屏，再打下一个词的读音；返回上屏后的联想行和两段的耗时。找不到这一行或上屏的不是这个词时为 `None`。
fn run_case(
    runtime: &mut Runtime<Session>,
    case: &Case,
) -> Result<Option<Committed>, Box<dyn std::error::Error>> {
    runtime.dispatch(Action::Command(Command::Cancel))?;
    runtime.clear_context();
    runtime.seed_context(&case.context);
    for byte in case.pinyin.bytes() {
        runtime.dispatch(Action::Character {
            value: byte,
            shift: false,
        })?;
    }
    let snapshot = runtime.all_candidates();
    let Some(id) = snapshot
        .candidates
        .iter()
        .find(|row| row.text == case.word)
        .map(|row| row.id)
    else {
        return Ok(None);
    };
    let started = Instant::now();
    let transition = runtime.dispatch(Action::SelectAnyCandidate(id))?;
    let elapsed = started.elapsed();
    if transition.commit.as_deref() != Some(case.word.as_str()) {
        return Ok(None);
    }
    let rows = if transition.view.prediction {
        transition
            .view
            .candidates
            .iter()
            .map(|row| row.text.clone())
            .collect()
    } else {
        Vec::new()
    };
    let started = Instant::now();
    for byte in case.next_pinyin.bytes() {
        runtime.dispatch(Action::Character {
            value: byte,
            shift: false,
        })?;
    }
    let next = started.elapsed();
    Ok(Some((rows, elapsed, next)))
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    let rank = (p / 100.0 * sorted.len() as f64).ceil().max(1.0) as usize;
    sorted[rank.min(sorted.len()) - 1]
}

/// 互补误差函数，Numerical Recipes 的 `erfcc`，相对误差小于 1.2e-7，尾部也成立。
fn erfc(x: f64) -> f64 {
    let z = x.abs();
    let t = 1.0 / (1.0 + 0.5 * z);
    let r = t
        * (-z * z - 1.265_512_23
            + t * (1.000_023_68
                + t * (0.374_091_96
                    + t * (0.096_784_18
                        + t * (-0.186_288_06
                            + t * (0.278_868_07
                                + t * (-1.135_203_98
                                    + t * (1.488_515_87
                                        + t * (-0.822_152_23 + t * 0.170_872_77)))))))))
            .exp();
    if x >= 0.0 {
        r
    } else {
        2.0 - r
    }
}

/// McNemar 检验（带连续性校正）：`better` 是只有新臂命中的用例数，`worse` 是只有今天命中的。
fn mcnemar(better: usize, worse: usize) -> f64 {
    let discordant = (better + worse) as f64;
    if discordant == 0.0 {
        return 1.0;
    }
    let statistic = ((better as f64 - worse as f64).abs() - 1.0)
        .max(0.0)
        .powi(2)
        / discordant;
    erfc((statistic / 2.0).sqrt())
}

/// 这一臂在第 `case` 条用例上屏后显示的联想行；没走到上屏时为空。
fn rows(arm: &Arm, case: usize) -> &[String] {
    arm.rows[case].as_deref().unwrap_or_default()
}

fn hits(rows: &[String], gold: &str, top: usize) -> bool {
    rows.iter()
        .take(top)
        .any(|row| gold.starts_with(row.as_str()))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut resources: Option<PathBuf> = None;
    let mut resource_lock: Option<PathBuf> = None;
    let mut corpus: Option<PathBuf> = None;
    let mut count = 3000usize;
    let mut longest = 2usize;
    let mut seed = 7u64;
    let mut warmup = 20usize;
    let mut bundled = true;
    let mut onnx_models: Vec<PathBuf> = Vec::new();
    let mut onnx_threads = 2usize;
    let mut unranked = false;
    let mut dump: Option<PathBuf> = None;
    let mut index = 0;
    let take = |index: &mut usize| -> Result<String, Box<dyn std::error::Error>> {
        *index += 1;
        args.get(*index)
            .cloned()
            .ok_or_else(|| format!("{} needs a value", args[*index - 1]).into())
    };
    while index < args.len() {
        match args[index].as_str() {
            "--resources" => resources = Some(PathBuf::from(take(&mut index)?)),
            "--resource-lock" => resource_lock = Some(PathBuf::from(take(&mut index)?)),
            "--corpus" => corpus = Some(PathBuf::from(take(&mut index)?)),
            "--cases" => count = take(&mut index)?.parse()?,
            "--longest" => longest = take(&mut index)?.parse()?,
            "--seed" => seed = take(&mut index)?.parse()?,
            "--warmup" => warmup = take(&mut index)?.parse()?,
            "--no-bundled" => bundled = false,
            "--onnx-model" => onnx_models.push(PathBuf::from(take(&mut index)?)),
            "--onnx-threads" => onnx_threads = take(&mut index)?.parse()?,
            "--unranked" => unranked = true,
            "--dump" => dump = Some(PathBuf::from(take(&mut index)?)),
            other => return Err(format!("unknown argument {other}").into()),
        }
        index += 1;
    }
    let usage = "usage: next_word_eval --resources <verified-dir> --corpus cmn_sentences_detailed.tsv [--resource-lock <file.json>] [--cases 3000] [--longest 2] [--seed 7] [--warmup 20] [--no-bundled] [--onnx-model <dir>]... [--onnx-threads 2] [--unranked] [--dump cases.jsonl]";
    let (Some(resources), Some(corpus)) = (resources, corpus) else {
        return Err(usage.into());
    };
    if !(1..=4).contains(&longest) {
        return Err("--longest is 1 to 4 characters".into());
    }
    let verified = eval_resources::verify(&resources, resource_lock.as_deref())?;
    let readings = readings(&resources.join("msime-pinyin.db"))?;
    let cases = build_cases(&corpus, &readings, count, seed, longest)?;
    eprintln!(
        "{} cases (longest commit {longest}, seed {seed})",
        cases.len()
    );

    // 每一臂有自己的运行时，打分器的前缀缓存不共用，所以不排的那一臂另加载一份模型。
    let mut scorers: Vec<PlannedArm> = vec![("today".into(), None, false)];
    let rankings: &[bool] = if unranked { &[true, false] } else { &[true] };
    let named = |label: &str, ranks: bool| {
        if ranks {
            label.to_owned()
        } else {
            format!("{label} unranked")
        }
    };
    if bundled {
        let bytes = std::fs::read(resources.join("sentence-model.safetensors"))?;
        let model = std::sync::Arc::new(SentenceModel::load(&bytes)?);
        for &ranks in rankings {
            let scorer: Box<dyn CandidateScorer> = Box::new(Reranker::new(model.clone()));
            scorers.push((named("model-v1", ranks), Some(scorer), ranks));
        }
    }
    for directory in &onnx_models {
        let label = directory.file_name().map_or_else(
            || directory.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        );
        for &ranks in rankings {
            let scorer = onnx_scorer(directory, onnx_threads)?;
            scorers.push((named(&label, ranks), Some(scorer), ranks));
        }
    }
    let mut arms = Vec::new();
    for (label, scorer, ranks) in scorers {
        let state = tempfile::tempdir()?;
        let path = |name: &str| -> Result<String, Box<dyn std::error::Error>> {
            Ok(state
                .path()
                .join(name)
                .to_str()
                .ok_or("non-UTF-8 state")?
                .to_owned())
        };
        let mut options = msime_engine::host::prepare_options(
            resources.to_str().ok_or("non-UTF-8 resource path")?,
            &path("user")?,
            &path("cache")?,
            &verified.source_commit,
        )?;
        options.scheme = 0;
        options.learning = false;
        options.frequency_mode = "disabled".into();
        options.next_word_prediction = true;
        let mut runtime = Runtime::new(Session::new(&options)?, 9)?;
        runtime.focus(true)?;
        runtime.set_candidate_scorer(
            scorer.map(|scorer| Box::new(Pooled(scorer, ranks)) as Box<dyn CandidateScorer>),
        );
        arms.push(Arm {
            label,
            runtime,
            _state: state,
            rows: Vec::with_capacity(cases.len()),
            timings: Vec::with_capacity(cases.len()),
        });
    }

    for (number, case) in cases.iter().enumerate() {
        // 各臂轮流先跑，冷缓存的代价不总落在同一臂上。
        let first = number % arms.len();
        let order: Vec<usize> = (0..arms.len()).map(|k| (first + k) % arms.len()).collect();
        for &arm in &order {
            let outcome = run_case(&mut arms[arm].runtime, case)?;
            let arm = &mut arms[arm];
            match outcome {
                Some((rows, elapsed, next)) => {
                    arm.rows.push(Some(rows));
                    arm.timings
                        .push((number >= warmup).then_some((elapsed, next)));
                }
                None => {
                    arm.rows.push(None);
                    arm.timings.push(None);
                }
            }
        }
        if (number + 1) % 500 == 0 {
            eprintln!("{} / {}", number + 1, cases.len());
        }
    }

    // 只算每一臂都走到上屏的用例：各臂的候选集合相同（逐键重排只换顺序），按理全部一致。
    let counted: Vec<usize> = (0..cases.len())
        .filter(|&case| arms.iter().all(|arm| arm.rows[case].is_some()))
        .collect();
    let total = counted.len();
    if total == 0 {
        return Err("no case reached a commit".into());
    }
    let today = &arms[0];
    let suggested = counted
        .iter()
        .filter(|&&case| !rows(today, case).is_empty())
        .count();
    println!(
        "cases {} built, {total} committed in every arm; today shows a suggestion for {suggested} ({:.1}%)",
        cases.len(),
        100.0 * suggested as f64 / total as f64
    );
    println!("hit@k: one of the first k rows is a prefix of what the writer wrote next, over all {total} committed cases; chars@1: characters of a correct first row, per case.");
    println!(
        "\n{:<16} {:>7} {:>7} {:>7} {:>8}   McNemar p vs today (hit@1 / hit@3 / hit@8)",
        "arm", "hit@1", "hit@3", "hit@8", "chars@1"
    );
    for arm in &arms {
        let rate = |top: usize| {
            counted
                .iter()
                .filter(|&&case| hits(rows(arm, case), &cases[case].gold, top))
                .count()
        };
        let chars: usize = counted
            .iter()
            .filter(|&&case| hits(rows(arm, case), &cases[case].gold, 1))
            .map(|&case| rows(arm, case)[0].chars().count())
            .sum();
        let test = |top: usize| {
            let (mut better, mut worse) = (0, 0);
            for &case in &counted {
                let gold = &cases[case].gold;
                match (
                    hits(rows(arm, case), gold, top),
                    hits(rows(today, case), gold, top),
                ) {
                    (true, false) => better += 1,
                    (false, true) => worse += 1,
                    _ => {}
                }
            }
            format!("{:.1e} (+{better}/-{worse})", mcnemar(better, worse))
        };
        let pct = |n: usize| 100.0 * n as f64 / total as f64;
        let tests = if std::ptr::eq(arm, today) {
            String::new()
        } else {
            format!("{} / {} / {}", test(1), test(3), test(8))
        };
        println!(
            "{:<16} {:>6.1}% {:>6.1}% {:>6.1}% {:>8.3}   {tests}",
            arm.label,
            pct(rate(1)),
            pct(rate(3)),
            pct(rate(8)),
            chars as f64 / total as f64
        );
    }

    let report = |title: &str, measure: &dyn Fn((Duration, Duration)) -> Duration| {
        println!("\n{title}, ms (cases after the first {warmup}; paired delta = arm minus today on the same case)");
        let millis = |arm: &Arm, case: usize| {
            arm.timings[case].map(|timing| measure(timing).as_secs_f64() * 1000.0)
        };
        for arm in &arms {
            let mut own = Vec::new();
            let mut delta = Vec::new();
            for &case in &counted {
                let (Some(mine), Some(base)) = (millis(arm, case), millis(today, case)) else {
                    continue;
                };
                own.push(mine);
                delta.push(mine - base);
            }
            own.sort_by(f64::total_cmp);
            delta.sort_by(f64::total_cmp);
            let own = format!(
                "  {:<20} p50 {:>7.2}  p95 {:>7.2}  max {:>7.2}",
                arm.label,
                percentile(&own, 50.0),
                percentile(&own, 95.0),
                percentile(&own, 100.0)
            );
            if std::ptr::eq(arm, today) {
                println!("{own}");
            } else {
                println!(
                    "{own}   delta p50 {:>7.2}  p95 {:>7.2}",
                    percentile(&delta, 50.0),
                    percentile(&delta, 95.0)
                );
            }
        }
    };
    report("commit dispatch", &|(commit, _)| commit);
    report("typing the next word's reading after the commit", &|(
        _,
        next,
    )| {
        next
    });
    report("commit plus the next word's reading", &|(commit, next)| {
        commit + next
    });

    if let Some(path) = dump {
        let mut file = std::io::BufWriter::new(std::fs::File::create(path)?);
        for &case in &counted {
            let item = &cases[case];
            let mut arms_json = serde_json::Map::new();
            for arm in &arms {
                arms_json.insert(arm.label.clone(), serde_json::json!(rows(arm, case)));
            }
            serde_json::to_writer(
                &mut file,
                &serde_json::json!({
                    "context": item.context,
                    "word": item.word,
                    "pinyin": item.pinyin,
                    "gold": item.gold,
                    "rows": arms_json,
                }),
            )?;
            file.write_all(b"\n")?;
        }
    }
    Ok(())
}
