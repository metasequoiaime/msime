//! 打拼音出符号的真词库探针：对符号表里每个名称打出全拼（两个音节以上的另打首字母，拉丁名称打原文），断言首位不是符号行、符号行出现（首字母和单音节只在名称词出现时才要求），并打印名称词和第一个符号的位置。最后列出名称词不在前 20 个候选里的名称，供写表时换成词库里有的写法。
//!
//! 用法：`symbol_candidates_dictionary <已校验的资源目录>`，资源由 `msime-client-core` 的 `install_resources` 示例准备。
use msime_engine::host::session::Command;
use msime_engine::host::{prepare_options, EngineOptions, Session};

const TABLE: &str = include_str!("../src/local/symbol_candidates.tsv");
/// `CandidateSource::Emoji`：符号行与 emoji 行共用这个来源。
const EMOJI: u8 = 6;
const ANCHOR_HORIZON: usize = 20;

struct Probe<'a> {
    input: String,
    name: &'a str,
    symbols: Vec<&'a str>,
    /// 名称词不在候选里时也必须出现（全拼两个音节以上、拉丁名称）。
    required: bool,
}

fn probes() -> Vec<Probe<'static>> {
    let mut probes = Vec::new();
    for line in TABLE.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        let [name, pinyin, symbols] = fields[..] else {
            panic!("bad line {line:?}");
        };
        let symbols: Vec<&str> = symbols.split(' ').collect();
        if pinyin == "-" {
            probes.push(Probe {
                input: name.to_owned(),
                name,
                symbols,
                required: true,
            });
            continue;
        }
        let syllables: Vec<&str> = pinyin.split('\'').collect();
        probes.push(Probe {
            input: syllables.concat(),
            name,
            symbols: symbols.clone(),
            required: syllables.len() >= 2,
        });
        if syllables.len() >= 2 {
            probes.push(Probe {
                input: syllables
                    .iter()
                    .filter_map(|syllable| syllable.get(..1))
                    .collect(),
                name,
                symbols,
                required: false,
            });
        }
    }
    probes
}

fn options(resources: &str, root: &std::path::Path) -> msime_engine::Result<EngineOptions> {
    let mut options = prepare_options(
        resources,
        root.join("user").to_str().unwrap(),
        root.join("cache").to_str().unwrap(),
        "symbol-candidates-probe",
    )?;
    options.helpcode = false;
    options.learning = false;
    options.mixed_symbols = true;
    Ok(options)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let resources = std::fs::canonicalize(
        std::env::args_os()
            .nth(1)
            .ok_or("usage: symbol_candidates_dictionary <verified-resources>")?,
    )?;
    let temporary = tempfile::tempdir()?;
    let options = options(
        resources.to_str().ok_or("resource path is not UTF-8")?,
        temporary.path(),
    )?;
    let mut session = Session::new(&options)?;
    let mut far = Vec::new();
    let mut failures = Vec::new();
    println!("input\tname\tname_at\tsymbol_at\tfirst_page");
    for probe in probes() {
        for byte in probe.input.bytes() {
            session.character(byte, false)?;
        }
        let snapshot = session.snapshot()?;
        session.command(Command::Cancel)?;
        let name_at = snapshot
            .candidates
            .iter()
            .position(|word| word.eq_ignore_ascii_case(probe.name));
        // 词库里本来就有这个符号时（来源不是 Emoji）符号行被去重，也算出现。
        let symbol_at = snapshot
            .candidates
            .iter()
            .position(|word| probe.symbols.contains(&word.as_str()));
        let first_page: Vec<&str> = snapshot
            .candidates
            .iter()
            .take(6)
            .map(String::as_str)
            .collect();
        println!(
            "{}\t{}\t{}\t{}\t{}",
            probe.input,
            probe.name,
            name_at.map_or("-".to_owned(), |at| at.to_string()),
            symbol_at.map_or("-".to_owned(), |at| at.to_string()),
            first_page.join(" ")
        );
        if snapshot.candidate_sources.first() == Some(&EMOJI)
            && snapshot
                .candidates
                .first()
                .is_some_and(|word| probe.symbols.contains(&word.as_str()))
        {
            failures.push(format!("{}: 符号占了首位", probe.input));
        }
        let expected = probe.required || name_at.is_some();
        if expected && symbol_at.is_none() && !snapshot.candidates.is_empty() {
            failures.push(format!("{} ({}): 没有符号行", probe.input, probe.name));
        }
        if probe.required && name_at.is_none_or(|at| at >= ANCHOR_HORIZON) {
            far.push(format!("{} {} {:?}", probe.name, probe.input, name_at));
        }
    }
    println!(
        "\n名称词不在前 {ANCHOR_HORIZON} 个候选里的名称（{} 个）：",
        far.len()
    );
    for line in &far {
        println!("  {line}");
    }
    if !failures.is_empty() {
        for failure in &failures {
            eprintln!("FAIL {failure}");
        }
        return Err(format!("{} probes failed", failures.len()).into());
    }
    println!("symbol candidate probe passed");
    Ok(())
}
