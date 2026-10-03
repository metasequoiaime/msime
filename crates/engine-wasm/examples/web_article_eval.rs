//! 网页内置输入法在 TapTapGo 跟打文章上的转换质量：逐个分句打拼音，看整句排第几、玩家要选几次。
//!
//! 这是 `article_eval`（web-dict 评测分支里驱动 `Runtime` 的那一份）的移植，改成驱动 `WebHost`，资源也换成网页实际发布的那几个文件：目录里只有裁剪后的拼音库（`msime-dict-build web` 输出的 `msime-pinyin.db`，改名为 `msime.db`）和 `sentence-model.safetensors`，没有 english.db、others.db 和 n-gram。每个分句前先 `reset`，再用 `seed_context_for_eval` 放进文章里它前面的文字，和 article_eval 一样。
//!
//! 分句来自 TapTapGo 的文章数据（`extract_articles.py` 生成），不在本仓库里，所以这是发版前在本地跑的检查，不是 CI 门禁：门槛是 top-1 不低于 40.0%，发版的人把输出的那一行贴进发布说明。`resources/eval/baseline-web-articles.json` 只记录汇总数字和两个输入文件的 sha256，不含任何分句文字。
//!
//! 玩家模型和 article_eval 相同：先看第一页，选其中最长的、是剩余原文前缀的候选；第一页没有就往后翻页找第一个；哪里都没有就算卡住。
//!
//! usage: web_article_eval --resources <dir> --set <clauses.tsv> [--no-model] [--no-context] [--baseline <file.json> [--update-baseline]]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use msime_engine_wasm::host::{Frame, Key, Out, Scheme, WebHost};

/// 一页的候选数，和网页一样。
const PAGE: usize = 9;
/// 一个分句最多选几次，防止异常情况下死循环。
const MAX_STEPS: usize = 64;
/// 发版门槛：整句排在首位的分句占比（%）。
const TOP1_FLOOR: f64 = 40.0;

struct Case {
    input: String,
    gold: String,
    context: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut resources: Option<PathBuf> = None;
    let mut set: Option<PathBuf> = None;
    let mut model = true;
    let mut context = true;
    let mut baseline: Option<PathBuf> = None;
    let mut update = false;
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut index = 0;
    while index < args.len() {
        let flag = args[index].clone();
        let mut value = || -> Result<String, String> {
            index += 1;
            args.get(index)
                .cloned()
                .ok_or_else(|| format!("{flag} needs a value"))
        };
        match flag.as_str() {
            "--resources" => resources = Some(PathBuf::from(value()?)),
            "--set" => set = Some(PathBuf::from(value()?)),
            "--baseline" => baseline = Some(PathBuf::from(value()?)),
            "--update-baseline" => update = true,
            "--no-model" => model = false,
            "--no-context" => context = false,
            other => return Err(format!("unknown option: {other}").into()),
        }
        index += 1;
    }
    let resources = resources.ok_or("--resources is required")?;
    let set = set.ok_or("--set is required")?;
    let cases = load(&set)?;
    if cases.is_empty() {
        return Err(format!("{}: no clauses", set.display()).into());
    }

    // 只把网页会发布的主库放进会话目录；资源目录本身只读。
    let database = resources.join("msime.db");
    let dictionaries = tempfile::tempdir()?;
    std::fs::copy(&database, dictionaries.path().join("msime.db"))?;
    let user = tempfile::tempdir()?;
    let cache = tempfile::tempdir()?;
    let model_bytes = if model {
        Some(std::fs::read(resources.join("sentence-model.safetensors"))?)
    } else {
        None
    };
    let started = Instant::now();
    let mut host = WebHost::new_with_paths(
        Scheme::Quanpin,
        PAGE,
        model_bytes.as_deref(),
        dictionaries.path(),
        user.path(),
        cache.path(),
    )?;
    // 测的是排序质量，不能让机器负载触发熔断改变结果。
    host.disable_slow_frame_breaker_for_eval();
    let setup = started.elapsed();

