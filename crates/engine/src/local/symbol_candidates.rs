//! 打拼音出符号：全拼、双拼（26 键和九宫格）整段组字正好拼出某个符号的名称时，混排给出对应的符号，`zhengfu` 得 ±、`pingfangmi` 得 ㎡、`gamma` 得 γ Γ。
//!
//! 数据是本项目自写的 `symbol_candidates.tsv`，编进引擎，不读磁盘；格式和写法规则见文件头。它是 msime-dictionary `sources/symbols/symbol-names.tsv` 的逐字节副本，由 `scripts/sync_symbol_tables.py` 按 `resources/symbol-tables.lock.json` 同步，改名称要改词库仓，不在这里手改（`scripts/test-symbol-tables.py` 核对）。第一次查询时解析一次，建三个索引：全拼（去掉 `'`）、两个音节以上非数字名称的首字母、拉丁名称。
//!
//! 只认完全相等，不按前缀：表是封闭的小集合，前缀命中几乎都是噪音（`zheng` 能命中正负、正比……），打完整名称只多几个字母。行的位置由混排决定（`queries::place_symbol_rows`）：跟在名称那个词后面，永远不占首位。行的来源是 `CandidateSource::Emoji`，所以选中时整段上屏、结束组字，不学习、不调频、不进个人上下文，也不能置顶、固定或删除。

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::shuangpin::query::normalize_input;
use crate::shuangpin::ShuangpinProfile;
use crate::types::{CandidateSource, SchemeType, WordItem};

const TABLE: &str = include_str!("symbol_candidates.tsv");

/// 一次最多给出几行符号，各名称合计。混排去重用一个 `u64` 记去留，不能超过 64。
pub const MIXED_SYMBOL_LIMIT: usize = 8;
/// 一个名称最多几个符号，由表检查保证。
#[cfg(test)]
pub const SYMBOLS_PER_NAME: usize = 6;
/// 拉丁名称至少几个字母；不到 `LATIN_SPELLING_FREE_LENGTH` 个字母的还不能整段拆成拼音：`phi`、`psi`、`rho`、`tau` 收，`pi`、`mu`、`xi`、`nu`、`chi`、`eta` 是常用拼音（`eta` 拆成 e'ta），这些希腊字母走中文名，符号模式里另有 LaTeX 命令名。
#[cfg(test)]
pub const LATIN_NAME_MINIMUM: usize = 3;
/// 至少这么多字母的拉丁名称不再检查能否拆成拼音，`beta`、`zeta` 这类表里原有的名称照收。
#[cfg(test)]
pub const LATIN_SPELLING_FREE_LENGTH: usize = 4;
/// 单音节名称只收有依据的派、度、一至十和希腊字母的单字读音（缪 μ、纽 ν、柔 ρ、套 τ、斐 φ、凯 χ）：常用音节必须有名称锚定词，且最早排第 4 位（`SymbolRow::ambiguous`），避免序号和字母挤动前三个普通词。
#[cfg(test)]
pub const SINGLE_SYLLABLE_NAMES: [&str; 18] = [
    "派", "度", "一", "二", "三", "四", "五", "六", "七", "八", "九", "十", "缪", "纽", "柔", "套",
    "斐", "凯",
];

/// 混排的一行符号，带着决定它位置的信息，规则见 `queries::place_symbol_rows`。
#[derive(Debug, Clone, PartialEq)]
pub struct SymbolRow {
    pub item: WordItem,
    /// 锚定词：中文名称就是那个词，拉丁名称是英文混输给出的同名英文行（`gamma`）。同一次查询里几个名称给出同一个符号时（`dg` 的对钩、打钩都给 √）每个名称都在，按表的先后；符号跟在其中排得最靠前的那个后面。
    pub anchors: Vec<&'static str>,
    /// 输入含糊：只有一个音节（派、度），或者是两个字母的首字母（`ax` 爱心、`jf` 积分）。这样的输入能指的词太多，符号跟在锚定词后面时最早排第 4 位，不挤动前三个词。
    pub ambiguous: bool,
    /// 锚定词不在候选里时也出现（排在第一页最后一位）。全拼两个音节以上和拉丁名称是，单音节和首字母匹配不是。
    pub unanchored: bool,
}

/// 表里的一行。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SymbolEntry {
    pub name: &'static str,
    /// 带 `'` 的全拼；拉丁名称为 `None`。
    pub pinyin: Option<&'static str>,
    /// 空格分隔的符号。
    pub symbols: &'static str,
}

