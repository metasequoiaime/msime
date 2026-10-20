//! Fuzzy pinyin expansion (quanpin.md §6, `R/quanpin/fuzzy_pinyin.h`). The dictionary side (`fuzzy_candidates`) is in `quanpin::dictionary`.

use std::borrow::Cow;

use super::syllables::{intact_piece, is_intact, MAX_SYLLABLE_LENGTH};
use crate::types::{fuzzy_rule, FuzzyPinyinOptions};

pub const FUZZY_SEGMENTATION_LIMIT: usize = 64;

const INITIAL_PAIRS: [(&str, &str, u32); 6] = [
    ("z", "zh", fuzzy_rule::Z_ZH),
    ("c", "ch", fuzzy_rule::C_CH),
    ("s", "sh", fuzzy_rule::S_SH),
    ("n", "l", fuzzy_rule::N_L),
    ("f", "h", fuzzy_rule::F_H),
    ("r", "l", fuzzy_rule::R_L),
];

const FINAL_PAIRS: [(&str, &str, u32); 5] = [
    ("an", "ang", fuzzy_rule::AN_ANG),
    ("en", "eng", fuzzy_rule::EN_ENG),
    ("in", "ing", fuzzy_rule::IN_ING),
    ("ian", "iang", fuzzy_rule::IAN_IANG),
    ("uan", "uang", fuzzy_rule::UAN_UANG),
];

/// 返回已启用规则中与声母互为模糊读法的伙伴，不包含声母自身。
pub(crate) fn fuzzy_initial_partners(
    initial: &str,
    options: FuzzyPinyinOptions,
) -> impl Iterator<Item = &'static str> + '_ {
    INITIAL_PAIRS
        .iter()
        .filter_map(move |&(left, right, rule)| {
            if !options.enabled(rule) {
                None
            } else if initial == left {
                Some(right)
            } else if initial == right {
                Some(left)
            } else {
                None
            }
        })
}

/// The partners of `part` under every enabled pair, `part` itself first.
fn with_partners<'a, const N: usize>(
    part: &'a str,
    pairs: &[(&'a str, &'a str, u32)],
    options: FuzzyPinyinOptions,
) -> ([&'a str; N], usize) {
    let mut variants = [part; N];
    let mut count = 1;
    for &(a, b, rule) in pairs {
        if !options.enabled(rule) {
            continue;
        }
        if part == a {
            debug_assert!(count < N);
            variants[count] = b;
            count += 1;
        } else if part == b {
            debug_assert!(count < N);
            variants[count] = a;
            count += 1;
        }
    }
    (variants, count)
}

// 原音节加全部伙伴组合，上界直接来自规则表，不限制调用方输入。
const FUZZY_VARIANT_CAPACITY: usize = (INITIAL_PAIRS.len() + 1) * (FINAL_PAIRS.len() + 1);

struct FuzzySyllables<'a> {
    values: [&'a str; FUZZY_VARIANT_CAPACITY],
    count: usize,
}

impl<'a> FuzzySyllables<'a> {
    fn as_slice(&self) -> &[&'a str] {
        &self.values[..self.count]
    }
}

/// 原音节在首位，随后按规则保留合法且不同的变体；输入与静态音节均只借用（FZ:10-68）。
fn fuzzy_syllables(syllable: &str, options: FuzzyPinyinOptions) -> FuzzySyllables<'_> {
    let mut result = FuzzySyllables {
        values: [syllable; FUZZY_VARIANT_CAPACITY],
        count: 1,
    };
    // 只扩展完整音节，未完成的前缀保留普通补全。
    if options.rules == 0 || !is_intact(syllable) {
        return result;
    }
    let initial_length = if ["zh", "ch", "sh"]
        .iter()
        .any(|initial| syllable.starts_with(initial))
    {
        2
    } else if syllable.starts_with(|c: char| "bpmfdtnlgkhjqxrzcsyw".contains(c)) {
        1
    } else {
        0
    };
    let (initial, final_part) = syllable.split_at(initial_length);
    let (starts, start_count) =
        with_partners::<{ INITIAL_PAIRS.len() + 1 }>(initial, &INITIAL_PAIRS, options);
    let (ends, end_count) =
        with_partners::<{ FINAL_PAIRS.len() + 1 }>(final_part, &FINAL_PAIRS, options);
    let starts = &starts[..start_count];
    let ends = &ends[..end_count];
    let mut buffer = [0; MAX_SYLLABLE_LENGTH];
    for (index, start) in starts.iter().enumerate() {
        // 原音节已经在首行，跳过两个伙伴列表首项组成的重复组合。
        for end in ends.iter().skip(usize::from(index == 0)) {
            // 超过完整音节表上限的组合必定无效，其余先在栈上验证再分配。
            let Some(bytes) = buffer.get_mut(..start.len() + end.len()) else {
                continue;
            };
            bytes[..start.len()].copy_from_slice(start.as_bytes());
            bytes[start.len()..].copy_from_slice(end.as_bytes());
            if let Some(candidate) = intact_piece(bytes) {
                if !result.as_slice().contains(&candidate) {
                    result.values[result.count] = candidate;
                    result.count += 1;
                }
            }
        }
    }
    result
}