    let mut top1 = 0usize;
    let mut top5 = 0usize;
    let mut top9 = 0usize;
    let mut found = 0usize;
    let mut steps_total = 0usize;
    let mut pages_total = 0usize;
    let mut stuck = 0usize;
    let mut key_times: Vec<Duration> = Vec::new();
    let mut model_on = model;

    for case in &cases {
        host.reset();
        if context {
            host.seed_context_for_eval(&case.context);
        }
        let mut frame = None;
        for key in typed(&case.input) {
            let key_start = Instant::now();
            frame = Some(host.keys(&[key]));
            key_times.push(key_start.elapsed());
        }
        let Some(first) = frame else {
            continue;
        };
        model_on &= first.model_on || !model;

        // 整个候选列表：一页页往后翻到底（翻页会展开被截留的候选），再翻回第一页。
        let mut all: Vec<String> = Vec::new();
        let mut current = first;
        loop {
            all.extend(current.page.iter().map(|row| row.text.clone()));
            let next = host.keys(&[Key::PageNext]);
            if next.page_index == current.page_index {
                break;
            }
            current = next;
        }
        while current.page_index > 0 {
            current = host.keys(&[Key::PagePrev]);
        }
        if let Some(rank) = all.iter().position(|text| *text == case.gold) {
            found += 1;
            top1 += usize::from(rank == 0);
            top5 += usize::from(rank < 5);
            top9 += usize::from(rank < PAGE);
        }

        match play(&mut host, current, &case.gold) {
            Some((steps, pages)) => {
                steps_total += steps;
                pages_total += pages;
            }
            None => stuck += 1,
        }
    }

    let clauses = cases.len();
    let finished = clauses - stuck;
    let rate = |count: usize| (count as f64 / clauses as f64 * 1000.0).round() / 10.0;
    let per_clause = |total: usize, scale: f64| {
        if finished == 0 {
            0.0
        } else {
            (total as f64 / finished as f64 * scale).round() / scale
        }
    };
    let ms = |duration: Duration| duration.as_secs_f64() * 1000.0;
    key_times.sort();
    let percentile = |p: f64| {
        key_times
            .get(((key_times.len() as f64 - 1.0) * p).round() as usize)
            .copied()
            .unwrap_or_default()
    };
    let key_mean = if key_times.is_empty() {
        0.0
    } else {
        key_times.iter().map(|duration| ms(*duration)).sum::<f64>() / key_times.len() as f64
    };

    // 记入基线的只有确定的汇总数字和两个输入的指纹，不含分句文字，也不含随机器变化的耗时。
    let recorded = format!(
        "{{\n  \"clauses\": {clauses},\n  \"top1_pct\": {},\n  \"top5_pct\": {},\n  \"top9_pct\": {},\n  \"anywhere_pct\": {},\n  \"picks_per_clause\": {},\n  \"page_turns_per_clause\": {},\n  \"stuck\": {stuck},\n  \"model\": {model},\n  \"context\": {context},\n  \"clauses_sha256\": \"{}\",\n  \"db_sha256\": \"{}\"\n}}",
        rate(top1),
        rate(top5),
        rate(top9),
        rate(found),
        per_clause(steps_total, 100.0),
        per_clause(pages_total, 1000.0),
        sha256(&set)?,
        sha256(&database)?,
    );
    println!("{recorded}");
    eprintln!(
        "setup {:.1} ms; per key mean {:.3} ms, p95 {:.3} ms, max {:.2} ms over {} keys",
        ms(setup),
        key_mean,
        ms(percentile(0.95)),
        ms(key_times.last().copied().unwrap_or_default()),
        key_times.len(),
    );
    if model && !model_on {
        return Err("the sentence model was off for some clauses".into());
    }