impl SymbolEntry {
    pub fn symbols(&self) -> impl Iterator<Item = &'static str> {
        self.symbols.split(' ')
    }

    pub fn syllables(&self) -> usize {
        self.pinyin.map_or(0, |pinyin| pinyin.split('\'').count())
    }
}

struct SymbolTable {
    entries: Vec<SymbolEntry>,
    /// 全拼去掉 `'` → 行号，按表中先后。
    quanpin: HashMap<String, Vec<usize>>,
    /// 两个音节以上中文名称的首字母 → 行号。
    initials: HashMap<String, Vec<usize>>,
    /// 拉丁名称 → 行号。
    latin: HashMap<&'static str, Vec<usize>>,
}

fn table() -> &'static SymbolTable {
    static TABLE_CELL: OnceLock<SymbolTable> = OnceLock::new();
    TABLE_CELL.get_or_init(|| SymbolTable::parse(TABLE))
}

impl SymbolTable {
    fn parse(text: &'static str) -> Self {
        let entries: Vec<SymbolEntry> = parse_entries(text);
        let mut quanpin: HashMap<String, Vec<usize>> = HashMap::new();
        let mut initials: HashMap<String, Vec<usize>> = HashMap::new();
        let mut latin: HashMap<&'static str, Vec<usize>> = HashMap::new();
        for (index, entry) in entries.iter().enumerate() {
            let Some(pinyin) = entry.pinyin else {
                latin.entry(entry.name).or_default().push(index);
                continue;
            };
            quanpin
                .entry(pinyin.replace('\'', ""))
                .or_default()
                .push(index);
            // 数字名称的首字母高度重合（十三、十四、三十、四十都是 ss），只接受完整拼音，避免混入其他数字或耗尽符号上限。
            let number_name = entry
                .name
                .chars()
                .all(|character| "一二三四五六七八九十".contains(character));
            if entry.syllables() >= 2 && !number_name {
                let first_letters = pinyin
                    .split('\'')
                    .filter_map(|syllable| syllable.chars().next())
                    .collect();
                initials.entry(first_letters).or_default().push(index);
            }
        }
        Self {
            entries,
            quanpin,
            initials,
            latin,
        }
    }
}

/// 解析整张表，跳过空行和 `#` 注释；列数不对的行也跳过（表检查会把它们报出来）。
fn parse_entries(text: &'static str) -> Vec<SymbolEntry> {
    text.lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| {
            let mut fields = line.split('\t');
            let (Some(name), Some(pinyin), Some(symbols), None) =
                (fields.next(), fields.next(), fields.next(), fields.next())
            else {
                return None;
            };
            Some(SymbolEntry {
                name,
                pinyin: (pinyin != "-").then_some(pinyin),
                symbols,
            })
        })
        .collect()
}

/// 表里的全部行，供数据检查用；真词库探针直接读 TSV。
#[cfg(test)]
pub fn symbol_entries() -> &'static [SymbolEntry] {
    &table().entries
}

/// 名称怎么命中的，决定行的位置。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Match {
    Quanpin,
    Latin,
    Initials,
}

/// 26 键混排：整段组字（去掉 `'`、转成小写）正好是某个名称时给出它的符号。全拼认名称的全拼、两个音节以上非数字名称的首字母（`ssd` 摄氏度）和拉丁名称（`gamma`）；双拼把输入归一成全拼后比全拼，另按原样比拉丁名称，不认首字母。微软双拼的 `;` 是 `ing` 韵母键，作为双拼编码保留；其他方案、输入不到两键或含无效字符时没有行。同一个符号只出一次，合计最多 `MIXED_SYMBOL_LIMIT` 行；行的 `pinyin` 是整段输入，选中即吃掉全部组字。
pub fn query_mixed_symbols(
    code: &str,
    scheme: SchemeType,
    profile: &ShuangpinProfile,
) -> Vec<SymbolRow> {
    if !matches!(scheme, SchemeType::Quanpin | SchemeType::Shuangpin) {
        return Vec::new();
    }
    let mut keys = String::with_capacity(code.len());
    for byte in code.bytes() {
        match byte {
            b'\'' => {}
            byte if byte.is_ascii_alphabetic() => keys.push(byte.to_ascii_lowercase() as char),
            b';' if scheme == SchemeType::Shuangpin && profile.uses_semicolon_key() => {
                keys.push(';');
            }
            _ => return Vec::new(),
        }
    }
    if keys.len() < 2 {
        return Vec::new();
    }
    let table = table();
    let mut hits: Vec<(usize, Match)> = Vec::new();
    let mut add = |indices: Option<&Vec<usize>>, kind: Match| {
        hits.extend(indices.into_iter().flatten().map(|&index| (index, kind)));
    };
    if scheme == SchemeType::Shuangpin {
        add(
            table.quanpin.get(normalize_input(&keys, profile).as_str()),
            Match::Quanpin,
        );
    } else {
        add(table.quanpin.get(keys.as_str()), Match::Quanpin);
    }
    add(table.latin.get(keys.as_str()), Match::Latin);
    if scheme == SchemeType::Quanpin {
        add(table.initials.get(keys.as_str()), Match::Initials);
    }
    rows_for(table, &hits, code)
}

