//! 模糊音命中的词库行和字面命中的词库行怎么比（#11321）。全拼的合并和全拼的选词调频共用这一把尺：模糊行的权重先打折再和字面行比，调频也在折后的尺度上算、写回时再还原。26 键双拼（按原权重合并）和九键（模糊行整组在后）都不走这里，见 `.agents/notes/implemented/bug-fix/2026-11-12-fuzzy-rows-rank-with-exact-rows.md`。

use crate::types::WordItem;

/// 模糊行和字面行比权重时，模糊行的权重除以这个数。
///
/// 取值来自锁定词库（dict-v2.1.2）上的实测，方法和全部数字在上面那篇笔记里：
/// - 不打折（1）时，权重只略高的模糊词会顶掉用户照字面打的常用词：开全部规则打 `wangshang`，「晚上」排到「网上」前面；打 `hen`，「分」排到「很」前面。除以 2 这两处都回到字面在前。
/// - 再往大取，按模糊读法打的常用词开始掉出首选，丢的比字面拼写收回的多：`xinqi` 的「星期」除以 3 就落到「新奇」后面，`nihai` 的「厉害」除以 4 落到第 8 位。词频第 4000 名起的 300 个词，把后鼻音打成前鼻音时首选命中率是 67.0%（÷2）、65.3%（÷3）、62.3%（÷4）、60.0%（÷8），照字面打同一档词的命中率只从 93.3% 回到 93.7%、93.7%、94.3%。
///
/// 所以取能挡住「只稍高一点」的最小整数。
pub(crate) const FUZZY_WEIGHT_DIVISOR: i64 = 2;

/// 这一行拿去和同一覆盖档里其他词库行比的权重：字面行是词库权重本身，模糊行除以 [`FUZZY_WEIGHT_DIVISOR`]。零和负权重不除，免得折后反而变大。只对词库行有意义，整句行的 `weight` 是另一种量纲，不拿来比。
pub(crate) fn ranking_weight(item: &WordItem) -> i64 {
    if item.fuzzy && item.weight > 0 {
        item.weight / FUZZY_WEIGHT_DIVISOR
    } else {
        item.weight
    }
}

/// [`ranking_weight`] 的逆：调频在折后的尺度上算出 `ranking`，写回词库前还原成这一行存的权重，下次查出来折算后正好是 `ranking`。
pub(crate) fn stored_weight(fuzzy: bool, ranking: i64) -> i64 {
    if fuzzy && ranking > 0 {
        ranking.saturating_mul(FUZZY_WEIGHT_DIVISOR)
    } else {
        ranking
    }
}

/// 匹配码里实际按下的键数，`'` 只是显示用的分隔。
fn covered_keys(pinyin: &str) -> usize {
    pinyin.bytes().filter(|&byte| byte != b'\'').count()
}

/// 全拼的模糊合并把模糊行接在同一覆盖档的末尾，这里把它们按折后权重并进字面行里。`rows` 已按覆盖的键数排成一档一档；每一档里：
///
/// - 字面行、整句行之间的先后一行不动，只挪模糊行。
/// - 一条模糊行排到第一条比它（折后）轻的字面词库行前面，紧跟在那一行之前最近的一条字面词库行后面：整句块跟在字面整词后面，模糊整词也是整词，不落到整句块后面。权重相同时字面行在前。
/// - 没有比它轻的字面词库行时留在档尾原位，所以排不过任何字面行的模糊行和以前一样在最后。
/// - 整句行的 `weight` 不参与比较，它们只是原地不动。
///
/// 模糊行之间按折后权重从高到低。没有模糊行要挪时不申请内存。
pub(crate) fn rank_fuzzy_rows_with_exact_rows(rows: &mut [WordItem]) {
    let mut start = 0;
    while start < rows.len() {
        let keys = covered_keys(&rows[start].pinyin);
        let end = start
            + rows[start..]
                .iter()
                .take_while(|row| covered_keys(&row.pinyin) == keys)
                .count();
        rank_bucket(&mut rows[start..end]);
        start = end;
    }
}

fn is_exact_dictionary_row(row: &WordItem) -> bool {
    !row.fuzzy && row.source.is_dictionary()
}

