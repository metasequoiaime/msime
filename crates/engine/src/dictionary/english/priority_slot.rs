//! 中文状态下英文词能不能坐优先位置（九键的第 2 位，26 键短输入的首选之后）。
//!
//! 2026-11-04 在 dict-v2.0.41 上实测，常用前 5000 个九键码里 697 个（约 14%）第 2 位是英文，一半以上是 SCOWL 补进来的冷僻词：所以（78694）是 stowing，他们 是 tandoori，其实 是 Qiqihar；词库仓库贡献的品牌拼音名（Xiaohongshu、Hunyuan）也坐在 想、或许 的第 2 位。原来的规则是「有权重就坐第 2 位」，而发布库里 74158 行英文只有 810 行权重是 0。维护者定的门槛是：SCOWL 独有的词、Google 排名 2 万以后的词、没有计数的贡献词都不坐第 2 位。
//!
//! `msime-english.db` 的 `english_words` 只有 `word`、`display`、`weight` 三列，不记来源和排名。但构建器（`crates/dict-builder/src/english.rs` 的 `build_english_words`）写的权重就是这个词在 `google-word-counts.txt` 里的计数（大小写不同的几种写法依次少 1），没有计数的词库行是 0，没有计数的贡献词封顶在最小计数之下（`custom_english_weight`，最大 12710），所以「排名 2 万以内」就是「权重不低于第 2 万名的计数」，后两类词的权重都低于这个数，同一个门槛一起挡住。SCOWL 独有的词里排名在 2 万以内的只有 19 个（its、mrs、lets 和月份、星期的缩写），门槛对它们不起作用也无妨。要按来源精确区分，构建器得在库里另记一列，见 `.agents/notes/implemented/bug-fix/2026-11-04-vulgar-first-page-and-english-second-slot.md`。

use std::path::Path;

use crate::lattice::offensive::{is_offensive_english, OffensiveWords};
use crate::offensive_placement::{grid_demotion, keep_off_first_page};
use crate::types::WordItem;
use crate::user_dictionary::journal::has_english_upsert;

/// `google-word-counts.txt`（msime-dictionary `sources/english/`，SHA-256 `c71de38c5a4e97e08192fb1c74fc3395d68782bbc2d6dea190c6c90542b06c1f`）第 20000 行的计数。换了这份文件就要按同样的口径重取。
pub(crate) const COMMON_ENGLISH_MIN_WEIGHT: i64 = 1_602_219;

/// 英文行能不能坐优先位置：常见（权重不低于 [`COMMON_ENGLISH_MIN_WEIGHT`]），而且不是粗俗词或蔑称。
pub(crate) fn takes_priority_slot(item: &WordItem) -> bool {
    item.weight >= COMMON_ENGLISH_MIN_WEIGHT && !is_offensive_english(&item.word)
}

/// 同 [`takes_priority_slot`]，另外放行用户自己的词：`own` 答是（日志里有这个词，用户加过或选过它）时不看常见度。个人词库加的英文词默认权重只有 10 万（`PersonalDictionaryEntry::DEFAULT_WEIGHT`），选词学习也只把它抬到同一列表的最大权重之上，都可能低于门槛；门槛要挡的是词库里的冷僻词，不是用户自己的词。粗俗词照样不坐。`own` 要查日志，只在门槛挡住、又不是粗俗词时才调用。
pub(crate) fn takes_priority_slot_or_own(
    item: &WordItem,
    own: impl FnOnce(&WordItem) -> bool,
) -> bool {
    !is_offensive_english(&item.word) && (item.weight >= COMMON_ENGLISH_MIN_WEIGHT || own(item))
}

/// 九键把查到的英文行 `english`（已按原来的先后排好）并进拼音候选 `candidates`。常见、不粗俗的英文（[`takes_priority_slot`]）坐第 2 位和首页剩余空间（`first_page_count` 行，不超出 `first_page_rows`），按原来的先后排在前面；其余的接在拼音末尾。用户自己加过或选过的词把拼写打全时也坐（[`takes_priority_slot_or_own`]）：只有字母数等于已打数字数 `typed` 的行要查日志 `journal`，一键查不了几次。没有拼音读法时（`candidates` 是空的）列表只有英文，照原来的先后给，只把英文粗俗词挪出首屏。
pub(crate) fn seat_nine_key_english(
    candidates: &mut Vec<WordItem>,
    mut english: Vec<WordItem>,
    typed: usize,
    journal: &Path,
    first_page_count: usize,
    first_page_rows: usize,
) {
    let seated = if candidates.is_empty() {
        // 全是英文行，只看英文粗俗词表，中文那张用不上。
        let vulgar = OffensiveWords::default();
        keep_off_first_page(&mut english, first_page_rows, |item| {
            grid_demotion(item, &vulgar)
        });
        english.len()
    } else {
        let (mut seated, rest): (Vec<WordItem>, Vec<WordItem>) =
            english.into_iter().partition(|word| {
                takes_priority_slot_or_own(word, |word| {
                    word.pinyin.len() == typed && has_english_upsert(journal, &word.pinyin)
                })
            });
        let count = seated.len();
        seated.extend(rest);
        english = seated;
        count
    };
    if seated > 0 {
        let first = english.remove(0);
        let slot = candidates.len().min(1);
        candidates.insert(slot, first);
        let extra = first_page_count
            .saturating_sub(1)
            .min(first_page_rows.saturating_sub(slot + 1))
            .min(seated - 1);
        if extra > 0 {
            candidates.splice(slot + 1..slot + 1, english.drain(..extra));
        }
    }
    candidates.extend(english);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::CandidateSource;

    fn english(display: &str, weight: i64) -> WordItem {
        WordItem::new(
            display.to_ascii_lowercase(),
            display,
            weight,
            CandidateSource::EnglishDictionary,
            "",
        )
    }

    /// 权重取自 dict-v2.0.41 的 `msime-english.db`。
    #[test]
    fn rare_uncounted_and_vulgar_words_do_not_take_the_priority_slot() {
        for (word, weight) in [
            ("stowing", 49_889),
            ("tandoori", 246_988),
            ("Qiqihar", 15_366),
            ("Xiaohongshu", 1),
            ("Hunyuan", 1),
            ("Negro", 2_489_080),
            ("cumshot", 16_597_227),
            ("gangbang", 10_767_141),
            ("shitty", 730_189),
        ] {
            assert!(!takes_priority_slot(&english(word, weight)), "{word}");
        }
        for (word, weight) in [
            ("the", 23_135_851_162),
            ("good", 400_000_000),
            ("Lipitor", 4_243_818),
        ] {
            assert!(takes_priority_slot(&english(word, weight)), "{word}");
        }
    }

    #[test]
    fn the_users_own_words_pass_the_commonness_gate_but_not_the_vulgar_list() {
        let own = english("Metasequoia", 100_000);
        assert!(!takes_priority_slot(&own));
        assert!(takes_priority_slot_or_own(&own, |_| true));
        assert!(!takes_priority_slot_or_own(&own, |_| false));
        assert!(!takes_priority_slot_or_own(
            &english("shitty", 730_189),
            |_| true
        ));
        // 常见的词不查日志。
        assert!(takes_priority_slot_or_own(
            &english("good", 400_000_000),
            |_| unreachable!()
        ));
        assert!(!takes_priority_slot_or_own(
            &english("Negro", 2_489_080),
            |_| unreachable!()
        ));
    }
}
