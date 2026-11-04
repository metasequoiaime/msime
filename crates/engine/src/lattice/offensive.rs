//! 不拿来拼整句的粗俗词，以及候选排位用的轻度粗口表和英文粗俗词表。词表、豁免写法和收词标准见各词表文件的文件头，排位规则见 `offensive_placement`。
//!
//! 词网格给多字词每个音节 `PHRASE_LENGTH_BONUS` 的奖励，一个只有几千权重的两字词也能压过同码的两个常用单字，所以一个罕见的粗俗词会被拼进用户本来想打的句子当首选（九键 我们走路去 解成「我们有巨乳」）。含粗俗词的长词条也一样（个鸡巴、吃屎了、是傻逼），所以按子串查：词里任何一段在表里就算命中，除非这一段被一条豁免写法整个盖住（水乳交融 里的 乳交、口交换机 里的 口交）。整句是猜出来的，用户没打这个词；命中的词因此不进词网格（整段输入恰好是这个词时除外，那时它就是这串输入对应的词库行），也不进纠错整句（纠错边的词来自另一串读音，盖住整段也不用）和上屏后的联想。词库行照样在候选里，只是排位另由 `offensive_placement` 管。

use std::cmp::Ordering;

const WORDS: &str = include_str!("offensive_words.txt");
const MILD_WORDS: &str = include_str!("mild_slang_words.txt");
const ENGLISH_WORDS: &str = include_str!("offensive_english_words.txt");

/// 豁免写法在词表文件里以这个字符开头。
const EXEMPT_MARK: u8 = b'!';

