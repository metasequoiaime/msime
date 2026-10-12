use super::*;
use crate::ime::queries::spells_whole_syllables;
use crate::ime::reads_as_known_japanese;
use crate::japanese::romaji::is_romaji_complete;
use crate::shuangpin::profile::default_profile;

/// 日文混输看的两个信号建立在整段读音上：读音完整、至少两个假名、模型在场才答，只有假名可给时不算。
#[test]
fn whole_reading_signals_need_a_complete_reading_and_the_model() {
    let model = test_model::bytes(
        &[
            ("おはよう", "おはよう", 0, 0, 500),
            ("かいし", "開始", 0, 0, 800),
        ],
        1,
        &[0],
    );
    let (_root, mut provider) = provider_with(Some(model));
    assert_eq!(
        provider.whole_reading_has_lemma(&request("ohayou")),
        Some(true)
    );
    assert_eq!(
        provider.whole_reading_has_lemma(&request("kaishiha")),
        Some(false)
    );
    let path = provider
        .whole_reading_best_path(&request("kaishi"))
        .expect("路径");
    assert_eq!(
        (path.text.as_str(), path.unknown_kana, path.kana),
        ("開始", 0, 3)
    );
    // 末尾待定字母、单个假名、非日文请求都不答。
    for raw in ["ohayo-k", "kaish", "ka"] {
        assert_eq!(
            provider.whole_reading_has_lemma(&request(raw)),
            None,
            "{raw}"
        );
        assert_eq!(
            provider.whole_reading_best_path(&request(raw)),
            None,
            "{raw}"
        );
    }
    let mut quanpin = request("ohayou");
    quanpin.scheme = SchemeType::Quanpin;
    assert_eq!(provider.whole_reading_has_lemma(&quanpin), None);

    // 模型缺失时只有假名，不算日文。
    let (_root, mut missing) = provider_with(None);
    assert_eq!(missing.whole_reading_has_lemma(&request("ohayou")), None);
    assert_eq!(missing.whole_reading_best_path(&request("ohayou")), None);
}

/// 混输转换的文字就是日文方案候选第一行的整句。
#[test]
fn the_best_path_is_the_first_provider_row() {
    let model = test_model::bytes(
        &[
            ("おはよう", "おはよう", 0, 0, 500),
            ("かいし", "開始", 0, 0, 800),
            ("かいし", "海士", 0, 0, 900),
            ("は", "は", 0, 0, 300),
        ],
        1,
        &[0],
    );
    let (_root, mut provider) = provider_with(Some(model));
    for raw in ["ohayou", "kaishi", "kaishiha", "ohayouha"] {
        let path = provider
            .whole_reading_best_path(&request(raw))
            .expect("路径");
        assert_eq!(path.text, provider.query(&request(raw))[0].word, "{raw}");
    }
}

fn real_model() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("MSIME_JAPANESE_MODEL") {
        return Some(PathBuf::from(path));
    }
    std::env::var_os("MSIME_EVAL_RESOURCES")
        .map(|resources| Path::new(&resources).join(crate::assets::JAPANESE_MODEL))
}

/// 拼音拆不开、没有精确词条、看得出是日文的输入，用来看「认得」信号的召回。
const INCOMPLETE_JAPANESE: &[&str] = &[
    "ohayougozaimasu",
    "tabemasu",
    "tabetai",
    "ikimasu",
    "nomimasu",
    "kakimasu",
    "yonda",
    "kimasu",
    "wakatta",
    "shitteiru",
    "matte",
    "ittekimasu",
    "itterasshai",
    "tsukareta",
    "nodogakawaita",
    "kyouhaiitenkidesune",
    "nihongowobenkyoushiteimasu",
    "watashihagakuseidesu",
    "korehananidesuka",
    "dokoniikimasuka",
];

struct Signals {
    exact: bool,
    path: BestPath,
}

fn signals(provider: &mut JapaneseProvider, raw: &str) -> Option<Signals> {
    let request = request(raw);
    Some(Signals {
        exact: provider.whole_reading_has_lemma(&request)?,
        path: provider.whole_reading_best_path(&request)?,
    })
}

fn pinyin_incomplete(raw: &str) -> bool {
    !spells_whole_syllables(raw, SchemeType::Quanpin, default_profile())
}

/// `ImeSession::mixed_japanese_conversion` 给不给一行的规则：精确，或者拼音拆不开且认得。
fn offers(raw: &str, signals: &Signals) -> bool {
    signals.exact || (pinyin_incomplete(raw) && reads_as_known_japanese(&signals.path))
}

fn eval_words() -> Vec<String> {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources/eval/quanpin-words-v1.tsv");
    let text = std::fs::read_to_string(path).expect("quanpin-words-v1.tsv");
    text.lines()
        .filter(|line| !line.starts_with('#'))
        .skip(1)
        .filter_map(|line| line.split('\t').nth(1).map(str::to_owned))
        .collect()
}