fn rank_bucket(bucket: &mut [WordItem]) {
    let Some(lightest) = bucket
        .iter()
        .filter(|row| is_exact_dictionary_row(row))
        .map(|row| row.weight)
        .min()
    else {
        return;
    };
    if !bucket
        .iter()
        .any(|row| row.fuzzy && ranking_weight(row) > lightest)
    {
        return;
    }
    let (fuzzy, kept): (Vec<WordItem>, Vec<WordItem>) = bucket
        .iter_mut()
        .map(std::mem::take)
        .partition(|row| row.fuzzy);
    // 每条字面词库行在 `kept` 里的下标，和到它为止（含它）最轻的字面词库权重：后一列单调不增，可以二分。
    let mut seats = Vec::new();
    let mut floor = i64::MAX;
    for (index, row) in kept.iter().enumerate() {
        if is_exact_dictionary_row(row) {
            floor = floor.min(row.weight);
            seats.push((index, floor));
        }
    }
    // 模糊行前面该有几条 `kept` 行。
    let mut placed: Vec<(usize, WordItem)> = fuzzy
        .into_iter()
        .map(|row| {
            let weight = ranking_weight(&row);
            let lighter = seats.partition_point(|&(_, floor)| floor >= weight);
            let before = if lighter == seats.len() {
                kept.len()
            } else if lighter == 0 {
                seats[0].0
            } else {
                seats[lighter - 1].0 + 1
            };
            (before, row)
        })
        .collect();
    placed.sort_by_key(|(before, row)| (*before, std::cmp::Reverse(ranking_weight(row))));
    let mut placed = placed.into_iter().peekable();
    let mut seat = 0;
    for (index, row) in kept.into_iter().enumerate() {
        while let Some((_, fuzzy)) = placed.next_if(|(before, _)| *before <= index) {
            bucket[seat] = fuzzy;
            seat += 1;
        }
        bucket[seat] = row;
        seat += 1;
    }
    for (_, fuzzy) in placed {
        bucket[seat] = fuzzy;
        seat += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::CandidateSource;

    fn row(pinyin: &str, word: &str, weight: i64, fuzzy: bool) -> WordItem {
        let mut item = WordItem::new(pinyin, word, weight, CandidateSource::Database, pinyin);
        item.fuzzy = fuzzy;
        item
    }

    fn sentence(pinyin: &str, word: &str, weight: i64) -> WordItem {
        let mut item = WordItem::new(pinyin, word, weight, CandidateSource::Generated, pinyin);
        item.sentence_association = true;
        item
    }

    fn words(rows: &[WordItem]) -> Vec<&str> {
        rows.iter().map(|row| row.word.as_str()).collect()
    }

    #[test]
    fn the_discount_round_trips_and_leaves_exact_and_non_positive_weights_alone() {
        let exact = row("zi", "字", 900, false);
        let fuzzy = row("zi", "只", 900, true);
        assert_eq!(ranking_weight(&exact), 900);
        assert_eq!(ranking_weight(&fuzzy), 900 / FUZZY_WEIGHT_DIVISOR);
        assert_eq!(ranking_weight(&row("zi", "零", 0, true)), 0);
        assert_eq!(ranking_weight(&row("zi", "负", -8, true)), -8);
        for ranking in [1, 7, 450, 100_000_000] {
            let stored = stored_weight(true, ranking);
            assert_eq!(ranking_weight(&row("zi", "只", stored, true)), ranking);
            assert_eq!(stored_weight(false, ranking), ranking);
        }
        assert_eq!(stored_weight(true, -8), -8);
        assert_eq!(stored_weight(true, i64::MAX), i64::MAX);
    }

    #[test]
    fn a_fuzzy_row_passes_only_exact_rows_lighter_than_its_discounted_weight() {
        let divisor = FUZZY_WEIGHT_DIVISOR;
        let mut rows = vec![
            row("zi'qian", "自谦", 5_000, false),
            row("zi'qian", "自遣", 3_000, false),
            row("zi'qian", "自谴", 100, false),
            row("zi'qian", "之前", 4_000_000, true),
            // 折后正好等于「自遣」：字面在前。
            row("zi'qian", "值钱", 3_000 * divisor, true),
            // 原权重比「自谦」高，折后比它低。
            row("zi'qian", "纸钱", 5_000 * divisor - 2, true),
            row("zi'qian", "掷签", 100, true),
            row("zi", "自", 9_000, false),
            row("zi", "只", 9_000 * divisor + divisor, true),
            row("zi", "之", 10, true),
        ];
        // 「纸钱」到货时排在「值钱」后面，合并后按折后权重排回它前面。
        rank_fuzzy_rows_with_exact_rows(&mut rows);
        assert_eq!(
            words(&rows),
            [
                "之前", "自谦", "纸钱", "自遣", "值钱", "自谴", "掷签", "只", "自", "之"
            ]
        );
    }

    #[test]
    fn sentences_keep_their_seats_and_fuzzy_words_stay_with_the_dictionary_words() {
        let mut rows = vec![
            row("ni'hai", "你还", 1_000, false),
            sentence("ni'hai", "你海", 900_000),
            sentence("ni'hai", "你孩", -50),
            row("ni'hai", "你还是", 400, false),
            sentence("ni'h'ai", "你喝爱", 700_000),
            row("ni'hai", "厉害", 800 * FUZZY_WEIGHT_DIVISOR, true),
            row("ni'hai", "利害", 400, true),
        ];
        rank_fuzzy_rows_with_exact_rows(&mut rows);
        // 「厉害」比「你还是」重、比「你还」轻：紧跟在「你还」后面，不落到整句块后面，也不和整句比权重。「利害」排不过任何字面词库行，留在档尾。
        assert_eq!(
            words(&rows),
            ["你还", "厉害", "你海", "你孩", "你还是", "你喝爱", "利害"]
        );

        // 首位是整句、后面才有字面词库行：比它们都重的模糊行排到第一条字面词库行前面，整句仍在首位。
        let mut led_by_sentence = vec![
            sentence("zong'guo", "总国", -9_000),
            row("zong'guo", "总国务", 50, false),
            row("zong'guo", "中国", 1_000_000, true),
        ];
        rank_fuzzy_rows_with_exact_rows(&mut led_by_sentence);
        assert_eq!(words(&led_by_sentence), ["总国", "中国", "总国务"]);
    }

    #[test]
    fn buckets_without_a_move_are_left_untouched_and_do_not_allocate() {
        let build = || {
            vec![
                row("zi'qian", "自谦", 5_000, false),
                sentence("zi'qian", "自前", 2_000_000),
                row("zi'qian", "支前", 5_000 * FUZZY_WEIGHT_DIVISOR, true),
                // 这一档只有整句和模糊行：没有字面词库行可比。
                sentence("zi'qi", "自欺", 10),
                row("zi'qi", "之期", 9_000_000, true),
                row("zi", "自", 100, false),
            ]
        };
        let mut rows = build();
        let ((), allocations) = crate::ime::personal_rerank::allocations::count(|| {
            rank_fuzzy_rows_with_exact_rows(&mut rows);
        });
        assert_eq!(rows, build());
        assert_eq!(allocations, 0, "没有模糊行要挪却申请了内存: {allocations}");
        rank_fuzzy_rows_with_exact_rows(&mut []);
    }
}
