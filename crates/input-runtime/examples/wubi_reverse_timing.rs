//! 在真实词库上测每次按键（一次候选刷新）的耗时，看五笔反查编码占了多少。
//!
//! 五笔方案里每次刷新都要给候选逐个反查完整编码，这个例子分别跑五笔、五笔混输拼音和全拼三种方案，打印每键平均耗时、最慢一键和平均候选数。数字要和另一份构建的结果成对比较，不要当绝对值读：同一份构建在有负载的机器上前后能差好几倍。
//!
//! 用法：`cargo run -p msime-input-runtime --example wubi_reverse_timing -- <资源目录>`
use std::time::Instant;

use msime_engine::host::{prepare_options, Command, Session};
use msime_engine::SchemeType;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let resources = std::env::args_os()
        .nth(1)
        .ok_or("usage: wubi_reverse_timing <resources>")?;
    let resources = std::fs::canonicalize(resources)?;
    let state = tempfile::tempdir()?;
    let started = Instant::now();
    let options = prepare_options(
        resources.to_str().ok_or("non-UTF-8 resource path")?,
        state.path().join("user").to_str().ok_or("path")?,
        state.path().join("cache").to_str().ok_or("path")?,
        "wubi-reverse-timing",
    )?;
    println!(
        "准备代次 {:8.1}ms",
        started.elapsed().as_secs_f64() * 1000.0
    );

    // 合成的输入：五笔取每个首字母及其两字母组合，覆盖每次最多 50 行的前缀查询；拼音取常见音节串。
    let letters: Vec<u8> = (b'a'..=b'y').filter(|letter| *letter != b'z').collect();
    let wubi: Vec<Vec<u8>> = letters
        .iter()
        .flat_map(|first| {
            letters
                .iter()
                .step_by(3)
                .map(move |second| vec![*first, *second])
        })
        .collect();
    let pinyin: Vec<Vec<u8>> = [
        "nihao", "women", "zhongguo", "shijie", "jintian", "mingtian", "shurufa", "diannao",
        "gongzuo", "xuexi", "pengyou", "wenti", "shenme", "keyi", "zhidao", "xihuan",
    ]
    .iter()
    .map(|text| text.as_bytes().to_vec())
    .collect();

    for (label, scheme, mixed, inputs) in [
        ("五笔", SchemeType::Wubi, false, &wubi),
        ("五笔混输拼音", SchemeType::Wubi, true, &pinyin),
        ("全拼", SchemeType::Quanpin, false, &pinyin),
    ] {
        let mut options = options.clone();
        options.scheme = scheme as u8;
        options.wubi_mixed_pinyin = mixed;
        let mut session = Session::new(&options)?;
        let mut keys = 0u32;
        let mut candidates = 0usize;
        let mut total = 0f64;
        let mut slowest = 0f64;
        for input in inputs {
            for byte in input {
                let started = Instant::now();
                session.character(*byte, false)?;
                let view = session.snapshot()?;
                let elapsed = started.elapsed().as_secs_f64() * 1000.0;
                keys += 1;
                candidates += view.candidates.len();
                total += elapsed;
                slowest = slowest.max(elapsed);
            }
            session.command(Command::Cancel)?;
        }
        println!(
            "{label:<8} 按键 {keys:4}  每键平均 {:8.2}ms  最慢 {slowest:8.2}ms  平均候选 {:6.1}",
            total / f64::from(keys),
            candidates as f64 / f64::from(keys)
        );
    }
    Ok(())
}
