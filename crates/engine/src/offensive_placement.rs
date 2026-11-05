//! 粗俗词不当首选、不进歧义输入的首屏。
//!
//! 维护者 2026-11-04 定的粗俗内容政策：粗俗词本身（傻逼、鸡巴）留在词库里，但不得当首选，也不得出现在前缀、部分候选、简拼和九键、14 键歧义码的首屏；流行的轻度粗口（牛逼、卧槽）照样当 26 键的首选，只是不进九键、14 键歧义码的首屏。词表见 `lattice::offensive`。`lattice::offensive` 原来只管拼整句、纠错和联想，词库行本身不受影响，所以 九键 2435422 的首选是只有占位权重 100 的 别鸡巴，九键 内容（63476）第 2 位是英文 Negro。
//!
//! 这里只挪位置，不删行：往后翻页照样选得到，26 键上把一个粗俗词的完整拼写打全时它仍在首屏（只是不在第一位）。挪到首屏之后的行保持原来的先后，首屏空出来的位置由后面的行依次补上。用户固定了位置的行不受影响：调用方在应用固定位置之前调用这里，固定位置照旧生效。

use crate::lattice::offensive::{
    is_mild_slang, is_mild_slang_sentence, is_offensive_english, OffensiveWords,
};
use crate::shuangpin::utils::encode_pinyin;
use crate::shuangpin::ShuangpinProfile;
use crate::types::{CandidateSource, SchemeType, WordItem};

/// 一行候选要挪到哪里。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Demotion {
    /// 不动。
    Keep,
    /// 不当首选，首屏其余位置可以。
    OffLead,
    /// 挪出首屏。
    OffPage,
}

/// 按 `demotion` 把行挪出首选或首屏，`page` 是每页候选数（0 当作 1）。首屏里没有要挪的行时不分配内存、不动列表。全是要挪的行时它们按原来的先后接在后面，所以只剩粗俗词时它照样是第一行。
pub(crate) fn keep_off_first_page(
    list: &mut Vec<WordItem>,
    page: usize,
    mut demotion: impl FnMut(&WordItem) -> Demotion,
) {
    let page = page.max(1);
    let needs_moving =
        list.iter()
            .take(page)
            .enumerate()
            .any(|(index, item)| match demotion(item) {
                Demotion::Keep => false,
                Demotion::OffLead => index == 0,
                Demotion::OffPage => true,
            });
    if !needs_moving {
        return;
    }
    let rows = std::mem::take(list);
    list.reserve(rows.len());
    let mut off_lead = Vec::new();
    let mut off_page = Vec::new();
    for item in rows {
        // 首屏排满之后的行不再判断，原样接上。
        let kind = if list.len() < page {
            demotion(&item)
        } else {
            Demotion::Keep
        };
        match kind {
            Demotion::OffPage => {
                off_page.push(item);
                continue;
            }
            Demotion::OffLead if list.is_empty() => {
                off_lead.push(item);
                continue;
            }
            _ => list.push(item),
        }
        // 首选定下之后，让出首选的行紧跟着放回去。
        list.append(&mut off_lead);
        if list.len() >= page {
            list.append(&mut off_page);
        }
    }
    list.append(&mut off_lead);
    list.append(&mut off_page);
}

/// 固定位置和云、AI 行在 [`keep_off_first_page`] 之后才放回各自的位置，放的时候会把首屏的行挪走、后面的行依次前移，挪出首屏的行可能因此又落回首屏的最后几位。这里在那之后补一遍：首屏里还有要挪的行时，和首屏之后第一个可以上首屏的行（不挪、没有固定位置、不是云或 AI 行）对调，其余每一行都留在原位，用户固定的位置和云、AI 的席位不受影响。首选替补还排除表情、颜文字和混输日文；中文首选不拿混输英文补位，原首选是英文时仍可用普通英文替补。用户固定了位置的行本身也不挪。首屏里没有要挪的行时只做判断，不分配内存。
pub(crate) fn evict_after_positions(
    list: &mut [WordItem],
    page: usize,
    mut demotion: impl FnMut(&WordItem) -> Demotion,
) {
    let page = page.max(1);
    for index in 0..page.min(list.len()) {
        let item = &list[index];
        if item.fixed_position > 0 {
            continue;
        }
        let from = match demotion(item) {
            Demotion::Keep => continue,
            Demotion::OffLead if index > 0 => continue,
            Demotion::OffLead => 1,
            Demotion::OffPage => page,
        };
        let Some(tail) = list.get(from..) else {
            continue;
        };
        let replacement = tail.iter().position(|other| {
            other.fixed_position == 0
                && !other.source.takes_online_seat()
                // 中文首选的替补仍需遵守混输约束；纯英文首选可以用另一行英文补位。
                && (index > 0
                    || (!matches!(
                        other.source,
                        CandidateSource::Emoji | CandidateSource::Kaomoji | CandidateSource::MixedJapanese
                    ) && (other.source != CandidateSource::EnglishDictionary
                        || item.source == CandidateSource::EnglishDictionary)))
                && demotion(other) == Demotion::Keep
        });
        if let Some(offset) = replacement {
            list.swap(index, from + offset);
        }
    }
}

