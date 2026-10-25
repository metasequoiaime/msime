//! 符号模式（`S`）：按分组码（`xl` 希腊字母、`sx` 数学、`szq` 带圈数字、`jt` 箭头）或符号名（`alpha`、`infinity`）查符号。
//!
//! 两条查法共用一张编进引擎的表，都不读磁盘：
//!
//! - 分组码表是本项目自定的 `symbol_groups.tsv`（`编码<TAB>中文组名<TAB>符号...`），分组码与码位来自 rime-wanxiang 的 `wanxiang_symbols.yaml`（CC BY 4.0，署名与改动说明在 `resources/licenses/rime-wanxiang-CC-BY-4.0.txt`），中文组名与排序由本项目自定。
//! - LaTeX 命令名读官方短语表「LaTeX 符号」的数据源 `plugins/phrase_table/latex-symbols/symbols.tsv`（本项目自写，MIT；是 msime-dictionary `sources/symbols/latex-symbols.tsv` 的同步副本，见 `scripts/sync_symbol_tables.py`），编进引擎，六个平台不用启用插件也能用：`sum` 得 ∑、`leq` 得 ≤、`pi` 得 π Π、`mathbbr` 得 ℝ。命令名去掉反斜杠和花括号、转成小写，只差大小写的合成一行、小写的在前（`rightarrow` 得 → ⇒），与那个短语表的编码规则相同；`\S`、`\P` 这种只剩一个字母的不收，单个字母是分组码前缀。只在这里查，不进普通打字的混排，`pi`、`in`、`le` 这些短命令名在混排里会和拼音、英文撞上。
//! - 符号名复用混排那半边（`symbol_candidates`）的 `symbol_candidates.tsv`：`alpha` 得 α、`infinity` 得 ∞。
//!
//! 打出的整串是混排表里的拉丁名称（`phi`、`epsilon`）时，正文用的希腊字母（φ、ε）排在最前；接着是整串 LaTeX 命令名的符号（`\phi` 排出来是 ϕ），打出完整命令名就是要那个符号，所以它排在分组之前（`nu` 的 ν 排在以 `nu` 开头的 `num` 组之前）。分组码先认整串（`xl` 只出希腊字母小写），整串没有时按前缀给整组（`x` 出所有以 `x` 开头的组），组内按表的顺序；同一个符号只出一次。符号名在分组码之后给，同样只认整串。都不中时由会话补上原样上屏的兜底行。
//!
//! 行的来源是 `CandidateSource::Emoji`（与 emoji、颜文字、混排符号行同一个值）：选中时整段上屏、结束组字，不学习、不调频、不进个人上下文，也不能置顶、固定或删除。

use crate::types::{CandidateSource, WordItem};

/// 一次最多给出几行符号，分组码与符号名合计。与 Windows 正式版的 `query_symbols` 默认上限 200 一致：
/// 一个分组最多几百个符号（`kx` 康熙部首 214 个、`jt` 箭头 148 个），整组放进列表靠翻页看全。
pub const SYMBOL_MODE_LIMIT: usize = 200;

const TABLE: &str = include_str!("symbol_groups.tsv");
const LATEX_TABLE: &str =
    include_str!("../../../../plugins/phrase_table/latex-symbols/symbols.tsv");

/// 表里的一行。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SymbolGroup {
    pub code: &'static str,
    pub name: &'static str,
    /// 空格分隔的符号。
    pub symbols: &'static str,
}

impl SymbolGroup {
    /// 这个组的符号，按表的顺序。
    pub fn symbols(&self) -> impl Iterator<Item = &'static str> {
        self.symbols.split(' ')
    }
}

/// 解析整张表，跳过空行和 `#` 注释；列数不对的行也跳过（表检查会把它们报出来）。
fn parse_groups(text: &'static str) -> Vec<SymbolGroup> {
    text.lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| {
            let mut fields = line.split('\t');
            let (Some(code), Some(name), Some(symbols), None) =
                (fields.next(), fields.next(), fields.next(), fields.next())
            else {
                return None;
            };
            Some(SymbolGroup {
                code,
                name,
                symbols,
            })
        })
        .collect()
}

/// 表里的全部组，按文件的先后。
pub fn groups() -> &'static [SymbolGroup] {
    static GROUPS: std::sync::OnceLock<Vec<SymbolGroup>> = std::sync::OnceLock::new();
    GROUPS.get_or_init(|| parse_groups(TABLE))
}