    if let Some(path) = baseline {
        if update {
            std::fs::write(&path, format!("{recorded}\n"))?;
            eprintln!("baseline written: {}", path.display());
        } else {
            let previous = std::fs::read_to_string(&path)?;
            if previous.trim() != recorded.trim() {
                eprintln!(
                    "result differs from {}; run with --update-baseline to accept",
                    path.display()
                );
                return Err("web article eval baseline mismatch".into());
            }
            eprintln!("web article eval: at baseline");
        }
    }
    let top1_pct = rate(top1);
    if model && context && top1_pct < TOP1_FLOOR {
        return Err(format!("top-1 {top1_pct}% is below the {TOP1_FLOOR}% floor").into());
    }
    eprintln!(
        "web article eval: {clauses} clauses, top-1 {top1_pct}% (floor {TOP1_FLOOR}%), {} picks per clause",
        per_clause(steps_total, 100.0)
    );
    Ok(())
}

/// 模拟玩家选完一个分句；返回 (选择次数, 翻页次数)，卡住时返回 None。
fn play(host: &mut WebHost, mut frame: Frame, gold: &str) -> Option<(usize, usize)> {
    let mut committed = String::new();
    let mut steps = 0usize;
    let mut pages = 0usize;
    while committed.len() < gold.len() {
        let remaining = &gold[committed.len()..];
        let is_prefix = |text: &str| !text.is_empty() && remaining.starts_with(text);
        // 当前页上最长的正确前缀；同样长时取靠前的。
        let mut choice = frame
            .page
            .iter()
            .enumerate()
            .filter(|(_, row)| is_prefix(&row.text))
            .max_by_key(|(slot, row)| (row.text.len(), std::cmp::Reverse(*slot)))
            .map(|(slot, _)| slot);
        // 没有就往后翻，取第一个正确前缀。
        while choice.is_none() {
            let next = host.keys(&[Key::PageNext]);
            if next.page_index == frame.page_index {
                return None;
            }
            pages += 1;
            frame = next;
            choice = frame.page.iter().position(|row| is_prefix(&row.text));
        }
        steps += 1;
        frame = host.pick(choice?);
        for out in &frame.out {
            if let Out::Commit { text, .. } = out {
                committed.push_str(text);
            }
        }
        if !gold.starts_with(&committed) || steps >= MAX_STEPS {
            return None;
        }
        if !frame.composing && committed.len() < gold.len() {
            return None;
        }
    }
    Some((steps, pages))
}

fn typed(input: &str) -> Vec<Key> {
    input
        .bytes()
        .map(|byte| match byte {
            b'a'..=b'z' => Key::Letter(byte),
            _ => Key::Punct(byte),
        })
        .collect()
}

/// `extract_articles.py` 的输出：id、input、gold、syllables、tags、context。
fn load(path: &Path) -> Result<Vec<Case>, Box<dyn std::error::Error>> {
    let mut cases = Vec::new();
    for line in std::fs::read_to_string(path)?.lines() {
        if line.is_empty() || line.starts_with("id\t") {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() < 4 {
            return Err(format!("{}: malformed row", path.display()).into());
        }
        cases.push(Case {
            input: fields[1].to_owned(),
            gold: fields[2].to_owned(),
            context: fields.get(5).copied().unwrap_or("").to_owned(),
        });
    }
    Ok(cases)
}

/// 文件的 sha256，用系统自带的工具算（Linux 的 `sha256sum`，macOS 的 `shasum -a 256`），本 crate 不为一个本地检查再加哈希依赖。
fn sha256(path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let attempts: [(&str, &[&str]); 2] = [("sha256sum", &[]), ("shasum", &["-a", "256"])];
    for (program, args) in attempts {
        let Ok(output) = Command::new(program).args(args).arg(path).output() else {
            continue;
        };
        if output.status.success() {
            let text = String::from_utf8(output.stdout)?;
            if let Some(digest) = text.split_whitespace().next() {
                return Ok(digest.to_owned());
            }
        }
    }
    Err(format!("neither sha256sum nor shasum could hash {}", path.display()).into())
}
