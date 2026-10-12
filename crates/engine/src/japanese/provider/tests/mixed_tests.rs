use super::*;
use crate::ime::queries::spells_whole_syllables;
use crate::ime::{counts_as_exact_japanese, reads_as_known_japanese};
use crate::japanese::romaji::is_romaji_complete;
use crate::shuangpin::profile::default_profile;

/// 日文混输看的两个信号建立在整段读音上：读音完整、至少两个假名、模型在场才答，只有假名可给时不算。精确词条要三拍以上才算精确，拗音的小写假名不单算拍。
#[test]
fn whole_reading_signals_need_a_complete_reading_and_the_model() {
    let model = test_model::bytes(
        &[
            ("おはよう", "おはよう", 0, 0, 500),
            ("かい", "会", 0, 0, 400),
            ("かいし", "開始", 0, 0, 800),
            ("しょう", "章", 0, 0, 400),
        ],
        1,
        &[0],
    );
    let (_root, mut provider) = provider_with(Some(model));
    // （输入，有没有精确词条，拍数，算不算精确）
    for (raw, exact, morae, counts) in [
        ("ohayou", true, 4, true),
        ("kaishi", true, 3, true),
        ("kaishiha", false, 4, false),
        ("kai", true, 2, false),
        ("shou", true, 2, false),
    ] {
        let found = provider.whole_reading_lemma(&request(raw));
        assert_eq!(found, Some(WholeReadingLemma { exact, morae }), "{raw}");
        assert_eq!(counts_as_exact_japanese(found.unwrap()), counts, "{raw}");
    }
    let path = provider
        .whole_reading_best_path(&request("kaishi"))
        .expect("路径");
    assert_eq!(
        (path.text.as_str(), path.unknown_kana, path.kana),
        ("開始", 0, 3)
    );
    // 末尾待定字母、单个假名、非日文请求都不答。
    for raw in ["ohayo-k", "kaish", "ka"] {
        assert_eq!(provider.whole_reading_lemma(&request(raw)), None, "{raw}");
        assert_eq!(
            provider.whole_reading_best_path(&request(raw)),
            None,
            "{raw}"
        );
    }
    let mut quanpin = request("ohayou");
    quanpin.scheme = SchemeType::Quanpin;
    assert_eq!(provider.whole_reading_lemma(&quanpin), None);

    // 模型缺失时只有假名，不算日文。
    let (_root, mut missing) = provider_with(None);
    assert_eq!(missing.whole_reading_lemma(&request("ohayou")), None);
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
    lemma: WholeReadingLemma,
    path: BestPath,
}

impl Signals {
    /// 现行的精确信号：三拍以上的精确词条。
    fn exact(&self) -> bool {
        counts_as_exact_japanese(self.lemma)
    }

    fn known(&self) -> bool {
        reads_as_known_japanese(&self.path)
    }
}

fn signals(provider: &mut JapaneseProvider, raw: &str) -> Option<Signals> {
    let request = request(raw);
    Some(Signals {
        lemma: provider.whole_reading_lemma(&request)?,
        path: provider.whole_reading_best_path(&request)?,
    })
}

fn pinyin_incomplete(raw: &str) -> bool {
    !spells_whole_syllables(raw, SchemeType::Quanpin, default_profile())
}

/// `ImeSession::mixed_japanese_conversion` 给不给一行的规则：精确（三拍以上的词条），或者拼音拆不开且认得。
fn offers(raw: &str, signals: &Signals) -> bool {
    signals.exact() || (pinyin_incomplete(raw) && signals.known())
}

/// 拼音拆不开时 `InputSession::mixed_japanese_row` 让日文领先的规则：认得才领先，只有精确词条时进优先位置。
fn leads_when_incomplete(raw: &str, signals: &Signals) -> bool {
    offers(raw, signals) && signals.known()
}