/// 每轮受 `limit` 限制的 beam 笛卡尔积，排除原切分（FZ:71-94）。
pub fn fuzzy_segmentations<S: AsRef<str>>(
    segments: &[S],
    options: FuzzyPinyinOptions,
    limit: usize,
) -> Vec<Vec<String>> {
    if options.rules == 0 || segments.is_empty() || limit <= 1 {
        return Vec::new();
    }
    // 原组合始终在首位；真正分叉前只借用，避免为最终删掉的唯一原路径拥有字符串。
    let mut paths: Vec<Vec<Cow<'_, str>>> = Vec::new();
    for (index, syllable) in segments.iter().enumerate() {
        let variants = fuzzy_syllables(syllable.as_ref(), options);
        let alternatives = variants.as_slice();
        if paths.is_empty() {
            if alternatives.len() == 1 {
                continue;
            }
            // 按原追加顺序恢复前缀，实际分叉保持原路径向量容量策略。
            let mut prefix = Vec::new();
            for segment in &segments[..index] {
                prefix.push(Cow::Borrowed(segment.as_ref()));
            }
            paths = vec![prefix];
        }
        // 单变体不会增加路径数，已有路径满足上限，直接复用当前层容器。
        if let [alternative] = alternatives {
            for path in &mut paths {
                path.push(Cow::Borrowed(*alternative));
            }
            continue;
        }
        let capacity = limit.min(paths.len().saturating_mul(alternatives.len()));
        let mut next = Vec::with_capacity(capacity);
        // 达到上限即停止整轮展开，与参考实现只退出内层循环的截断结果一致。
        'beam: for mut path in paths {
            let take = (limit - next.len()).min(alternatives.len());
            for alternative in alternatives.iter().take(take.saturating_sub(1)) {
                let mut extended = Vec::with_capacity(path.len() + 1);
                extended.extend_from_slice(&path);
                extended.push(Cow::Borrowed(*alternative));
                next.push(extended);
            }
            if take == 0 {
                break 'beam;
            }
            path.push(Cow::Borrowed(alternatives[take - 1]));
            next.push(path);
            if next.len() == limit {
                break 'beam;
            }
        }
        paths = next;
    }
    paths.retain(|path| {
        !path
            .iter()
            .map(AsRef::as_ref)
            .eq(segments.iter().map(AsRef::as_ref))
    });
    // 只为最终返回字段拥有字符串，消费式收集复用内外层路径容器。
    paths
        .into_iter()
        .map(|path| path.into_iter().map(Cow::into_owned).collect())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules(rules: u32) -> FuzzyPinyinOptions {
        FuzzyPinyinOptions { rules }
    }

    // test_fuzzy_pinyin.cpp:66-111: each of the eleven rules forward and reverse, and only under its own bit.
    #[test]
    fn every_rule_expands_both_ways_under_its_own_bit_only() {
        let pairs = [
            ("zan", "zhan"),
            ("can", "chan"),
            ("san", "shan"),
            ("na", "la"),
            ("fa", "ha"),
            ("ran", "lan"),
            ("ban", "bang"),
            ("ben", "beng"),
            ("bin", "bing"),
            ("lian", "liang"),
            ("guan", "guang"),
        ];
        for (bit, (a, b)) in pairs.iter().enumerate() {
            let own = rules(1 << bit);
            assert!(fuzzy_syllables(a, own).as_slice().contains(b), "{a} -> {b}");
            assert!(fuzzy_syllables(b, own).as_slice().contains(a), "{b} -> {a}");
            let other = rules(1 << ((bit + 1) % pairs.len()));
            assert!(
                !fuzzy_syllables(a, other).as_slice().contains(b),
                "{a} under another rule"
            );
        }
    }

    #[test]
    fn expansions_keep_the_original_first_and_stay_intact() {
        let variants = fuzzy_syllables("zhan", rules(fuzzy_rule::ALL));
        assert_eq!(variants.as_slice(), ["zhan", "zhang", "zan", "zang"]);
        assert!(variants.as_slice().iter().all(|variant| is_intact(variant)));
        // `l` 同时有 `n` 和 `r` 两个伙伴。
        assert_eq!(
            fuzzy_syllables("lan", rules(fuzzy_rule::N_L | fuzzy_rule::R_L)).as_slice(),
            ["lan", "nan", "ran"]
        );
        assert_eq!(
            fuzzy_syllables("an", rules(fuzzy_rule::ALL)).as_slice(),
            ["an", "ang"]
        );
        let lan = fuzzy_syllables("lan", rules(fuzzy_rule::ALL));
        assert_eq!(lan.as_slice().len(), 6);
        let (initial_partners, initial_count) = with_partners::<{ INITIAL_PAIRS.len() + 1 }>(
            "l",
            &INITIAL_PAIRS,
            rules(fuzzy_rule::ALL),
        );
        assert_eq!(&initial_partners[..initial_count], ["l", "n", "r"]);
        assert_eq!(
            fuzzy_syllables("zh", rules(fuzzy_rule::ALL)).as_slice(),
            ["zh"]
        );
        assert_eq!(
            fuzzy_syllables("bian", rules(fuzzy_rule::AN_ANG)).as_slice(),
            ["bian"]
        );
        assert_eq!(fuzzy_syllables("zan", rules(0)).as_slice(), ["zan"]);
    }

    #[test]
    fn partner_references_need_no_temporary_heap_state() {
        let (summary, allocations) = crate::ime::personal_rerank::allocations::count(|| {
            let (variants, count) = with_partners::<{ INITIAL_PAIRS.len() + 1 }>(
                "l",
                &INITIAL_PAIRS,
                rules(fuzzy_rule::ALL),
            );
            (count, variants[0], variants[1], variants[2])
        });
        assert_eq!(summary, (3, "l", "n", "r"));
        assert_eq!(allocations, 0);
    }

    #[test]
    fn fuzzy_expansion_does_not_rebuild_the_original_syllable() {
        for (syllable, options, expected, budget) in [
            (
                "zhan",
                rules(fuzzy_rule::ALL),
                vec!["zhan", "zhang", "zan", "zang"],
                0,
            ),
            ("an", rules(fuzzy_rule::ALL), vec!["an", "ang"], 0),
            ("bian", rules(fuzzy_rule::Z_ZH), vec!["bian"], 0),
        ] {
            let _ = fuzzy_syllables(syllable, options);
            let (actual, allocations) = crate::ime::personal_rerank::allocations::count(|| {
                fuzzy_syllables(syllable, options)
            });
            assert_eq!(actual.as_slice(), expected);
            assert_eq!(
                allocations, budget,
                "原音节 {syllable} 被重复构造: {allocations}"
            );
        }
    }

    #[test]
    fn fuzzy_expansion_borrows_only_valid_variant_strings() {
        for (syllable, expected, budget) in [
            ("lian", vec!["lian", "liang", "nian", "niang"], 0),
            ("fo", vec!["fo"], 0),
            ("chuang", vec!["chuang", "chuan", "cuan"], 0),
        ] {
            let options = rules(fuzzy_rule::ALL);
            let _ = fuzzy_syllables(syllable, options);
            let (actual, allocations) = crate::ime::personal_rerank::allocations::count(|| {
                fuzzy_syllables(syllable, options)
            });

            assert_eq!(actual.as_slice(), expected);
            assert_eq!(
                allocations, budget,
                "无效模糊变体仍分配字符串: {syllable}: {allocations}"
            );
        }
    }

    #[test]
    fn every_syllable_expansion_borrows_without_heap_storage() {
        let _ = fuzzy_syllables("lian", rules(fuzzy_rule::ALL));
        for &syllable in super::super::syllables::intact_pinyin_list() {
            for mask in std::iter::once(0)
                .chain((0..11).map(|bit| 1 << bit))
                .chain(std::iter::once(fuzzy_rule::ALL))
            {
                let (variants, allocations) =
                    crate::ime::personal_rerank::allocations::count(|| {
                        fuzzy_syllables(syllable, rules(mask))
                    });
                let variants = variants.as_slice();
                assert_eq!(variants.first().copied(), Some(syllable));
                assert!(variants.iter().all(|variant| is_intact(variant)));
                for (index, variant) in variants.iter().enumerate() {
                    assert!(!variants[..index].contains(variant));
                }
                assert_eq!(allocations, 0, "{syllable}: {mask}: 规划仍分配临时缓冲");
            }
        }
    }

    #[test]
    fn segmentations_are_a_bounded_beam_without_the_original() {
        let typed = vec!["zan".to_owned(), "fa".to_owned()];
        let paths = fuzzy_segmentations(
            &typed,
            rules(fuzzy_rule::Z_ZH | fuzzy_rule::F_H),
            FUZZY_SEGMENTATION_LIMIT,
        );
        let joined: Vec<String> = paths.iter().map(|path| path.join("'")).collect();
        assert_eq!(joined, ["zan'ha", "zhan'fa", "zhan'ha"]);
        let long = vec!["lan".to_owned(); 20];
        assert!(
            fuzzy_segmentations(&long, rules(fuzzy_rule::ALL), FUZZY_SEGMENTATION_LIMIT).len()
                <= 63
        );
        assert!(fuzzy_segmentations(&typed, rules(0), FUZZY_SEGMENTATION_LIMIT).is_empty());
        assert!(fuzzy_segmentations(
            &[] as &[String],
            rules(fuzzy_rule::ALL),
            FUZZY_SEGMENTATION_LIMIT
        )
        .is_empty());
        assert!(fuzzy_segmentations(&typed, rules(fuzzy_rule::ALL), 0).is_empty());
    }

    #[test]
    fn fuzzy_beam_reuses_a_parent_path_for_the_last_alternative() {
        let typed = vec!["zan".to_owned(), "fa".to_owned()];
        let _ = fuzzy_segmentations(&typed, rules(fuzzy_rule::ALL), FUZZY_SEGMENTATION_LIMIT);
        let (paths, allocations) = crate::ime::personal_rerank::allocations::count(|| {
            fuzzy_segmentations(&typed, rules(fuzzy_rule::ALL), FUZZY_SEGMENTATION_LIMIT)
        });

        assert_eq!(paths.len(), 7);
        assert_eq!(allocations, 28);
    }

    #[test]
    fn truncated_beam_copies_only_the_variants_consumed_by_paths() {
        let options = rules(fuzzy_rule::ALL);
        let single = vec!["zan".to_owned()];
        let _ = fuzzy_segmentations(&single, options, FUZZY_SEGMENTATION_LIMIT);
        for (limit, budget) in [(1, 0), (2, 5), (3, 7), (4, 9), (64, 9)] {
            let (paths, allocations) = crate::ime::personal_rerank::allocations::count(|| {
                fuzzy_segmentations(&single, options, limit)
            });
            let expected = ["zang", "zhan", "zhang"];
            assert_eq!(paths.len(), (limit - 1).min(expected.len()));
            for (path, expected) in paths.iter().zip(expected) {
                assert_eq!(path, &[expected]);
            }
            assert_eq!(allocations, budget, "单父路径仍分配未采用的变体: {limit}");
        }

        let pair = vec!["zan".to_owned(), "fa".to_owned()];
        let (paths, allocations) = crate::ime::personal_rerank::allocations::count(|| {
            fuzzy_segmentations(&pair, options, 5)
        });
        assert_eq!(
            paths,
            [
                vec!["zan", "ha"],
                vec!["zang", "fa"],
                vec!["zang", "ha"],
                vec!["zhan", "fa"],
            ]
        );
        assert_eq!(allocations, 20, "截断父路径仍分配未采用的变体");
    }

    #[test]
    fn single_variant_steps_elide_original_only_and_reuse_branch_storage() {
        let options = rules(fuzzy_rule::ALL);
        let _ = fuzzy_syllables("guo", options);
        for (count, budget) in [(0, 0), (32, 0), (1, 0), (4, 0), (8, 0)] {
            let typed = vec!["guo".to_owned(); count];
            let (paths, allocations) = crate::ime::personal_rerank::allocations::count(|| {
                fuzzy_segmentations(&typed, options, FUZZY_SEGMENTATION_LIMIT)
            });
            assert!(paths.is_empty());
            assert_eq!(allocations, budget, "仅原切分仍拥有路径: {count}");
        }

        let typed = vec!["zan".to_owned(), "guo".to_owned()];
        for (limit, budget) in [(1, 0), (2, 7), (3, 11), (4, 15), (64, 15)] {
            let (paths, allocations) = crate::ime::personal_rerank::allocations::count(|| {
                fuzzy_segmentations(&typed, options, limit)
            });
            let expected = ["zang", "zhan", "zhang"];
            assert_eq!(paths.len(), (limit - 1).min(expected.len()));
            for (path, expected) in paths.iter().zip(expected) {
                assert_eq!(path, &[expected, "guo"]);
            }
            assert_eq!(allocations, budget, "多路径单变体仍创建下一层容器: {limit}");
        }
    }

    #[test]
    fn moving_beam_variants_preserves_the_reference_product_and_limits() {
        fn reference(
            segments: &[String],
            options: FuzzyPinyinOptions,
            limit: usize,
        ) -> Vec<Vec<String>> {
            if options.rules == 0 || segments.is_empty() || limit == 0 {
                return Vec::new();
            }
            let mut paths = vec![Vec::new()];
            for syllable in segments {
                let variants = fuzzy_syllables(syllable, options);
                let alternatives = variants.as_slice();
                let mut next = Vec::new();
                'beam: for path in paths {
                    for alternative in alternatives {
                        let mut extended = path.clone();
                        extended.push((*alternative).to_owned());
                        next.push(extended);
                        if next.len() == limit {
                            break 'beam;
                        }
                    }
                }
                paths = next;
            }
            paths.retain(|path| path.as_slice() != segments);
            paths
        }

        for input in [
            vec![],
            vec!["zan"],
            vec!["zan", "fa"],
            vec!["lan", "chuang"],
            vec!["an", "fo", "bian"],
            vec!["zh", "🧪"],
            vec!["guo", "zan"],
            vec!["zan", "guo", "bi"],
            vec!["guo", "zan", "bi"],
        ] {
            let segments: Vec<String> = input.into_iter().map(str::to_owned).collect();
            for mask in [0, fuzzy_rule::Z_ZH, fuzzy_rule::ALL] {
                let options = rules(mask);
                for limit in (0..=20).chain([64, 65, usize::MAX]) {
                    assert_eq!(
                        fuzzy_segmentations(&segments, options, limit),
                        reference(&segments, options, limit),
                        "转移变体改变了路径: {segments:?}, {mask}, {limit}"
                    );
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "fuzzy/variant_view_tests.rs"]
pub(crate) mod variant_view_tests;

#[cfg(test)]
#[path = "fuzzy/lazy_start_tests.rs"]
mod lazy_start_tests;

#[cfg(test)]
#[path = "fuzzy/deferred_string_tests.rs"]
pub(crate) mod deferred_string_tests;