/// 编译期从一份词表文件解析出来的表：表里的词和豁免写法（去掉开头的 `!`）各按字节序排好，查表用二分查找。运行时不建表、不分配内存，第一次按键不付初始化的开销，数分配次数的测试也不受先后顺序影响。
struct WordTable<const LISTED: usize, const EXEMPT: usize> {
    listed: [&'static str; LISTED],
    exempt: [&'static str; EXEMPT],
    /// 表里最长的词有几个字，按子串查时每个起点最多看这么长。
    longest: usize,
}

impl<const LISTED: usize, const EXEMPT: usize> WordTable<LISTED, EXEMPT> {
    const fn parse(source: &'static str) -> Self {
        let listed = parse_entries::<LISTED>(source, false);
        Self {
            longest: longest(&listed),
            listed,
            exempt: parse_entries::<EXEMPT>(source, true),
        }
    }

    /// `word` 本身或其中任何一段在表里，而且这一段没有被豁免写法盖住。表里只有两字以上的词，单字（UTF-8 不超过 4 字节）不查表，词网格里大多数边是单字。
    fn matches_within(&self, word: &str) -> bool {
        if word.len() <= 4 {
            return false;
        }
        for (start, _) in word.char_indices() {
            let rest = &word[start..];
            for (count, (offset, ch)) in rest.char_indices().enumerate() {
                if count >= self.longest {
                    break;
                }
                let end = offset + ch.len_utf8();
                if count >= 1
                    && self.listed.binary_search(&&rest[..end]).is_ok()
                    && !self.exempted(word, start, start + end)
                {
                    return true;
                }
            }
        }
        false
    }

    /// 整句 `sentence` 由 `words` 依次拼成时，句子里是否含表里的词：某个词本身含（规则同 [`Self::matches_within`]），或者连续几个词拼起来恰好是表里的词（别 + 鸡 + 巴）。只盖住某个词一部分的跨边界命中不算：微信群 + 交流 里的 群交、我 + 操作 里的 我操 是两个无关的词碰在一起，词网格按边查也不拦它们。`words` 拼不回 `sentence` 时退回按整串子串查。
    fn matches_in_sentence(&self, words: &[String], sentence: &str) -> bool {
        let mut rest = sentence;
        for word in words {
            match rest.strip_prefix(word.as_str()) {
                Some(tail) => rest = tail,
                None => return self.matches_within(sentence),
            }
        }
        if !rest.is_empty() {
            return self.matches_within(sentence);
        }
        if words.iter().any(|word| self.matches_within(word)) {
            return true;
        }
        let mut start = 0;
        for (index, first) in words.iter().enumerate() {
            let mut end = start;
            let mut chars = 0;
            for (count, word) in words[index..].iter().enumerate() {
                end += word.len();
                chars += word.chars().count();
                if chars > self.longest {
                    break;
                }
                if count >= 1
                    && self.listed.binary_search(&&sentence[start..end]).is_ok()
                    && !self.exempted(sentence, start, end)
                {
                    return true;
                }
            }
            start += first.len();
        }
        false
    }

    /// `word[start..end]` 这一段命中了词表时，有没有一条豁免写法在 `word` 里出现并整个盖住这一段。
    fn exempted(&self, word: &str, start: usize, end: usize) -> bool {
        self.exempt.iter().any(|pattern| {
            word.match_indices(pattern)
                .any(|(at, _)| at <= start && end <= at + pattern.len())
        })
    }

    /// 整个 `word` 不分大小写恰好是表里的词。英文表的词全是小写 ASCII。
    fn matches_whole_ignoring_ascii_case(&self, word: &str) -> bool {
        self.listed
            .binary_search_by(|probe| {
                probe
                    .bytes()
                    .cmp(word.bytes().map(|byte| byte.to_ascii_lowercase()))
            })
            .is_ok()
    }
}

/// 不拿来拼整句、也不进首屏的粗俗词（`offensive_words.txt`）。
static VULGAR: WordTable<{ count_entries(WORDS, false) }, { count_entries(WORDS, true) }> =
    WordTable::parse(WORDS);
/// 流行的轻度粗口（`mild_slang_words.txt`）：照样拼句、照样当 26 键的首选，只是不进九键、14 键歧义码的首屏。
static MILD: WordTable<{ count_entries(MILD_WORDS, false) }, { count_entries(MILD_WORDS, true) }> =
    WordTable::parse(MILD_WORDS);
/// 英文粗俗词和蔑称（`offensive_english_words.txt`），整词比较，不分大小写。
static ENGLISH: WordTable<
    { count_entries(ENGLISH_WORDS, false) },
    { count_entries(ENGLISH_WORDS, true) },
> = WordTable::parse(ENGLISH_WORDS);

/// 从 `rest` 开头取下一个条目：跳过空行和 `#` 开头的行，去掉首尾空白。返回这个条目和它之后剩下的部分。
const fn next_entry(mut rest: &'static [u8]) -> Option<(&'static [u8], &'static [u8])> {
    while !rest.is_empty() {
        let mut end = 0;
        while end < rest.len() && rest[end] != b'\n' {
            end += 1;
        }
        let (line, tail) = rest.split_at(end);
        rest = if tail.is_empty() {
            tail
        } else {
            tail.split_at(1).1
        };
        let line = line.trim_ascii();
        if !line.is_empty() && line[0] != b'#' {
            return Some((line, rest));
        }
    }
    None
}

/// 词表文件 `source` 里有几个词（`exempt` 为假）或几条豁免写法（`exempt` 为真）。
const fn count_entries(source: &'static str, exempt: bool) -> usize {
    let mut count = 0;
    let mut rest = source.as_bytes();
    while let Some((entry, tail)) = next_entry(rest) {
        rest = tail;
        if (entry[0] == EXEMPT_MARK) == exempt {
            count += 1;
        }
    }
    count
}

/// 按字节比较两个串，与运行时 `str` 的 `Ord` 一致，二分查找因此能用编译期排出的顺序。
const fn compare(a: &[u8], b: &[u8]) -> Ordering {
    let mut i = 0;
    while i < a.len() && i < b.len() {
        if a[i] != b[i] {
            return if a[i] < b[i] {
                Ordering::Less
            } else {
                Ordering::Greater
            };
        }
        i += 1;
    }
    if a.len() < b.len() {
        Ordering::Less
    } else if a.len() > b.len() {
        Ordering::Greater
    } else {
        Ordering::Equal
    }
}

/// 取出 `source` 里的全部词或全部豁免写法，按字节序插入排序。重复的条目和不是 UTF-8 的条目让编译失败。
const fn parse_entries<const N: usize>(source: &'static str, exempt: bool) -> [&'static str; N] {
    let mut out = [""; N];
    let mut len = 0;
    let mut rest = source.as_bytes();
    while let Some((entry, tail)) = next_entry(rest) {
        rest = tail;
        if (entry[0] == EXEMPT_MARK) != exempt {
            continue;
        }
        let text = if exempt { entry.split_at(1).1 } else { entry };
        let Ok(text) = std::str::from_utf8(text) else {
            panic!("粗俗词表里有不是 UTF-8 的条目");
        };
        let mut at = len;
        while at > 0 {
            match compare(out[at - 1].as_bytes(), text.as_bytes()) {
                Ordering::Less => break,
                Ordering::Equal => panic!("粗俗词表里有重复的条目"),
                Ordering::Greater => {
                    out[at] = out[at - 1];
                    at -= 1;
                }
            }
        }
        out[at] = text;
        len += 1;
    }
    out
}

/// 最长的条目有几个字（数 UTF-8 里不是续字节的字节）。
const fn longest(entries: &[&str]) -> usize {
    let mut longest = 0;
    let mut i = 0;
    while i < entries.len() {
        let bytes = entries[i].as_bytes();
        let mut chars = 0;
        let mut j = 0;
        while j < bytes.len() {
            if bytes[j] & 0xC0 != 0x80 {
                chars += 1;
            }
            j += 1;
        }
        if chars > longest {
            longest = chars;
        }
        i += 1;
    }
    longest
}

/// `word` 是否含粗俗词：它本身或其中任何一段在 `offensive_words.txt` 里，而且这一段没有被豁免写法盖住。
pub(crate) fn is_offensive(word: &str) -> bool {
    VULGAR.matches_within(word)
}

/// `word` 是否含 `mild_slang_words.txt` 里的轻度粗口，按子串查，规则与 [`is_offensive`] 相同。
pub(crate) fn is_mild_slang(word: &str) -> bool {
    MILD.matches_within(word)
}

/// 词网格整句 `sentence`（由 `words` 依次拼成）是否含粗俗词，跨词边界的命中只在恰好由整词拼成时算，见 `WordTable::matches_in_sentence`。
pub(crate) fn is_offensive_sentence(words: &[String], sentence: &str) -> bool {
    VULGAR.matches_in_sentence(words, sentence)
}

/// 词网格整句是否含轻度粗口，规则与 [`is_offensive_sentence`] 相同。
pub(crate) fn is_mild_slang_sentence(words: &[String], sentence: &str) -> bool {
    MILD.matches_in_sentence(words, sentence)
}

/// 英文词 `word`（显示形式或小写的词都行）是否是 `offensive_english_words.txt` 里的粗俗词或蔑称。整词比较：英文按子串查会误伤 cocktail、Scunthorpe 这类词。
pub(crate) fn is_offensive_english(word: &str) -> bool {
    ENGLISH.matches_whole_ignoring_ascii_case(word)
}

/// 词网格里 `start..end` 这条词库行的边可不可以用 `word`：含粗俗词的词只在独占整段输入（`0..len`）时成边，这时整句就是这个词本身，与它的词库行相同。纠错边不用它，见 `decode_typo_on_graph_with_policy`。
pub(crate) fn admits_edge(word: &str, start: usize, end: usize, len: usize) -> bool {
    (start == 0 && end == len) || !is_offensive(word)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    /// 用标准库逐行解析词表文件，与编译期的 `parse_entries` 互相核对。
    fn entries(source: &'static str) -> impl Iterator<Item = &'static str> {
        source
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
    }

    /// 编译期的表与逐行解析的结果相同、排好了序，豁免写法都盖住表里的某个词而自己不在表里。
    fn check_table<const L: usize, const E: usize>(table: &WordTable<L, E>, source: &'static str) {
        let mut listed: Vec<&str> = entries(source)
            .filter(|entry| !entry.starts_with(char::from(EXEMPT_MARK)))
            .collect();
        listed.sort_unstable();
        assert_eq!(table.listed.as_slice(), listed.as_slice());
        let mut exempt: Vec<&str> = entries(source)
            .filter_map(|entry| entry.strip_prefix(char::from(EXEMPT_MARK)))
            .collect();
        exempt.sort_unstable();
        assert_eq!(table.exempt.as_slice(), exempt.as_slice());
        let longest = listed.iter().map(|word| word.chars().count()).max();
        assert_eq!(Some(table.longest), longest);
        for pattern in &table.exempt {
            assert!(
                table.listed.binary_search(pattern).is_err(),
                "{pattern} 既在表里又是豁免写法"
            );
            assert!(
                table.listed.iter().any(|word| pattern.contains(word)),
                "{pattern} 不含表里的词，豁免它没有意义"
            );
        }
    }

    #[test]
    fn chinese_lists_hold_only_multi_character_han_words_once_each() {
        let mut seen = HashSet::new();
        for source in [WORDS, MILD_WORDS] {
            for entry in entries(source) {
                let text = entry.strip_prefix(char::from(EXEMPT_MARK)).unwrap_or(entry);
                assert!(text.chars().count() >= 2, "{entry} 不到两个字");
                assert!(crate::text::is_all_han(text), "{entry} 不全是汉字");
                // 同一个词不能同时在两张表里：一个词只按一档处理。
                assert!(seen.insert(entry), "{entry} 重复");
            }
        }
        assert_eq!(
            seen.len(),
            VULGAR.listed.len() + VULGAR.exempt.len() + MILD.listed.len() + MILD.exempt.len()
        );
    }

    #[test]
    fn the_english_list_holds_lowercase_ascii_words_once_each() {
        let mut seen = HashSet::new();
        for entry in entries(ENGLISH_WORDS) {
            assert!(
                entry.len() >= 2 && entry.bytes().all(|byte| byte.is_ascii_lowercase()),
                "{entry} 不是两个字母以上的小写 ASCII 词"
            );
            assert!(seen.insert(entry), "{entry} 重复");
        }
        assert!(ENGLISH.exempt.is_empty(), "英文表整词比较，用不着豁免写法");
    }

    #[test]
    fn compile_time_tables_match_the_files_and_are_sorted() {
        check_table(&VULGAR, WORDS);
        check_table(&MILD, MILD_WORDS);
        check_table(&ENGLISH, ENGLISH_WORDS);
    }

    #[test]
    fn offensive_words_are_recognised_and_ordinary_words_are_not() {
        assert!(is_offensive("巨乳"));
        assert!(is_offensive("他妈的"));
        for word in [
            "走路", "我们", "乳", "巨", "邮局", "坚韧", "阴道", "强奸", "",
        ] {
            assert!(!is_offensive(word), "{word}");
        }
    }

    #[test]
    fn a_longer_word_containing_an_offensive_word_is_offensive() {
        for word in [
            "个鸡巴",
            "净鸡巴",
            "吃屎了",
            "去吃屎",
            "是傻逼",
            "都是傻逼",
            "我们有巨乳",
            "去他妈的",
            "约炮吗",
            "童颜巨乳",
            "巨乳模特",
            "性奴调教",
        ] {
            assert!(is_offensive(word), "{word}");
        }
    }

    #[test]
    fn an_exempt_pattern_releases_only_the_part_it_covers() {
        for word in [
            "水乳交融",
            "口交换机",
            "多口交换机",
            "港口交货",
            "人口爆炸",
            "众口交赞",
            "内射函数",
            "马奶子酒",
            "奶子酒",
            "酸奶子",
            "奸夫淫妇",
            "狗吃屎",
            "摔了个狗吃屎",
            "狗改不了吃屎",
            "巨乳症",
            "巨乳缩小术",
            "性奴隶",
            "荡妇羞辱",
        ] {
            assert!(!is_offensive(word), "{word}");
        }
        // 豁免写法只放行它盖住的那一段，同一个词里别处的命中照样算。
        assert!(is_offensive("水乳交融傻逼"));
        assert!(is_offensive("口交换机口交"));
    }

    #[test]
    fn mild_slang_is_its_own_tier() {
        for word in ["牛逼", "真牛逼", "卧槽", "屌丝"] {
            assert!(is_mild_slang(word), "{word}");
            assert!(!is_offensive(word), "{word}");
        }
        for word in ["牛排", "尼玛", "笨蛋", "傻逼"] {
            assert!(!is_mild_slang(word), "{word}");
        }
    }

    fn split(words: &[&str]) -> (Vec<String>, String) {
        (
            words.iter().map(|word| (*word).to_owned()).collect(),
            words.concat(),
        )
    }

    #[test]
    fn a_sentence_is_offensive_only_through_whole_words() {
        for words in [
            &["别", "鸡", "巴"][..],
            &["别", "鸡巴"],
            &["是", "傻逼"],
            &["我们", "有", "巨乳"],
            &["个鸡巴", "啊"],
        ] {
            let (words, sentence) = split(words);
            assert!(is_offensive_sentence(&words, &sentence), "{sentence}");
        }
        for words in [
            &["微信群", "交流"][..],
            &["加", "群", "交流"],
            &["水", "乳", "交融"],
            &["他", "妈妈", "的"],
        ] {
            let (words, sentence) = split(words);
            assert!(!is_offensive_sentence(&words, &sentence), "{sentence}");
        }
        // 拼不回整句时按整串查。
        assert!(is_offensive_sentence(&["别".to_owned()], "别鸡巴"));
        assert!(is_offensive_sentence(&[], "别鸡巴"));
    }

    #[test]
    fn mild_slang_in_a_sentence_follows_the_same_rule() {
        let (words, sentence) = split(&["我", "操作"]);
        assert!(!is_mild_slang_sentence(&words, &sentence));
        let (words, sentence) = split(&["不必", "为", "我", "操心"]);
        assert!(!is_mild_slang_sentence(&words, &sentence));
        let (words, sentence) = split(&["真", "牛", "逼"]);
        assert!(is_mild_slang_sentence(&words, &sentence));
        assert!(!is_mild_slang("不必为我操心"));
    }

    #[test]
    fn english_words_match_whole_and_ignore_case() {
        for word in [
            "Negro", "negro", "NEGRO", "cumshot", "gangbang", "shitty", "shitting", "nigger",
        ] {
            assert!(is_offensive_english(word), "{word}");
        }
        for word in [
            "negroni",
            "cocktail",
            "Scunthorpe",
            "assess",
            "semen",
            "dick",
            "",
            "s",
        ] {
            assert!(!is_offensive_english(word), "{word}");
        }
    }

    #[test]
    fn an_offensive_word_is_an_edge_only_when_it_spans_the_whole_input() {
        assert!(admits_edge("巨乳", 0, 2, 2));
        assert!(!admits_edge("巨乳", 3, 5, 5));
        assert!(!admits_edge("巨乳", 0, 2, 3));
        assert!(!admits_edge("吃屎了", 2, 5, 5));
        assert!(admits_edge("水乳交融", 2, 6, 6));
        assert!(admits_edge("走路", 2, 4, 5));
    }
}