/// LaTeX 命令名表（`\命令名<TAB>U+码位`）按符号模式的编码合并：去掉 `\` 和花括号、转成小写，同一个编码的符号按表的先后，大写开头的那条在表里排在小写的后面。码位解析不了的行跳过，表本身由 `gen_latex_symbols.py --check` 核对。
fn parse_latex(text: &str) -> Vec<(String, Vec<String>)> {
    let mut entries: Vec<(String, Vec<String>)> = Vec::new();
    for line in text.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((name, point)) = line.split_once('\t') else {
            continue;
        };
        let Some(symbol) = point
            .strip_prefix("U+")
            .and_then(|hex| u32::from_str_radix(hex, 16).ok())
            .and_then(char::from_u32)
        else {
            continue;
        };
        let code: String = name
            .chars()
            .filter(|character| character.is_ascii_alphabetic())
            .map(|character| character.to_ascii_lowercase())
            .collect();
        if code.len() < 2 {
            continue;
        }
        match entries.iter_mut().find(|(known, _)| *known == code) {
            Some((_, symbols)) => symbols.push(symbol.to_string()),
            None => entries.push((code, vec![symbol.to_string()])),
        }
    }
    entries
}

/// LaTeX 命令名（符号模式的编码）→ 符号。
fn latex_names() -> &'static std::collections::HashMap<String, Vec<String>> {
    static NAMES: std::sync::OnceLock<std::collections::HashMap<String, Vec<String>>> =
        std::sync::OnceLock::new();
    NAMES.get_or_init(|| parse_latex(LATEX_TABLE).into_iter().collect())
}

/// 编码只认 ASCII 字母，统一转小写；含别的字符时返回空串（查不到任何符号）。
fn normalize(code: &str) -> String {
    if code.is_empty() || !code.bytes().all(|byte| byte.is_ascii_alphabetic()) {
        return String::new();
    }
    code.to_ascii_lowercase()
}