/// 词库给的中文行和词网格整句：用户自己的词（`UserDatabase`）、快捷短语、联系人、云和 AI 候选这些都不是共享词库给的，不管。
fn is_dictionary_text(source: CandidateSource) -> bool {
    matches!(
        source,
        CandidateSource::Database
            | CandidateSource::Generated
            | CandidateSource::Fallback
            | CandidateSource::NeuralKeyboard
            | CandidateSource::Collocation
    )
}

/// 这一行含 `vulgar` 表里的粗俗词（活动词库的 `sentence_excluded` 表，没有时是内嵌表，与拼整句同一张）。词网格拼出的整句按它的词查（`sentence_words`）：跨词边界的命中只在恰好由整词拼成时算，微信群 + 交流 不因 群交 被挪走；词库行按整个词按子串查，规则同词网格的边。
fn has_vulgar(item: &WordItem, vulgar: &OffensiveWords) -> bool {
    if item.sentence_words.is_empty() {
        vulgar.is_offensive(&item.word)
    } else {
        vulgar.is_offensive_sentence(&item.sentence_words, &item.word)
    }
}

/// 这一行含轻度粗口，按句和按词的区分同 [`has_vulgar`]。
fn has_mild_slang(item: &WordItem) -> bool {
    if item.sentence_words.is_empty() {
        is_mild_slang(&item.word)
    } else {
        is_mild_slang_sentence(&item.sentence_words, &item.word)
    }
}

/// 九键、14 键这类一个键几个字母的键盘：同一串码有许多种读法，粗俗词即使整段命中也不一定是用户要的词，所以粗俗词、轻度粗口和英文粗俗词一律挪出首屏。中文状态下的英文行在这一步之后才插进来，坐不坐第二位由 `english::takes_priority_slot` 管，这里的英文判断只对专用英文九键起作用。
pub(crate) fn grid_demotion(item: &WordItem, vulgar: &OffensiveWords) -> Demotion {
    let flagged = match item.source {
        CandidateSource::EnglishDictionary => is_offensive_english(&item.word),
        source if is_dictionary_text(source) => has_vulgar(item, vulgar) || has_mild_slang(item),
        _ => false,
    };
    if flagged {
        Demotion::OffPage
    } else {
        Demotion::Keep
    }
}

/// 26 键全拼、双拼：粗俗词（不含轻度粗口）是整段输入的完整拼写时（`exact`）只让出首选，其余情况（前缀补全、只盖住一部分输入的词、简拼、模糊音和纠错查出来的行）挪出首屏。混输的英文行不经过这里：英文粗俗词在 `CandidateQueries::mixed` 里就不坐优先位置，接在列表末尾。
pub(crate) fn typed_demotion(
    item: &WordItem,
    vulgar: &OffensiveWords,
    exact: impl Fn(&WordItem) -> bool,
) -> Demotion {
    if !is_dictionary_text(item.source) || !has_vulgar(item, vulgar) {
        Demotion::Keep
    } else if exact(item) {
        Demotion::OffLead
    } else {
        Demotion::OffPage
    }
}

/// 26 键上这一行是不是整段输入 `input` 的完整拼写：全拼是词库读音去掉分隔符等于输入，双拼是读音按方案编出的码等于输入。模糊音和纠错查出来的行不算，用户打的不是它的读音。其余方案一概不算。
pub(crate) fn spells_whole_input(
    item: &WordItem,
    input: &str,
    scheme: SchemeType,
    profile: &ShuangpinProfile,
) -> bool {
    if item.fuzzy || !item.corrected_from.is_empty() || item.canonical_pinyin.is_empty() {
        return false;
    }
    match scheme {
        SchemeType::Quanpin => same_spelling(&item.canonical_pinyin, input),
        SchemeType::Shuangpin => encode_pinyin(&item.canonical_pinyin, profile)
            .is_some_and(|code| same_spelling(&code, input)),
        _ => false,
    }
}

