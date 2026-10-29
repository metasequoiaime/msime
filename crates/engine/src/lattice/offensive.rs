//! 不拿来拼整句的粗俗词。词表、豁免写法、收词标准和为什么只管拼句见 `offensive_words.txt` 的文件头。
//!
//! 词网格给多字词每个音节 `PHRASE_LENGTH_BONUS` 的奖励，一个只有几千权重的两字词也能压过同码的两个常用单字，所以一个罕见的粗俗词会被拼进用户本来想打的句子当首选（九键 我们走路去 解成「我们有巨乳」）。含粗俗词的长词条也一样（个鸡巴、吃屎了、是傻逼），所以按子串查：词里任何一段在表里就算命中，除非这一段被一条豁免写法整个盖住（水乳交融 里的 乳交、口交换机 里的 口交）。整句是猜出来的，用户没打这个词；命中的词因此不进词网格（整段输入恰好是这个词时除外，那时它就是这串输入对应的词库行），也不进纠错整句和上屏后的联想。词库行不受影响。

use std::collections::HashSet;
use std::sync::OnceLock;

const WORDS: &str = include_str!("offensive_words.txt");

/// 豁免写法在词表文件里以这个字符开头。
const EXEMPT_MARK: char = '!';

struct Lists {
    words: HashSet<&'static str>,
    /// 表里最长的词有几个字，按子串查时每个起点最多看这么长。
    longest: usize,
    exempt: Vec<&'static str>,
}

fn entries() -> impl Iterator<Item = &'static str> {
    WORDS
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
}

fn lists() -> &'static Lists {
    static LISTS: OnceLock<Lists> = OnceLock::new();
    LISTS.get_or_init(|| {
        let mut words = HashSet::new();
        let mut exempt = Vec::new();
        for entry in entries() {
            match entry.strip_prefix(EXEMPT_MARK) {
                Some(pattern) => exempt.push(pattern),
                None => {
                    words.insert(entry);
                }
            }
        }
        let longest = words
            .iter()
            .map(|word| word.chars().count())
            .max()
            .unwrap_or(0);
        Lists {
            words,
            longest,
            exempt,
        }
    })
}

/// `word` 是否含粗俗词：它本身或其中任何一段在表里，而且这一段没有被豁免写法盖住。表里只有两字以上的词，单字（UTF-8 不超过 4 字节）不查表，词网格里大多数边是单字。
pub(crate) fn is_offensive(word: &str) -> bool {
    if word.len() <= 4 {
        return false;
    }
    let lists = lists();
    for (start, _) in word.char_indices() {
        let rest = &word[start..];
        for (count, (offset, ch)) in rest.char_indices().enumerate() {
            if count >= lists.longest {
                break;
            }
            let end = offset + ch.len_utf8();
            if count >= 1
                && lists.words.contains(&rest[..end])
                && !exempted(lists, word, start, start + end)
            {
                return true;
            }
        }
    }
    false
}

/// `word[start..end]` 这一段命中了词表时，有没有一条豁免写法在 `word` 里出现并整个盖住这一段。
fn exempted(lists: &Lists, word: &str, start: usize, end: usize) -> bool {
    lists.exempt.iter().any(|pattern| {
        word.match_indices(pattern)
            .any(|(at, _)| at <= start && end <= at + pattern.len())
    })
}

/// 词网格里 `start..end` 这条边可不可以用 `word`：含粗俗词的词只在独占整段输入（`0..len`）时成边，这时整句就是这个词本身，与它的词库行相同。
pub(crate) fn admits_edge(word: &str, start: usize, end: usize, len: usize) -> bool {
    (start == 0 && end == len) || !is_offensive(word)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_holds_only_multi_character_han_words_once_each() {
        let mut seen = HashSet::new();
        for entry in entries() {
            let text = entry.strip_prefix(EXEMPT_MARK).unwrap_or(entry);
            assert!(text.chars().count() >= 2, "{entry} 不到两个字");
            assert!(crate::text::is_all_han(text), "{entry} 不全是汉字");
            assert!(seen.insert(entry), "{entry} 重复");
        }
        let lists = lists();
        assert_eq!(seen.len(), lists.words.len() + lists.exempt.len());
    }

    #[test]
    fn every_exempt_pattern_covers_a_listed_word_and_is_not_one() {
        let lists = lists();
        for pattern in &lists.exempt {
            assert!(
                !lists.words.contains(pattern),
                "{pattern} 既在表里又是豁免写法"
            );
            assert!(
                lists.words.iter().any(|word| pattern.contains(word)),
                "{pattern} 不含表里的词，豁免它没有意义"
            );
        }
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
    fn an_offensive_word_is_an_edge_only_when_it_spans_the_whole_input() {
        assert!(admits_edge("巨乳", 0, 2, 2));
        assert!(!admits_edge("巨乳", 3, 5, 5));
        assert!(!admits_edge("巨乳", 0, 2, 3));
        assert!(!admits_edge("吃屎了", 2, 5, 5));
        assert!(admits_edge("水乳交融", 2, 6, 6));
        assert!(admits_edge("走路", 2, 4, 5));
    }
}
