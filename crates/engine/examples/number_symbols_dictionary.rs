//! 数字序号的真词库验收：检查发现、选中后的整段提交以及关闭符号混输后的来源；用法：`number_symbols_dictionary <已校验资源目录>`。
use msime_engine::host::session::Command;
use msime_engine::host::{prepare_options, EngineOptions, Session};
use msime_engine::ShuangpinProfileKind;

const TABLE: &str = include_str!("../src/local/symbol_candidates.tsv");

fn verify(
    resources: &str,
    root: &std::path::Path,
    label: &str,
    input: &str,
    symbols: &[&str],
    enabled: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut options: EngineOptions = prepare_options(
        resources,
        root.join("user").to_str().unwrap(),
        root.join("cache").to_str().unwrap(),
        "number-symbols-probe",
    )?;
    options.learning = false;
    options.helpcode = false;
    options.mixed_symbols = enabled;
    if label == "xiaohe" {
        options.scheme = 1;
        options.shuangpin_profile = ShuangpinProfileKind::Xiaohe as u8;
    }
    let mut session = Session::new(&options)?;
    if label == "nine-key" {
        session.set_nine_key_enabled(true)?;
    }
    for symbol in symbols {
        for byte in input.bytes() {
            session.character(byte, false)?;
        }
        let snapshot = session.snapshot()?;
        if enabled {
            let index = snapshot
                .candidates
                .iter()
                .position(|word| word == symbol)
                .ok_or_else(|| format!("{label} {input}: 缺少 {symbol}"))?;
            let result = session.select(index)?;
            assert!(result.has_commit, "{label} {input}: 没有提交 {symbol}");
            assert_eq!(result.commit, *symbol, "{label} {input}: 上屏内容");
            assert!(
                result.diagnostic.is_empty(),
                "{label} {input}: {}",
                result.diagnostic
            );
            let after = session.snapshot()?;
            assert!(
                after.preedit.is_empty() && after.editing_text.is_empty(),
                "{label} {input}: 序号选中后仍有组字"
            );
            println!("{label} {input} -> {symbol}: committed at {index}");
        } else {
            // 词库自身收录的符号仍属于普通候选；这里只检查本次混输来源没有绕过开关。
            assert!(
                !snapshot
                    .candidates
                    .iter()
                    .zip(&snapshot.candidate_sources)
                    .any(|(word, source)| word == symbol && *source == 6),
                "{label} {input}: 关闭符号混输后仍有 {symbol}"
            );
            session.command(Command::Cancel)?;
        }
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let resources = std::fs::canonicalize(
        std::env::args_os()
            .nth(1)
            .ok_or("usage: number_symbols_dictionary <verified-resources>")?,
    )?;
    let resources = resources.to_str().ok_or("resource path is not UTF-8")?;
    let temporary = tempfile::tempdir()?;
    let mut numbers = 0;
    for line in TABLE
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
    {
        let fields: Vec<&str> = line.split('\t').collect();
        if !fields[0]
            .chars()
            .all(|character| "一二三四五六七八九十".contains(character))
        {
            continue;
        }
        numbers += 1;
        let input = fields[1].replace('\'', "");
        let symbols: Vec<&str> = fields[2].split(' ').collect();
        for enabled in [true, false] {
            verify(
                resources,
                temporary.path(),
                "quanpin",
                &input,
                &symbols,
                enabled,
            )?;
        }
    }
    assert_eq!(numbers, 50);
    for (label, input, symbols) in [
        ("xiaohe", "erui", vec!["⑳", "⒇", "⒛"]),
        ("nine-key", "94", vec!["①", "⑴", "⒈"]),
    ] {
        for enabled in [true, false] {
            verify(resources, temporary.path(), label, input, &symbols, enabled)?;
        }
    }
    println!("number symbols: 50 names, selection, switch, shuangpin and nine-key passed");
    Ok(())
}