/// 两段拼写去掉音节分隔符 `'` 之后相同（ASCII 大小写不计）。
pub(crate) fn same_spelling(left: &str, right: &str) -> bool {
    left.bytes()
        .filter(|&byte| byte != b'\'')
        .map(|byte| byte.to_ascii_lowercase())
        .eq(right
            .bytes()
            .filter(|&byte| byte != b'\'')
            .map(|byte| byte.to_ascii_lowercase()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lattice::offensive::OffensiveEntry;

    fn grid(item: &WordItem) -> Demotion {
        grid_demotion(item, &OffensiveWords::default())
    }

    fn typed(item: &WordItem, exact: impl Fn(&WordItem) -> bool) -> Demotion {
        typed_demotion(item, &OffensiveWords::default(), exact)
    }

    fn row(word: &str, source: CandidateSource) -> WordItem {
        WordItem::new("", word, 0, source, "")
    }

    fn words(list: &[WordItem]) -> Vec<&str> {
        list.iter().map(|item| item.word.as_str()).collect()
    }

    fn by_word<'a>(
        page_off: &'a [&'a str],
        lead_off: &'a [&'a str],
    ) -> impl FnMut(&WordItem) -> Demotion + 'a {
        move |item| {
            if page_off.contains(&item.word.as_str()) {
                Demotion::OffPage
            } else if lead_off.contains(&item.word.as_str()) {
                Demotion::OffLead
            } else {
                Demotion::Keep
            }
        }
    }

    fn list(words: &[&str]) -> Vec<WordItem> {
        words
            .iter()
            .map(|word| row(word, CandidateSource::Database))
            .collect()
    }

    #[test]
    fn rows_off_the_page_follow_the_first_page_in_their_order() {
        let mut rows = list(&["x1", "a", "x2", "b", "c", "d", "e"]);
        keep_off_first_page(&mut rows, 3, by_word(&["x1", "x2"], &[]));
        assert_eq!(words(&rows), ["a", "b", "c", "x1", "x2", "d", "e"]);
    }

    #[test]
    fn a_row_off_the_lead_takes_the_second_seat() {
        let mut rows = list(&["x", "a", "b", "c"]);
        keep_off_first_page(&mut rows, 3, by_word(&[], &["x"]));
        assert_eq!(words(&rows), ["a", "x", "b", "c"]);
        // 不在首位的不动。
        let mut rows = list(&["a", "x", "b"]);
        keep_off_first_page(&mut rows, 3, by_word(&[], &["x"]));
        assert_eq!(words(&rows), ["a", "x", "b"]);
    }

    #[test]
    fn rows_beyond_the_first_page_are_left_alone() {
        let mut rows = list(&["a", "b", "x", "c"]);
        keep_off_first_page(&mut rows, 2, by_word(&["x"], &[]));
        assert_eq!(words(&rows), ["a", "b", "x", "c"]);
    }

    #[test]
    fn a_list_of_nothing_else_keeps_its_rows() {
        let mut rows = list(&["x1", "x2"]);
        keep_off_first_page(&mut rows, 6, by_word(&["x1"], &["x2"]));
        assert_eq!(words(&rows), ["x2", "x1"]);
        let mut rows = list(&["x"]);
        keep_off_first_page(&mut rows, 0, by_word(&[], &["x"]));
        assert_eq!(words(&rows), ["x"]);
    }

    #[test]
    fn a_short_list_puts_the_moved_rows_after_the_rest() {
        let mut rows = list(&["x", "a", "b"]);
        keep_off_first_page(&mut rows, 6, by_word(&["x"], &[]));
        assert_eq!(words(&rows), ["a", "b", "x"]);
    }

    #[test]
    fn grids_move_vulgar_words_mild_slang_and_english_slurs() {
        for (word, source) in [
            ("别鸡巴", CandidateSource::Database),
            ("傻逼", CandidateSource::Generated),
            ("牛逼", CandidateSource::Database),
            ("Negro", CandidateSource::EnglishDictionary),
            ("cumshot", CandidateSource::EnglishDictionary),
        ] {
            assert_eq!(grid(&row(word, source)), Demotion::OffPage, "{word}");
        }
        for (word, source) in [
            ("内容", CandidateSource::Database),
            ("鸡巴", CandidateSource::UserDatabase),
            ("傻逼", CandidateSource::QuickPhrase),
            ("negroni", CandidateSource::EnglishDictionary),
            ("cocktail", CandidateSource::EnglishDictionary),
        ] {
            assert_eq!(grid(&row(word, source)), Demotion::Keep, "{word}");
        }
    }

    fn sentence(words: &[&str], source: CandidateSource) -> WordItem {
        let mut item = row(&words.concat(), source);
        item.sentence_words = words.iter().map(|word| (*word).to_owned()).collect();
        item
    }

    #[test]
    fn sentences_are_judged_by_their_words() {
        for words in [
            &["微信群", "交流"][..],
            &["我", "操作"],
            &["不必", "为", "我", "操心"],
        ] {
            let item = sentence(words, CandidateSource::Generated);
            assert_eq!(grid(&item), Demotion::Keep, "{}", item.word);
            assert_eq!(typed(&item, |_| true), Demotion::Keep, "{}", item.word);
        }
        for words in [&["别", "鸡", "巴"][..], &["是", "傻逼"]] {
            let item = sentence(words, CandidateSource::Generated);
            assert_eq!(grid(&item), Demotion::OffPage, "{}", item.word);
            assert_eq!(typed(&item, |_| true), Demotion::OffLead, "{}", item.word);
        }
        assert_eq!(
            grid(&sentence(&["真", "牛逼"], CandidateSource::Generated)),
            Demotion::OffPage
        );
    }

    #[test]
    fn typing_the_full_spelling_only_gives_up_the_lead() {
        let exact = |item: &WordItem| item.word.as_str() == "傻逼";
        assert_eq!(
            typed(&row("傻逼", CandidateSource::Database), exact),
            Demotion::OffLead
        );
        assert_eq!(
            typed(&row("是傻逼", CandidateSource::Database), exact),
            Demotion::OffPage
        );
        // 轻度粗口在 26 键上不动。
        assert_eq!(
            typed(&row("牛逼", CandidateSource::Database), |_| false),
            Demotion::Keep
        );
    }

    #[test]
    fn spellings_compare_without_separators_or_case() {
        assert!(same_spelling("sha'bi", "shabi"));
        assert!(same_spelling("Negro", "negro"));
        assert!(!same_spelling("sha'bi", "shab"));
    }

    /// 词库带着自己的表时排位读它：表里没有的 傻逼 照常当首选，表里的 走路 不进歧义码的首屏。
    #[test]
    fn placement_reads_the_dictionary_table() {
        let table = OffensiveWords::from_entries([OffensiveEntry {
            text: "走路".to_owned(),
            exempt: false,
        }])
        .unwrap();
        let shabi = row("傻逼", CandidateSource::Database);
        assert_eq!(grid_demotion(&shabi, &table), Demotion::Keep);
        assert_eq!(typed_demotion(&shabi, &table, |_| true), Demotion::Keep);
        assert_eq!(
            grid_demotion(&row("走路", CandidateSource::Database), &table),
            Demotion::OffPage
        );
        assert_eq!(
            grid_demotion(
                &sentence(&["去", "走", "路"], CandidateSource::Generated),
                &table
            ),
            Demotion::OffPage
        );
    }

    /// 固定位置把首屏的一行挪走后，挪出首屏的粗俗词落回了首屏最后一位：和首屏之后第一个可以上首屏的行对调，其余行不动。
    #[test]
    fn rows_that_slid_back_onto_the_first_page_are_swapped_out() {
        let mut list: Vec<WordItem> = ["别急吧", "扯家常", "别鸡巴", "车家村", "别家"]
            .iter()
            .map(|word| row(word, CandidateSource::Database))
            .collect();
        evict_after_positions(&mut list, 3, grid);
        assert_eq!(
            words(&list),
            ["别急吧", "扯家常", "车家村", "别鸡巴", "别家"]
        );

        // 用户固定的行不挪，也不拿来补位；云和 AI 行也不拿来补位。
        let mut list: Vec<WordItem> = ["别急吧", "别鸡巴", "扯家常", "车家村", "别家"]
            .iter()
            .map(|word| row(word, CandidateSource::Database))
            .collect();
        list[2].fixed_position = 3;
        list[3].source = CandidateSource::AiSuggestion;
        evict_after_positions(&mut list, 3, grid);
        assert_eq!(
            words(&list),
            ["别急吧", "别家", "扯家常", "车家村", "别鸡巴"]
        );

        let mut pinned = vec![
            row("别鸡巴", CandidateSource::Database),
            row("别急吧", CandidateSource::Database),
        ];
        pinned[0].fixed_position = 1;
        evict_after_positions(&mut pinned, 1, grid);
        assert_eq!(words(&pinned), ["别鸡巴", "别急吧"]);

        // 26 键打全拼写的粗俗词只让出首选。
        let mut lead = vec![
            row("傻逼", CandidateSource::Database),
            row("沙壁", CandidateSource::Database),
            row("啥比", CandidateSource::Database),
        ];
        evict_after_positions(&mut lead, 3, |item| typed(item, |_| true));
        assert_eq!(words(&lead), ["沙壁", "傻逼", "啥比"]);
    }

    /// 二次降位仍按混输的首选约束找替补，不能把表情、英文或日文抬到首位。
    #[test]
    fn evicting_the_lead_keeps_mixed_rows_in_their_seats() {
        for source in [
            CandidateSource::Emoji,
            CandidateSource::Kaomoji,
            CandidateSource::EnglishDictionary,
            CandidateSource::MixedJapanese,
        ] {
            let mut list = vec![
                row("傻逼", CandidateSource::Database),
                row("混入行", source),
                row("沙币", CandidateSource::Database),
            ];
            evict_after_positions(&mut list, 3, |item| typed(item, |_| true));
            assert_eq!(words(&list), ["沙币", "混入行", "傻逼"], "{source:?}");
        }
    }

    /// 固定行和在线席位保持原位，首屏尾部的表达式行也不能成为新的首选。
    #[test]
    fn evicting_the_first_page_keeps_mixed_rows_off_the_lead() {
        for source in [
            CandidateSource::Emoji,
            CandidateSource::Kaomoji,
            CandidateSource::EnglishDictionary,
            CandidateSource::MixedJapanese,
        ] {
            let mut list = vec![
                row("别鸡巴", CandidateSource::Database),
                row("固定二", CandidateSource::Database),
                row("固定三", CandidateSource::Database),
                row("固定四", CandidateSource::Database),
                row("混入行", source),
                row("别急吧", CandidateSource::Database),
            ];
            for (index, item) in list.iter_mut().enumerate().take(4).skip(1) {
                item.fixed_position = (index + 1) as i32;
            }
            evict_after_positions(&mut list, 3, grid);
            assert_eq!(
                words(&list),
                ["别急吧", "固定二", "固定三", "固定四", "混入行", "别鸡巴"],
                "{source:?}"
            );
            assert_eq!(list[1].fixed_position, 2);
            assert_eq!(list[2].fixed_position, 3);
            assert_eq!(list[3].fixed_position, 4);
        }
    }

    /// 只有英文行的列表照常拿普通英文替换粗俗英文首选。
    #[test]
    fn evicting_an_english_lead_can_use_another_english_word() {
        let mut list = vec![
            row("fuck", CandidateSource::EnglishDictionary),
            row("word", CandidateSource::EnglishDictionary),
        ];
        evict_after_positions(&mut list, 1, grid);
        assert_eq!(words(&list), ["word", "fuck"]);
    }

    /// 按九键真实顺序降位、混入、固定和兜底后，二次降位不能重新把表情抬到首位。
    #[test]
    fn evicting_after_fixed_positions_does_not_restore_expressive_lead() {
        use crate::ime::queries::keep_expressive_rows_off_the_lead;
        use crate::user_dictionary::journal::test_support::Dir;
        use crate::user_dictionary::positions::{apply_fixed_positions, set_fixed_position};

        for source in [CandidateSource::Emoji, CandidateSource::Kaomoji] {
            let directory = Dir::new();
            let journal = directory.journal();
            let mut list: Vec<_> = ["别鸡巴", "固定四", "固定二", "固定三", "别急吧"]
                .iter()
                .map(|word| row(word, CandidateSource::Database))
                .collect();
            keep_off_first_page(&mut list, 3, grid);
            list.insert(1, row("混入行", source));
            for (word, position) in [("固定二", 2), ("固定三", 3), ("固定四", 4)] {
                set_fixed_position(&journal, "synthetic", "synthetic", word, position).unwrap();
            }
            apply_fixed_positions(&journal, "synthetic", &mut list, false, None, false);
            keep_expressive_rows_off_the_lead(&mut list);
            assert_eq!(
                words(&list),
                ["别鸡巴", "固定二", "固定三", "固定四", "混入行", "别急吧"]
            );
            evict_after_positions(&mut list, 3, grid);
            assert_eq!(
                words(&list),
                ["别急吧", "固定二", "固定三", "固定四", "混入行", "别鸡巴"],
                "{source:?}"
            );
        }
    }
}