/// 收紧之前的宽规则（比较用）：任何精确词条都给，拼音拆不开时给了就领先。
fn loose_offers(raw: &str, signals: &Signals) -> bool {
    signals.lemma.exact || (pinyin_incomplete(raw) && signals.known())
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

/// 用真实日文模型校准日文混输的信号，并给出各类输入里会混进日文的比例（现行规则和收紧前的宽规则各一份）。设置 `MSIME_JAPANESE_MODEL`（`msime-japanese.dat` 的路径）或 `MSIME_EVAL_RESOURCES`（含它的资源目录）才跑，否则跳过；`--nocapture` 看数字。
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
        "watashi",
        "konnichiha",
        "konnnichiha",
        "sugoi",
        "kaishi",
        "sushi",
        "nihao",
        "women",
        "xiexie",
        "shenme",
        "zaijian",
        "hao",
        "kai",
        "you",
        "shou",
        "wode",
        "tamen",
        "hello",
        "home",
    ] {
        let found = signals(&mut provider, raw).expect("完整读音");
        eprintln!(
            "{raw:12} pinyin_incomplete={:5} lemma={:5} morae={} exact={:5} known={:5} offers={:5} cost/kana={:5} text={}",
            pinyin_incomplete(raw),
            found.lemma.exact,
            found.lemma.morae,
            found.exact(),
            found.known(),
            offers(raw, &found),
            found.path.cost / found.path.kana as i64,
            found.path.text
        );
    }
    let read = |provider: &mut JapaneseProvider, raw: &str| signals(provider, raw).expect(raw);
    for (raw, text) in [
        ("ohayou", "おはよう"),
        ("arigatou", "ありがとう"),
        ("watashi", "私"),
        ("konnnichiha", "こんにちは"),
        ("sugoi", "すごい"),
        ("kaishi", "開始"),
    ] {
        let found = read(&mut provider, raw);
        assert!(found.exact(), "{raw}");
        assert_eq!(found.path.text, text, "{raw}");
    }
    // 拼音拆不开的两个招牌例子要靠「认得」才领先。
    for raw in ["konnnichiha", "sugoi"] {
        let found = read(&mut provider, raw);
        assert!(leads_when_incomplete(raw, &found), "{raw}");
    }
    // `nn` 一律是一个ん，`konnichiha` 读成こんいちは，最佳整句今イチは不像日文，不给。
    let konnichiha = read(&mut provider, "konnichiha");
    assert!(!konnichiha.lemma.exact && !konnichiha.known());
    // 两拍的读音（すし、はお、かい、よう、しょう）有词条也不算精确，拼得开的拼音不再混进日文。
    for raw in [
        "nihao", "women", "xiexie", "shenme", "zaijian", "sushi", "hao", "kai", "you", "shou",
    ] {
        assert!(!offers(raw, &read(&mut provider, raw)), "{raw}");
    }
    let hello = read(&mut provider, "hello");
    assert!(!offers("hello", &hello));

    let recalled = INCOMPLETE_JAPANESE
        .iter()
        .filter(|raw| {
            let found = read(&mut provider, raw);
            assert!(pinyin_incomplete(raw) && !found.lemma.exact, "{raw}");
            leads_when_incomplete(raw, &found)
        })
        .count();
    eprintln!(
        "incomplete Japanese without an exact lemma: offered and leading {recalled}/{}",
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
    let (mut typo_measured, mut typo_offered, mut typo_leading, mut typo_loose) = (0, 0, 0, 0);
    for raw in &typos {
        let Some(found) = signals(&mut provider, raw) else {
            continue;
        };
        typo_measured += 1;
        typo_offered += usize::from(offers(raw, &found));
        typo_leading += usize::from(leads_when_incomplete(raw, &found));
        typo_loose += usize::from(loose_offers(raw, &found));
    }
    eprintln!(
        "pinyin typos that spell complete romaji: {typo_measured}; offered {typo_offered}, leading {typo_leading}; loose rule offered and leading {typo_loose}"
    );
    assert!(typo_leading * 50 < typo_measured);

    // 打完整词之前经过的前缀状态（拆不开、完整罗马字）：日文行在这些状态上会短暂领先。
    let mut prefixes: Vec<&str> = words
        .iter()
        .flat_map(|word| (2..word.len()).map(move |end| &word[..end]))
        .filter(|raw| pinyin_incomplete(raw) && is_romaji_complete(raw))
        .collect();
    prefixes.sort_unstable();
    prefixes.dedup();
    let (mut prefix_offered, mut prefix_leading, mut prefix_loose) = (0, 0, 0);
    for raw in &prefixes {
        let Some(found) = signals(&mut provider, raw) else {
            continue;
        };
        prefix_offered += usize::from(offers(raw, &found));
        prefix_leading += usize::from(leads_when_incomplete(raw, &found));
        prefix_loose += usize::from(loose_offers(raw, &found));
    }
    eprintln!(
        "typing prefixes that spell complete romaji: {}; offered {prefix_offered}, leading {prefix_leading}; loose rule offered and leading {prefix_loose}",
        prefixes.len()
    );

    // 拼得开的拼音只靠精确词条混进日文：整词和单音节各有多少。
    let (mut word_offered, mut word_loose) = (0, 0);
    for raw in &words {
        let Some(found) = signals(&mut provider, raw) else {
            continue;
        };
        word_offered += usize::from(offers(raw, &found));
        word_loose += usize::from(loose_offers(raw, &found));
    }
    eprintln!(
        "quanpin-words-v1 words offered a Japanese row: {word_offered}/{}; loose rule {word_loose}",
        words.len()
    );
    let all_syllables = crate::pinyin::syllables::intact_pinyin_list();
    let offered_syllables = |rule: fn(&str, &Signals) -> bool, provider: &mut JapaneseProvider| {
        all_syllables
            .iter()
            .copied()
            .filter(|raw| signals(provider, raw).is_some_and(|found| rule(raw, &found)))
            .collect::<Vec<&str>>()
    };
    let syllables = offered_syllables(offers, &mut provider);
    let loose_syllables = offered_syllables(loose_offers, &mut provider);
    eprintln!(
        "pinyin syllables offered a Japanese row: {}/{}: {}; loose rule {}",
        syllables.len(),
        all_syllables.len(),
        syllables.join(" "),
        loose_syllables.len()
    );
}