/// 符号模式的行：先按整串拉丁名称和 LaTeX 命令名出，再按整串分组码出、未命中时按分组码前缀出，最后按其余符号名（中文名称的全拼、首字母）整串出。`code` 是去掉入口字母之后的输入。
pub fn query_symbol_mode(code: &str, limit: usize) -> Vec<WordItem> {
    let code = normalize(code);
    if code.is_empty() || limit == 0 {
        return Vec::new();
    }
    let mut rows: Vec<WordItem> = Vec::new();
    let mut push = |symbol: &str| {
        if rows.len() == limit || rows.iter().any(|row| row.word == symbol) {
            return;
        }
        rows.push(WordItem::new(
            code.clone(),
            symbol,
            0,
            CandidateSource::Emoji,
            "",
        ));
    };
    for symbol in crate::local::symbol_candidates::latin_name_symbols(&code) {
        push(symbol);
    }
    for symbol in latex_names().get(code.as_str()).into_iter().flatten() {
        push(symbol);
    }
    let groups = groups();
    // 有整串分组码时只返回该组，输入尚未完整时才按前缀列出相关组。
    let exact = groups.iter().any(|group| group.code == code);
    for group in groups {
        if (exact && group.code == code) || (!exact && group.code.starts_with(&code)) {
            for symbol in group.symbols() {
                push(symbol);
            }
        }
    }
    // 符号名：`alpha`、`infinity`、`gamma` 这些名称本身就是字母，与分组码共用同一段输入。
    for row in crate::local::symbol_candidates::query_mixed_symbols(
        &code,
        crate::types::SchemeType::Quanpin,
        crate::shuangpin::profile::default_profile(),
    ) {
        push(&row.item.word);
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn texts(rows: &[WordItem]) -> Vec<&str> {
        rows.iter().map(|row| row.word.as_str()).collect()
    }

    #[test]
    fn the_table_follows_its_rules() {
        let groups = groups();
        assert_eq!(groups.len(), 87, "分组码个数");
        let mut codes = HashSet::new();
        let mut symbols = 0;
        for group in groups {
            assert!(
                group.code.bytes().all(|byte| byte.is_ascii_lowercase()),
                "编码只含小写字母：{}",
                group.code
            );
            assert!(codes.insert(group.code), "编码不重复：{}", group.code);
            assert!(
                !group.name.is_empty() && group.name.chars().all(crate::text::is_han),
                "组名只含汉字：{} {}",
                group.code,
                group.name
            );
            let count = group.symbols().count();
            assert!(count > 0, "组里至少一个符号：{}", group.code);
            for symbol in group.symbols() {
                symbols += 1;
                assert!(
                    !symbol.is_empty()
                        && !symbol
                            .chars()
                            .any(|character| character.is_whitespace() || character.is_control())
                        && symbol != "\u{FE0F}",
                    "符号非空、不含空白和控制字符：{} {symbol:?}",
                    group.code
                );
            }
        }
        assert_eq!(symbols, 3929, "符号总数");
    }

    #[test]
    fn group_codes_give_their_whole_group() {
        let xl = query_symbol_mode("xl", SYMBOL_MODE_LIMIT);
        let expected: Vec<_> = groups()
            .iter()
            .find(|group| group.code == "xl")
            .unwrap()
            .symbols()
            .collect();
        assert_eq!(texts(&xl), expected);
        let sx = query_symbol_mode("SX", SYMBOL_MODE_LIMIT);
        assert!(sx.iter().any(|row| row.word == "∑"), "{:?}", texts(&sx));
        let jt = query_symbol_mode("jt", SYMBOL_MODE_LIMIT);
        assert!(jt.iter().any(|row| row.word == "→"), "{:?}", texts(&jt));
        let szq = query_symbol_mode("szq", SYMBOL_MODE_LIMIT);
        assert_eq!(texts(&szq)[0], "⓪", "{:?}", texts(&szq));
        assert!(query_symbol_mode("", SYMBOL_MODE_LIMIT).is_empty());
        assert!(query_symbol_mode("x'1", SYMBOL_MODE_LIMIT).is_empty());
    }

    #[test]
    fn incomplete_group_code_lists_multiple_groups() {
        let rows = query_symbol_mode("x", SYMBOL_MODE_LIMIT);
        let values = texts(&rows);
        assert!(values.contains(&"α"));
        assert!(values.contains(&"Α"));
        assert_eq!(values.iter().filter(|&&value| value == "α").count(), 1);
    }

    #[test]
    fn symbol_names_give_their_symbol() {
        let alpha = query_symbol_mode("alpha", SYMBOL_MODE_LIMIT);
        assert_eq!(texts(&alpha)[0], "α", "{:?}", texts(&alpha));
        let infinity = query_symbol_mode("infinity", SYMBOL_MODE_LIMIT);
        assert_eq!(texts(&infinity), vec!["∞"], "{:?}", texts(&infinity));
        // 符号名在分组码之后，前缀命中的组不会把 `alpha` 的 α 挤掉。
        let exact = query_symbol_mode("xl", SYMBOL_MODE_LIMIT);
        assert_eq!(exact.iter().filter(|row| row.word == "α").count(), 1);
    }

    #[test]
    fn the_latex_table_merges_case_variants_under_one_code() {
        let entries = parse_latex(LATEX_TABLE);
        // 395 行的表去掉注释、合并大小写和 \S、\P 之后的编码数。
        assert!(entries.len() > 250, "{}", entries.len());
        let codes: std::collections::HashSet<&str> =
            entries.iter().map(|(code, _)| code.as_str()).collect();
        assert_eq!(codes.len(), entries.len(), "编码不重复");
        for (code, symbols) in &entries {
            assert!(
                code.len() >= 2 && code.bytes().all(|byte| byte.is_ascii_lowercase()),
                "编码全小写、至少两个字母：{code}"
            );
            assert!(
                (1..=2).contains(&symbols.len()),
                "同一编码最多大小写两行：{code}"
            );
        }
        let symbols = |code: &str| latex_names()[code].clone();
        assert_eq!(symbols("gamma"), ["γ", "Γ"]);
        assert_eq!(symbols("rightarrow"), ["→", "⇒"]);
        assert_eq!(symbols("mathbbr"), ["ℝ"]);
        assert!(!latex_names().contains_key("s"));
    }

    #[test]
    fn latex_names_give_their_symbol_first() {
        for (code, expected) in [
            ("sum", "∑"),
            ("int", "∫"),
            ("infty", "∞"),
            ("leq", "≤"),
            ("neq", "≠"),
            ("partial", "∂"),
            ("nabla", "∇"),
            ("forall", "∀"),
            ("subseteq", "⊆"),
            ("in", "∈"),
            ("pi", "π"),
            ("mu", "μ"),
            ("eta", "η"),
            ("varphi", "φ"),
            ("mathbbr", "ℝ"),
            ("Rightarrow", "→"),
        ] {
            let rows = query_symbol_mode(code, SYMBOL_MODE_LIMIT);
            assert_eq!(
                texts(&rows).first(),
                Some(&expected),
                "{code}: {:?}",
                texts(&rows)
            );
        }
        // 大写开头的命令名转成小写后与小写的同一行，两种箭头都在。
        assert_eq!(
            texts(&query_symbol_mode("Rightarrow", SYMBOL_MODE_LIMIT)),
            vec!["→", "⇒"]
        );
        // 命令名排在以它开头的分组之前：`nu` 先给 ν，再给 `num` 组的 №。
        assert_eq!(
            texts(&query_symbol_mode("nu", SYMBOL_MODE_LIMIT))[..2],
            ["ν", "№"]
        );
        // 正文用的希腊字母排在 LaTeX 的字形变体之前：`phi` 先给 φ，再给 `\phi` 的 ϕ。
        assert_eq!(
            texts(&query_symbol_mode("phi", SYMBOL_MODE_LIMIT))[..3],
            ["φ", "Φ", "ϕ"]
        );
        // 只认整串：没打完的命令名不给。
        assert!(!texts(&query_symbol_mode("subse", SYMBOL_MODE_LIMIT)).contains(&"⊆"));
    }

    #[test]
    fn every_row_commits_whole_and_is_not_edited() {
        for row in query_symbol_mode("xl", SYMBOL_MODE_LIMIT) {
            assert_eq!(row.source, CandidateSource::Emoji);
            assert!(row.canonical_pinyin.is_empty());
        }
    }
}