/// 整串（已转小写）正好是某个拉丁名称时它的符号，按表的先后：符号模式先给这些正文用的字母，再给 LaTeX 命令名的字形变体。
pub fn latin_name_symbols(code: &str) -> impl Iterator<Item = &'static str> {
    let table = table();
    table
        .latin
        .get(code)
        .into_iter()
        .flatten()
        .flat_map(move |&index| table.entries[index].symbols())
}

/// 九宫格混排：依次按 `readings`（小写全拼，没有 `'`，可能性高的在前）找全拼与读法完全相等、并且通过 `accept` 的名称。不认首字母（数字串的首字母读法太多），也不认拉丁名称。行的 `pinyin` 是命中的读法，调用方换成整串数字。
pub fn query_symbol_readings(readings: &[String], accept: &dyn Fn(&str) -> bool) -> Vec<SymbolRow> {
    let table = table();
    let mut rows = Vec::new();
    for reading in readings {
        if reading.len() < 2 || !accept(reading) {
            continue;
        }
        let hits: Vec<(usize, Match)> = table
            .quanpin
            .get(reading.as_str())
            .into_iter()
            .flatten()
            .map(|&index| (index, Match::Quanpin))
            .collect();
        for row in rows_for(table, &hits, reading) {
            if let Some(known) = rows
                .iter_mut()
                .find(|known: &&mut SymbolRow| known.item.word == row.item.word)
            {
                merge_anchors(known, &row.anchors, row.ambiguous, row.unanchored);
                continue;
            }
            if rows.len() == MIXED_SYMBOL_LIMIT {
                continue;
            }
            rows.push(row);
        }
    }
    rows
}

