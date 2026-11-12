//! 模糊音命中的词库行和字面命中的词库行怎么比（#11321）。全拼的合并和全拼的选词调频共用这一把尺：模糊行的权重先打折再和字面行比，调频也在折后的尺度上算、写回时再还原。26 键双拼（按原权重合并）和九键（模糊行整组在后）都不走这里，见 `.agents/notes/implemented/bug-fix/2026-11-12-fuzzy-rows-rank-with-exact-rows.md`。

use crate::types::WordItem;

/// 模糊行和字面行比权重时，模糊行的权重除以这个数。
///
/// 取值来自锁定词库（dict-v2.1.2）上的实测，方法和全部数字在上面那篇笔记里：
/// - 不打折（1）时，权重只略高的模糊词会顶掉用户照字面打的常用词：开全部规则打 `wangshang`，「晚上」排到「网上」前面；打 `hen`，「分」排到「很」前面。除以 2 这两处都回到字面在前。
/// - 再往大取，按模糊读法打的常用词开始掉出首选，丢的比字面拼写收回的多：`xinqi` 的「星期」除以 3 就落到「新奇」后面，`nihai` 的「厉害」除以 4 落到第 8 位。词频第 4000 名起的 300 个词，把后鼻音打成前鼻音时首选命中率是 67.0%（÷2）、64.7%（÷3）、62.0%（÷4）、59.3%（÷8），照字面打同一档词的命中率只从 93.0% 回到 93.3%、93.7%、94.3%。
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
/// - 一条模糊行紧跟在最后一条折后不比它轻的字面词库行后面。所以它绝不排在任何一条不比它轻的字面词库行之前，不管字面行自己是不是按权重排的：首位和备选切分提到第 2 位的词（`merge_alternative_rows`）都可以比后面的字面行轻得多，模糊行不能借它们越过后面更重的行。权重相同时字面行在前。
/// - 落点后面紧跟着的字面词库行如果是「占座」的（它后面还有比它重的字面词库行，即它的座位不是按权重得来的，例如提到第 2 位的备选切分词），模糊行排到它后面，不把它从座位上挤开：`lian` 开 n=l 是「连、立案、念~、粘~、练」，不是「连、念~、粘~、立案、练」。
/// - 比这一档所有字面词库行都重的模糊行排到第一条字面词库行前面，首位和占座的行顺次后移一位。
/// - 那条字面词库行后面若跟着整句块，模糊行排在整句块前面：模糊整词也是整词。但它后面已经没有别的字面词库行时，模糊行留在档尾原位，所以排不过任何字面行的模糊行和以前一样在最后。
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
    let Some(last_exact) = bucket.iter().rposition(is_exact_dictionary_row) else {
        return;
    };
    // 合并交来的形状：模糊行都在档尾。它们都不比最后一条字面词库行重时，谁也不用挪。
    let settled = bucket[..=last_exact].iter().all(|row| !row.fuzzy)
        && bucket[last_exact + 1..]
            .iter()
            .all(|row| !row.fuzzy || ranking_weight(row) <= bucket[last_exact].weight);
    if settled {
        return;
    }
    let (fuzzy, kept): (Vec<WordItem>, Vec<WordItem>) = bucket
        .iter_mut()
        .map(std::mem::take)
        .partition(|row| row.fuzzy);
    // 每条字面词库行在 `kept` 里的下标、从它起（含它）往后最重的字面词库权重（单调不增，可以二分），以及它是不是占座的（后面还有比它重的字面词库行）。
    let mut seats: Vec<(usize, i64, bool)> = kept
        .iter()
        .enumerate()
        .filter(|(_, row)| is_exact_dictionary_row(row))
        .map(|(index, row)| (index, row.weight, false))
        .collect();
    let mut ceiling = i64::MIN;
    for seat in seats.iter_mut().rev() {
        seat.2 = ceiling > seat.1;
        ceiling = ceiling.max(seat.1);
        seat.1 = ceiling;
    }
    // 模糊行前面该有几条 `kept` 行。
    let mut placed: Vec<(usize, WordItem)> = fuzzy
        .into_iter()
        .map(|row| {
            let weight = ranking_weight(&row);
            // 不比它轻的字面词库行里最后一条是第 `heavier - 1` 条。
            let heavier = seats.partition_point(|&(_, ceiling, _)| ceiling >= weight);
            let before = if heavier == seats.len() {
                kept.len()
            } else if heavier == 0 {
                seats[0].0
            } else {
                // 最后一条字面词库行后面没有更重的行，不会是占座的，所以不会越界。
                let held = seats[heavier..]
                    .iter()
                    .take_while(|&&(_, _, held)| held)
                    .count();
                seats[heavier + held - 1].0 + 1
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

    /// 字面行自己不一定按权重排：备选切分最常用的词被提到第 2 位（`merge_alternative_rows`），可以比后面的字面行轻两个量级。模糊行不能借它越过后面更重的字面行（真实词库上 `huan` 开 uan-uang 的形状，权重取自 dict-v2.1.2）。
    #[test]
    fn a_light_exact_row_in_a_protected_seat_does_not_pull_fuzzy_rows_past_heavier_ones() {
        let fuzzy = |word: &str, weight: i64| {
            let mut item = WordItem::new("huan", word, weight, CandidateSource::Database, "huang");
            item.fuzzy = true;
            item
        };
        let mut alternative = row("hu'an", "湖岸", 7_250, false);
        alternative.canonical_pinyin = "hu'an".into();
        let mut rows = vec![
            row("huan", "换", 555_252, false),
            alternative,
            row("huan", "还", 544_146, false),
            row("huan", "欢", 73_034, false),
            row("huan", "环", 58_260, false),
            row("huan", "缓", 28_493, false),
            row("huan", "幻", 14_012, false),
            fuzzy("黄", 172_829),
            fuzzy("皇", 29_635),
            fuzzy("煌", 19_518),
            fuzzy("凰", 12_736),
        ];
        rank_fuzzy_rows_with_exact_rows(&mut rows);
        // 折后：黄 86414、皇 14817、煌 9759、凰 6368。「黄」只越过比它轻的「欢」以下，「凰」比「湖岸」轻，但它前面最后一条不比它轻的字面行是「幻」。
        assert_eq!(
            words(&rows),
            ["换", "湖岸", "还", "黄", "欢", "环", "缓", "皇", "幻", "煌", "凰"]
        );
    }

    /// 提到第 2 位的备选切分词留在它的座位上：落在首位后面的模糊行排到它后面，不把它挤开；比所有字面行都重的模糊行到最前面，它顺次后移一位（真实词库上 `lian` 开 n=l 的形状）。
    #[test]
    fn fuzzy_rows_do_not_push_a_seated_light_row_out_of_its_seat() {
        let fuzzy = |word: &str, weight: i64| {
            let mut item = WordItem::new("lian", word, weight, CandidateSource::Database, "nian");
            item.fuzzy = true;
            item
        };
        let build = |top: Option<WordItem>| {
            let mut rows = vec![
                row("lian", "连", 1_667_029, false),
                row("li'an", "立案", 39_321, false),
                row("lian", "练", 300_555, false),
                row("lian", "联", 200_000, false),
            ];
            rows.extend(top);
            rows.push(fuzzy("念", 1_603_256));
            rows.push(fuzzy("粘", 840_650));
            rows.push(fuzzy("黏", 258_683));
            rank_fuzzy_rows_with_exact_rows(&mut rows);
            rows
        };
        assert_eq!(
            words(&build(None)),
            ["连", "立案", "念", "粘", "练", "联", "黏"]
        );
        assert_eq!(
            words(&build(Some(fuzzy("年", 12_826_037)))),
            ["年", "连", "立案", "念", "粘", "练", "联", "黏"]
        );
    }

    /// 固定种子的伪随机列表：字面行的次序任意（不按权重），夹着整句行。合并后 (1) 没有一条模糊行排在同一档里折后权重不比它轻的字面词库行前面，(2) 非模糊行的相对次序不变，(3) 行数和内容不变。
    #[test]
    fn no_fuzzy_row_ever_precedes_a_heavier_exact_row_whatever_order_the_exact_rows_are_in() {
        let mut state = 0x9e37_79b9_7f4a_7c15_u64;
        let mut next = |bound: u64| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state % bound
        };
        for round in 0..2_000 {
            let mut rows = Vec::new();
            for (bucket, pinyin) in ["zi'qian", "zi'qi", "zi"].into_iter().enumerate() {
                for index in 0..next(9) {
                    let word = format!("{round}-{bucket}-{index}");
                    // 权重范围小，相等和零、负数都会出现。
                    let weight = next(40) as i64 - 4;
                    rows.push(match next(4) {
                        0 => sentence(pinyin, &word, next(1_000_000) as i64 - 500_000),
                        1 => row(pinyin, &word, weight, true),
                        _ => row(pinyin, &word, weight, false),
                    });
                }
            }
            let before = rows.clone();
            rank_fuzzy_rows_with_exact_rows(&mut rows);
            for (seat, fuzzy) in rows.iter().enumerate().filter(|(_, row)| row.fuzzy) {
                let overtaken = rows[seat + 1..].iter().find(|later| {
                    is_exact_dictionary_row(later)
                        && covered_keys(&later.pinyin) == covered_keys(&fuzzy.pinyin)
                        && later.weight >= ranking_weight(fuzzy)
                });
                assert!(
                    overtaken.is_none(),
                    "第 {round} 轮：{fuzzy:?} 排在 {overtaken:?} 前面\n{before:?}"
                );
            }
            let kept = |rows: &[WordItem]| -> Vec<WordItem> {
                rows.iter().filter(|row| !row.fuzzy).cloned().collect()
            };
            assert_eq!(
                kept(&rows),
                kept(&before),
                "第 {round} 轮：非模糊行换了先后"
            );
            let sorted = |rows: &[WordItem]| {
                let mut words: Vec<String> = rows.iter().map(|row| row.word.to_string()).collect();
                words.sort();
                words
            };
            assert_eq!(sorted(&rows), sorted(&before));
            assert!(rows
                .windows(2)
                .all(|pair| covered_keys(&pair[0].pinyin) >= covered_keys(&pair[1].pinyin)));
        }
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