/// 用真实日文模型校准日文混输的信号，并给出各类输入里会混进日文的比例。设置 `MSIME_JAPANESE_MODEL`（`msime-japanese.dat` 的路径）或 `MSIME_EVAL_RESOURCES`（含它的资源目录）才跑，否则跳过；`--nocapture` 看数字。
#[test]
fn real_model_calibrates_mixed_japanese_signals() {
    let Some(model) = real_model() else {
        eprintln!("skipped: neither MSIME_JAPANESE_MODEL nor MSIME_EVAL_RESOURCES is set");
        return;
    };
    let mut provider = JapaneseProvider::new(&model);

    for raw in [
        "ohayou",
        "arigatou",
        "konnichiha",
        "konnnichiha",
        "sugoi",
        "kaishi",
        "watashi",
        "sushi",
        "nihao",
        "women",
        "xiexie",
        "shenme",
        "zaijian",
        "hello",
        "home",
    ] {
        let found = signals(&mut provider, raw).expect("完整读音");
        eprintln!(
            "{raw:12} pinyin_incomplete={:5} exact={:5} known={:5} offers={:5} cost/kana={:5} text={}",
            pinyin_incomplete(raw),
            found.exact,
            reads_as_known_japanese(&found.path),
            offers(raw, &found),
            found.path.cost / found.path.kana as i64,
            found.path.text
        );
    }
    let read = |provider: &mut JapaneseProvider, raw: &str| signals(provider, raw).expect(raw);
    for (raw, text) in [
        ("ohayou", "おはよう"),
        ("arigatou", "ありがとう"),
        ("konnnichiha", "こんにちは"),
        ("sugoi", "すごい"),
        ("kaishi", "開始"),
    ] {
        let found = read(&mut provider, raw);
        assert!(found.exact, "{raw}");
        assert_eq!(found.path.text, text, "{raw}");
    }
    // `nn` 一律是一个ん，`konnichiha` 读成こんいちは，最佳整句今イチは不像日文，不给。
    let konnichiha = read(&mut provider, "konnichiha");
    assert!(!konnichiha.exact && !reads_as_known_japanese(&konnichiha.path));
    for raw in ["nihao", "women", "xiexie", "shenme", "zaijian"] {
        assert!(!offers(raw, &read(&mut provider, raw)), "{raw}");
    }
    let hello = read(&mut provider, "hello");
    assert!(!offers("hello", &hello));

    let recalled = INCOMPLETE_JAPANESE
        .iter()
        .filter(|raw| {
            let found = read(&mut provider, raw);
            assert!(pinyin_incomplete(raw) && !found.exact, "{raw}");
            reads_as_known_japanese(&found.path)
        })
        .count();
    eprintln!(
        "incomplete Japanese without an exact lemma: known {recalled}/{}",
        INCOMPLETE_JAPANESE.len()
    );
    assert!(recalled * 10 >= INCOMPLETE_JAPANESE.len() * 9);

    // 拼音删一个键或对调两键、拆不开却是完整罗马字的输入：混进日文的多半是打错的拼音。
    let words = eval_words();
    let mut typos = Vec::new();
    for word in &words {
        let bytes = word.as_bytes();
        for index in 0..bytes.len() {
            let mut deleted = bytes.to_vec();
            deleted.remove(index);
            typos.push(deleted);
            if index + 1 < bytes.len() {
                let mut swapped = bytes.to_vec();
                swapped.swap(index, index + 1);
                typos.push(swapped);
            }
        }
    }
    let mut typos: Vec<String> = typos
        .into_iter()
        .map(|bytes| String::from_utf8(bytes).expect("ASCII"))
        .filter(|raw| pinyin_incomplete(raw) && is_romaji_complete(raw))
        .collect();
    typos.sort();
    typos.dedup();
    let (mut typo_exact, mut typo_known, mut typo_measured) = (0, 0, 0);
    for raw in &typos {
        let Some(found) = signals(&mut provider, raw) else {
            continue;
        };
        typo_measured += 1;
        if found.exact {
            typo_exact += 1;
        } else if reads_as_known_japanese(&found.path) {
            typo_known += 1;
        }
    }
    eprintln!(
        "pinyin typos that spell complete romaji: {typo_measured}; offered by exact {typo_exact}, by known only {typo_known}"
    );
    assert!(typo_known * 100 < typo_measured);

    // 打完整词之前经过的前缀状态（拆不开、完整罗马字）：日文行在这些状态上会短暂领先。
    let mut prefixes: Vec<&str> = words
        .iter()
        .flat_map(|word| (2..word.len()).map(move |end| &word[..end]))
        .filter(|raw| pinyin_incomplete(raw) && is_romaji_complete(raw))
        .collect();
    prefixes.sort_unstable();
    prefixes.dedup();
    let (mut prefix_exact, mut prefix_known) = (0, 0);
    for raw in &prefixes {
        let Some(found) = signals(&mut provider, raw) else {
            continue;
        };
        if found.exact {
            prefix_exact += 1;
        } else if reads_as_known_japanese(&found.path) {
            prefix_known += 1;
        }
    }
    eprintln!(
        "typing prefixes that spell complete romaji: {}; offered by exact {prefix_exact}, by known only {prefix_known}",
        prefixes.len()
    );

    // 拼得开的拼音只靠精确词条混进日文：整词和单音节各有多少。
    let complete_exact = words
        .iter()
        .filter(|raw| signals(&mut provider, raw).is_some_and(|found| found.exact))
        .count();
    let syllables: Vec<&str> = crate::pinyin::syllables::intact_pinyin_list()
        .iter()
        .copied()
        .filter(|raw| signals(&mut provider, raw).is_some_and(|found| found.exact))
        .collect();
    eprintln!(
        "quanpin-words-v1 words with an exact Japanese lemma: {complete_exact}/{}",
        words.len()
    );
    eprintln!(
        "pinyin syllables with an exact Japanese lemma: {}/{}: {}",
        syllables.len(),
        crate::pinyin::syllables::intact_pinyin_list().len(),
        syllables.join(" ")
    );
}
