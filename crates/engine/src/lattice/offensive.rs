//! 不拿来拼整句的粗俗词。词表、收词标准和为什么只管拼句见 `offensive_words.txt` 的文件头。
//!
//! 词网格给多字词每个音节 `PHRASE_LENGTH_BONUS` 的奖励，一个只有几千权重的两字词也能压过同码的两个常用单字，所以一个罕见的粗俗词会被拼进用户本来想打的句子当首选（九键 我们走路去 解成「我们有巨乳」）。整句是猜出来的，用户没打这个词；表里的词因此不进词网格（整段输入恰好是这个词时除外，那就是它的词库读音），也不进纠错整句和上屏后的联想。词库行不受影响。

use std::collections::HashSet;
use std::sync::OnceLock;

const WORDS: &str = include_str!("offensive_words.txt");

fn words() -> &'static HashSet<&'static str> {
    static SET: OnceLock<HashSet<&'static str>> = OnceLock::new();
    SET.get_or_init(|| {
        WORDS
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .collect()
    })
}

/// `word` 是否在粗俗词表里。表里只有两字以上的词，单字（UTF-8 不超过 4 字节）不查表，词网格里大多数边是单字。
pub(crate) fn is_offensive(word: &str) -> bool {
    word.len() > 4 && words().contains(word)
}

/// 词网格里 `start..end` 这条边可不可以用 `word`：粗俗词只在独占整段输入（`0..len`）时成边，这时整句就是这个词本身，与它的词库行相同。
pub(crate) fn admits_edge(word: &str, start: usize, end: usize, len: usize) -> bool {
    (start == 0 && end == len) || !is_offensive(word)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_holds_only_multi_character_han_words_once_each() {
        let mut seen = HashSet::new();
        for line in WORDS.lines().map(str::trim) {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            assert!(line.chars().count() >= 2, "{line} 不到两个字");
            assert!(crate::text::is_all_han(line), "{line} 不全是汉字");
            assert!(seen.insert(line), "{line} 重复");
        }
        assert_eq!(seen.len(), words().len());
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
    fn an_offensive_word_is_an_edge_only_when_it_spans_the_whole_input() {
        assert!(admits_edge("巨乳", 0, 2, 2));
        assert!(!admits_edge("巨乳", 3, 5, 5));
        assert!(!admits_edge("巨乳", 0, 2, 3));
        assert!(admits_edge("走路", 2, 4, 5));
    }
}
