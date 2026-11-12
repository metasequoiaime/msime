//! 模糊音候选排序的真实词库探针（#11321）。
//!
//!   fuzzy_rank_probe <资源目录> rank <规则掩码> <行数> <输入>[=<目标词>]...
//!   fuzzy_rank_probe <资源目录> learn <规则掩码> <输入> <目标词> <选几次>
//!
//! 资源目录是按 `resources/desktop-dictionary.lock.json` 备好的词库目录，规则掩码是 `fuzzy_rule` 各位之和（z=zh 是 1，全部是 2047）。`rank` 逐个输入打印前若干行候选（权重、来源、是否模糊命中、匹配的拼音和完整读音）和目标词的位置；`learn` 照 #11321 的步骤反复选同一个词，打印每次的位置，最后换一个新会话再看一次。用户目录和缓存都建在临时目录里，不碰本机的输入法状态。
use msime_engine::host::options::session_options;
use msime_engine::host::prepare_options;
use msime_engine::{Command, Session, WordItem};

type Failure = Box<dyn std::error::Error>;

struct Store {
    resources: String,
    temporary: tempfile::TempDir,
}

impl Store {
    fn new(resources: &std::ffi::OsStr) -> Result<Self, Failure> {
        let resources = std::fs::canonicalize(resources)?;
        Ok(Self {
            resources: resources
                .to_str()
                .ok_or("resource path is not UTF-8")?
                .to_owned(),
            temporary: tempfile::tempdir()?,
        })
    }

    /// 产品默认值上只改模糊规则、打开学习和候补整句（各平台宿主都开着候补整句）。
    fn session(&self, rules: u32) -> Result<Session, Failure> {
        let mut options = prepare_options(
            &self.resources,
            self.temporary.path().join("user").to_str().unwrap(),
            self.temporary.path().join("cache").to_str().unwrap(),
            "fuzzy-rank-probe",
        )?;
        options.fuzzy_pinyin_rules = rules;
        options.learning = true;
        options.sentence_alternatives = true;
        Ok(Session::new(session_options(&options)?)?)
    }
}

fn candidates(session: &mut Session, input: &str) -> Vec<WordItem> {
    session.command(Command::Cancel);
    for byte in input.bytes() {
        session.character(byte, false);
    }
    session.snapshot().candidates
}

fn position(rows: &[WordItem], target: &str) -> Option<usize> {
    rows.iter()
        .position(|row| row.word == target)
        .map(|index| index + 1)
}

fn print_rows(rows: &[WordItem], count: usize) {
    for (index, row) in rows.iter().take(count).enumerate() {
        println!(
            "  {:>2} {}{}\t{}\t{:?}\t{}\t{}",
            index + 1,
            row.word,
            if row.fuzzy { " ~" } else { "" },
            row.weight,
            row.source,
            row.pinyin,
            row.canonical_pinyin
        );
    }
}

fn main() -> Result<(), Failure> {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    let text = |index: usize| -> Result<&str, Failure> {
        Ok(arguments
            .get(index)
            .and_then(|value| value.to_str())
            .ok_or("usage: fuzzy_rank_probe <resources> rank <rules> <rows> <input>[=<word>]... | learn <rules> <input> <word> <times>")?)
    };
    let store = Store::new(arguments.first().ok_or("missing resource directory")?)?;
    let rules: u32 = text(2)?.parse()?;
    match text(1)? {
        "rank" => {
            let count: usize = text(3)?.parse()?;
            let mut session = store.session(rules)?;
            for index in 4..arguments.len() {
                let (input, target) = text(index)?.split_once('=').unwrap_or((text(index)?, ""));
                let rows = candidates(&mut session, input);
                match position(&rows, target) {
                    Some(seat) => println!("{input} rules={rules} 「{target}」第 {seat} 位"),
                    None if target.is_empty() => println!("{input} rules={rules}"),
                    None => println!("{input} rules={rules} 「{target}」不在候选里"),
                }
                print_rows(&rows, count);
            }
        }
        "learn" => {
            let (input, target) = (text(3)?, text(4)?);
            let times: usize = text(5)?.parse()?;
            {
                let mut session = store.session(rules)?;
                for round in 0..=times {
                    let rows = candidates(&mut session, input);
                    let Some(seat) = position(&rows, target) else {
                        println!("第 {} 次打 {input}：「{target}」不在候选里", round + 1);
                        break;
                    };
                    println!(
                        "第 {} 次打 {input}：「{target}」排第 {seat}，权重 {}",
                        round + 1,
                        rows[seat - 1].weight
                    );
                    if round < times {
                        let result = session.select(seat - 1);
                        if result.commit.as_deref() != Some(target) {
                            println!("  选词没有上屏：{:?}", result.commit);
                        }
                        if let Some(diagnostic) = result.diagnostic {
                            println!("  诊断：{diagnostic}");
                        }
                    }
                }
                session.command(Command::Cancel);
            }
            let mut session = store.session(rules)?;
            let rows = candidates(&mut session, input);
            match position(&rows, target) {
                Some(seat) => println!("新会话里再打 {input}：「{target}」排第 {seat}"),
                None => println!("新会话里再打 {input}：「{target}」不在候选里"),
            }
            print_rows(&rows, 9);
        }
        _ => return Err("the mode is rank or learn".into()),
    }
    Ok(())
}