/// 同一个符号又由另一个名称给出：记下新的锚定词，位置规则取两者中宽的那个（任一个不含糊就不含糊，任一个能脱离锚定词出现就能）。
fn merge_anchors(row: &mut SymbolRow, anchors: &[&'static str], ambiguous: bool, unanchored: bool) {
    for &anchor in anchors {
        if !row.anchors.contains(&anchor) {
            row.anchors.push(anchor);
        }
    }
    row.ambiguous &= ambiguous;
    row.unanchored |= unanchored;
}

fn rows_for(table: &SymbolTable, hits: &[(usize, Match)], code: &str) -> Vec<SymbolRow> {
    let mut rows: Vec<SymbolRow> = Vec::new();
    for &(index, kind) in hits {
        let entry = &table.entries[index];
        let single_syllable = kind == Match::Quanpin && entry.syllables() == 1;
        let ambiguous = single_syllable || (kind == Match::Initials && entry.syllables() == 2);
        let unanchored = kind != Match::Initials && !single_syllable;
        for symbol in entry.symbols() {
            if let Some(known) = rows.iter_mut().find(|row| row.item.word == symbol) {
                merge_anchors(known, &[entry.name], ambiguous, unanchored);
                continue;
            }
            if rows.len() == MIXED_SYMBOL_LIMIT {
                continue;
            }
            rows.push(SymbolRow {
                item: WordItem::new(code, symbol, 0, CandidateSource::Emoji, ""),
                anchors: vec![entry.name],
                ambiguous,
                unanchored,
            });
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use crate::shuangpin::profile::profile;
    use crate::types::ShuangpinProfileKind;

    fn quanpin(code: &str) -> Vec<SymbolRow> {
        query_mixed_symbols(
            code,
            SchemeType::Quanpin,
            crate::shuangpin::profile::default_profile(),
        )
    }

    fn texts(rows: &[SymbolRow]) -> Vec<&str> {
        rows.iter().map(|row| row.item.word.as_str()).collect()
    }

    #[test]
    fn the_table_follows_its_rules() {
        let mut lines = 0;
        for line in TABLE.lines() {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            lines += 1;
            assert_eq!(line.split('\t').count(), 3, "三列：{line}");
        }
        let entries = symbol_entries();
        assert_eq!(entries.len(), lines);
        let mut pairs = HashSet::new();
        for entry in entries {
            match entry.pinyin {
                Some(pinyin) => {
                    assert!(
                        entry.name.chars().all(crate::text::is_han),
                        "中文名称只含汉字：{}",
                        entry.name
                    );
                    let syllables: Vec<&str> = pinyin.split('\'').collect();
                    assert_eq!(
                        syllables.len(),
                        entry.name.chars().count(),
                        "音节数等于字数：{} {pinyin}",
                        entry.name
                    );
                    for syllable in syllables {
                        assert!(
                            syllable.bytes().all(|byte| byte.is_ascii_lowercase())
                                && crate::pinyin::segment::is_complete_pinyin_input(syllable),
                            "完整全拼音节：{} {pinyin}",
                            entry.name
                        );
                    }
                    if entry.syllables() == 1 {
                        assert!(
                            SINGLE_SYLLABLE_NAMES.contains(&entry.name),
                            "单音节名称必须在明确的白名单中：{}",
                            entry.name
                        );
                    }
                }
                None => {
                    assert!(
                        entry.name.len() >= LATIN_NAME_MINIMUM
                            && entry.name.bytes().all(|byte| byte.is_ascii_lowercase()),
                        "拉丁名称全小写、至少 {LATIN_NAME_MINIMUM} 个字母：{}",
                        entry.name
                    );
                    assert!(
                        entry.name.len() >= LATIN_SPELLING_FREE_LENGTH
                            || !crate::pinyin::segment::is_complete_pinyin_input(entry.name),
                        "不到 {LATIN_SPELLING_FREE_LENGTH} 个字母的拉丁名称不能是拼音：{}",
                        entry.name
                    );
                }
            }
            let symbols: Vec<&str> = entry.symbols().collect();
            assert!(
                !symbols.is_empty() && symbols.len() <= SYMBOLS_PER_NAME,
                "每个名称 1–{SYMBOLS_PER_NAME} 个符号：{}",
                entry.name
            );
            for symbol in symbols {
                assert!(
                    !symbol.is_empty() && symbol.chars().count() <= 8,
                    "符号非空、不超过 8 个字符：{} {symbol:?}",
                    entry.name
                );
                assert!(
                    symbol
                        .chars()
                        .all(|character| !crate::text::is_han(character)
                            && !character.is_whitespace()
                            && !character.is_control()
                            && character != '\u{FE0F}'),
                    "符号不含汉字、空白、控制字符和 U+FE0F：{} {symbol:?}",
                    entry.name
                );
                assert!(
                    pairs.insert((entry.name, symbol)),
                    "重复的（名称，符号）：{} {symbol}",
                    entry.name
                );
            }
        }
        let names: HashSet<&str> = entries.iter().map(|entry| entry.name).collect();
        assert_eq!(names.len(), entries.len(), "名称不重复");
    }

    #[test]
    fn whole_names_match_and_partial_spellings_do_not() {
        assert_eq!(texts(&quanpin("zhengfu")), vec!["±"]);
        assert_eq!(quanpin("zhengfu")[0].anchors, ["正负"]);
        assert!(quanpin("zhengf").is_empty());
        assert!(quanpin("zheng").is_empty());
        assert_eq!(texts(&quanpin("ZhengFu")), vec!["±"]);
        assert_eq!(texts(&quanpin("zheng'fu")), vec!["±"]);
        assert_eq!(texts(&quanpin("pingfangmi")), vec!["㎡", "m²"]);
        assert_eq!(texts(&quanpin("lifang")), vec!["m³", "³"]);
        // 一个字母不查；含字母以外的字符不查。
        assert!(quanpin("d").is_empty());
        assert!(quanpin("da1yu").is_empty());
    }

    #[test]
    fn number_names_offer_encoded_ordinal_symbols() {
        for (input, expected) in [
            ("yi", vec!["①", "⑴", "⒈"]),
            ("er", vec!["②", "⑵", "⒉"]),
            ("shi", vec!["⑩", "⑽", "⒑"]),
            ("shiyi", vec!["⑪", "⑾", "⒒"]),
            ("ershi", vec!["⑳", "⒇", "⒛"]),
            ("ershiyi", vec!["㉑"]),
            ("sanshi", vec!["㉚"]),
            ("sanshiwu", vec!["㉟"]),
            ("sanshiliu", vec!["㊱"]),
            ("wushi", vec!["㊿"]),
        ] {
            assert_eq!(texts(&quanpin(input)), expected, "{input}");
        }
        // 每个数字独立匹配；较小数字不能把以它开头的其他序号一并带出。
        assert!(quanpin("ersh").is_empty());
        assert!(!texts(&quanpin("ershi")).contains(&"㉑"));
        assert_eq!(texts(&quanpin("ss")), vec!["℃"]);
        assert!(quanpin("es").is_empty());
    }

    #[test]
    fn number_symbols_match_shuangpin_and_keypad_readings() {
        let xiaohe = profile(ShuangpinProfileKind::Xiaohe).unwrap();
        assert_eq!(
            texts(&query_mixed_symbols("erui", SchemeType::Shuangpin, xiaohe)),
            vec!["⑳", "⒇", "⒛"]
        );
        let rows = query_symbol_readings(&["ershi".to_owned(), "yi".to_owned()], &|_| true);
        assert_eq!(texts(&rows), vec!["⑳", "⒇", "⒛", "①", "⑴", "⒈"]);
        assert!(rows
            .iter()
            .filter(|row| row.anchors == ["一"])
            .all(|row| row.ambiguous && !row.unanchored));
    }

    #[test]
    fn initials_count_only_for_names_of_two_syllables_or_more() {
        let rows = quanpin("ssd");
        assert_eq!(texts(&rows), vec!["℃"]);
        assert_eq!(rows[0].anchors, ["摄氏度"]);
        assert!(!rows[0].unanchored && !rows[0].ambiguous);
        // 两个字母的首字母和单音节一样含糊（`ax` 爱心）。
        let ax = quanpin("ax");
        assert_eq!(texts(&ax), vec!["♥", "♡"]);
        assert!(ax[0].ambiguous && !ax[0].unanchored);
        // 度是单音节，首字母 d 本来就不到两个字母；pai 的首字母 p 也一样。
        let du = quanpin("du");
        assert_eq!(texts(&du), vec!["°"]);
        assert!(du[0].ambiguous && !du[0].unanchored);
        // 双拼不认首字母。
        let xiaohe = profile(ShuangpinProfileKind::Xiaohe).unwrap();
        assert!(query_mixed_symbols("ssd", SchemeType::Shuangpin, xiaohe).is_empty());
    }

    #[test]
    fn short_latin_names_must_not_spell_pinyin() {
        let rows = quanpin("gamma");
        assert_eq!(texts(&rows), vec!["γ", "Γ"]);
        assert_eq!(rows[0].anchors, ["gamma"]);
        assert!(rows[0].unanchored);
        // 三个字母、拆不成拼音的希腊字母名照收。
        for (name, expected) in [
            ("phi", ["φ", "Φ"]),
            ("psi", ["ψ", "Ψ"]),
            ("rho", ["ρ", "Ρ"]),
            ("tau", ["τ", "Τ"]),
        ] {
            let rows = quanpin(name);
            assert_eq!(texts(&rows), expected, "{name}");
            assert!(rows[0].unanchored, "{name}");
        }
        // pi、chi、eta 能拆成拼音，不作为拉丁名称；π 走中文名「派」，χ 走「凯」。
        assert!(quanpin("pi").is_empty());
        assert!(quanpin("chi").is_empty());
        assert!(quanpin("eta").is_empty());
        let pai = quanpin("pai");
        assert_eq!(texts(&pai), vec!["π"]);
        assert!(pai[0].ambiguous);
        let kai = quanpin("kai");
        assert_eq!(texts(&kai), vec!["χ", "Χ"]);
        assert!(kai[0].ambiguous && !kai[0].unanchored);
    }

    #[test]
    fn punctuation_names_offer_full_width_then_ascii_forms() {
        for (input, expected) in [
            ("douhao", vec!["，", ","]),
            ("juhao", vec!["。", "."]),
            ("wenhao", vec!["？", "?"]),
            ("maohao", vec!["：", ":"]),
            ("xiegang", vec!["／", "/"]),
            ("fanxiegang", vec!["\\"]),
            ("aite", vec!["@"]),
            ("xiaokuohao", vec!["（）", "()"]),
            ("jinghao", vec!["＃", "#"]),
        ] {
            assert_eq!(texts(&quanpin(input)), expected, "{input}");
        }
    }

    #[test]
    fn math_names_offer_set_logic_and_calculus_symbols() {
        for (input, expected) in [
            ("tidu", vec!["∇"]),
            ("dangqiejindang", vec!["⇔"]),
            ("shishuji", vec!["ℝ"]),
            ("baohanyu", vec!["⊆", "⊂"]),
            ("huanlujifen", vec!["∮"]),
            ("wufenzhiyi", vec!["⅕"]),
            ("nabla", vec!["∇"]),
        ] {
            assert_eq!(texts(&quanpin(input)), expected, "{input}");
        }
    }

    #[test]
    fn shuangpin_matches_the_normalised_quanpin() {
        let xiaohe = profile(ShuangpinProfileKind::Xiaohe).unwrap();
        let rows = query_mixed_symbols("vgfu", SchemeType::Shuangpin, xiaohe);
        assert_eq!(texts(&rows), vec!["±"]);
        assert_eq!(rows[0].item.pinyin, "vgfu");
        assert_eq!(
            texts(&query_mixed_symbols("gamma", SchemeType::Shuangpin, xiaohe)),
            vec!["γ", "Γ"]
        );
    }

    #[test]
    fn microsoft_semicolon_final_matches_a_symbol_name() {
        let microsoft = profile(ShuangpinProfileKind::Microsoft).unwrap();
        let rows = query_mixed_symbols("p;mi", SchemeType::Shuangpin, microsoft);
        assert_eq!(texts(&rows), vec!["㎡", "m²"]);
        assert_eq!(rows[0].item.pinyin, "p;mi");
        assert!(quanpin("p;mi").is_empty());
        let xiaohe = profile(ShuangpinProfileKind::Xiaohe).unwrap();
        assert!(query_mixed_symbols("p;mi", SchemeType::Shuangpin, xiaohe).is_empty());
    }

    #[test]
    fn other_schemes_have_no_symbol_rows() {
        let profile = crate::shuangpin::profile::default_profile();
        for scheme in [
            SchemeType::Wubi,
            SchemeType::JapaneseRomaji,
            SchemeType::Korean,
            SchemeType::Cantonese,
            SchemeType::Zhuyin,
            SchemeType::Stroke,
            SchemeType::Vietnamese,
            SchemeType::Tibetan,
        ] {
            assert!(
                query_mixed_symbols("zhengfu", scheme, profile).is_empty(),
                "{scheme:?}"
            );
        }
    }

    #[test]
    fn names_sharing_a_spelling_give_each_symbol_once() {
        // 西格马、西格玛同音，σ Σ 各出一次，两个名称都是锚定词，按表的先后。
        let rows = quanpin("xigema");
        assert_eq!(texts(&rows), vec!["σ", "Σ"]);
        assert_eq!(rows[0].anchors, ["西格马", "西格玛"]);
        // 首字母 dg 命中对钩、对勾、打钩、打勾，√ ✓ ✔ 各出一次，四个名称都记下。
        let rows = quanpin("dg");
        assert_eq!(texts(&rows), vec!["√", "✓", "✔"]);
        assert_eq!(rows[0].anchors, ["对钩", "对勾", "打钩", "打勾"]);
        assert!(rows[0].ambiguous && !rows[0].unanchored);
        // 合计不超过上限。
        let many = rows_for(
            table(),
            &(0..symbol_entries().len())
                .map(|index| (index, Match::Quanpin))
                .collect::<Vec<_>>(),
            "x",
        );
        assert_eq!(many.len(), MIXED_SYMBOL_LIMIT);
    }

    #[test]
    fn readings_match_whole_names_only() {
        let readings = vec!["zhengfu".to_owned(), "zhengbi".to_owned()];
        let rows = query_symbol_readings(&readings, &|_| true);
        assert_eq!(texts(&rows), vec!["±", "∝"]);
        assert!(query_symbol_readings(&["zheng".to_owned()], &|_| true).is_empty());
        // 首字母和拉丁名称都不认。
        assert!(query_symbol_readings(&["ssd".to_owned()], &|_| true).is_empty());
        assert!(query_symbol_readings(&["gamma".to_owned()], &|_| true).is_empty());
        // 几个读法给出同一个符号时只出一行，锚定词合在一起。
        let rows = query_symbol_readings(&["duigou".to_owned(), "dagou".to_owned()], &|_| true);
        assert_eq!(texts(&rows), vec!["√", "✓", "✔"]);
        assert_eq!(rows[0].anchors, ["对钩", "对勾", "打钩", "打勾"]);
        // `accept` 拒绝的读法不算。
        assert!(
            query_symbol_readings(&readings, &|reading| reading != "zhengfu")
                .iter()
                .all(|row| row.item.word != "±")
        );
    }
}
