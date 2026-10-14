use super::*;
use crate::ime::personal_rerank::allocations::count;

// 固定 e9861e5ae4cf498d291bc25fd9128cb9fe158f20 的完整拥有展开与 beam，只改名、转接冻结入口并移除注释。
pub(crate) fn legacy_syllables(syllable: &str, options: FuzzyPinyinOptions) -> Vec<String> {
    if options.rules == 0 || !is_intact(syllable) {
        return vec![syllable.to_owned()];
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
    let mut result = Vec::with_capacity(starts.len().saturating_mul(ends.len()));
    result.push(syllable.to_owned());
    let mut buffer = [0; MAX_SYLLABLE_LENGTH];
    for (index, start) in starts.iter().enumerate() {
        for end in ends.iter().skip(usize::from(index == 0)) {
            let Some(bytes) = buffer.get_mut(..start.len() + end.len()) else {
                continue;
            };
            bytes[..start.len()].copy_from_slice(start.as_bytes());
            bytes[start.len()..].copy_from_slice(end.as_bytes());
            if let Some(candidate) = intact_piece(bytes) {
                if !result.iter().any(|variant| variant == candidate) {
                    result.push(candidate.to_owned());
                }
            }
        }
    }
    result
}

pub(crate) fn legacy_segmentations<S: AsRef<str>>(
    segments: &[S],
    options: FuzzyPinyinOptions,
    limit: usize,
) -> Vec<Vec<String>> {
    if options.rules == 0 || segments.is_empty() || limit == 0 {
        return Vec::new();
    }
    let mut paths: Vec<Vec<String>> = vec![Vec::new()];
    for syllable in segments {
        let mut alternatives = legacy_syllables(syllable.as_ref(), options);
        if let [alternative] = alternatives.as_mut_slice() {
            let last_index = paths.len() - 1;
            for (index, path) in paths.iter_mut().enumerate() {
                path.push(if index == last_index {
                    std::mem::take(alternative)
                } else {
                    alternative.clone()
                });
            }
            continue;
        }
        let capacity = limit.min(paths.len().saturating_mul(alternatives.len()));
        let mut next = Vec::with_capacity(capacity);
        'beam: for mut path in paths {
            let take = (limit - next.len()).min(alternatives.len());
            let last_parent = next.len() + take == capacity;
            let take_alternative = |alternative: &mut String| {
                if last_parent {
                    std::mem::take(alternative)
                } else {
                    alternative.clone()
                }
            };
            for alternative in alternatives.iter_mut().take(take.saturating_sub(1)) {
                let mut extended = Vec::with_capacity(path.len() + 1);
                extended.extend_from_slice(&path);
                extended.push(take_alternative(alternative));
                next.push(extended);
            }
            if take == 0 {
                break 'beam;
            }
            path.push(take_alternative(&mut alternatives[take - 1]));
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
            .map(String::as_str)
            .eq(segments.iter().map(AsRef::as_ref))
    });
    paths
}

// 用冻结展开推导规划成本；必空路径在计量外取冻结 beam 的完整成本，不调用生产规划。
pub(crate) fn storage_savings<S: AsRef<str>>(
    segments: &[S],
    options: FuzzyPinyinOptions,
    limit: usize,
) -> usize {
    if options.rules == 0 || segments.is_empty() || limit == 0 {
        return 0;
    }
    if limit == 1
        || segments
            .iter()
            .all(|syllable| legacy_syllables(syllable.as_ref(), options).len() == 1)
    {
        return count(|| legacy_segmentations(segments, options, limit)).1;
    }
    let mut parents = 1usize;
    let mut saved = 0;
    for syllable in segments {
        let alternatives = legacy_syllables(syllable.as_ref(), options);
        let capacity = limit.min(parents.saturating_mul(alternatives.len()));
        let last_take = (capacity - 1) % alternatives.len() + 1;
        saved += 1 + alternatives[last_take..]
            .iter()
            .filter(|value| !value.is_empty())
            .count();
        parents = capacity;
    }
    saved
}

#[test]
fn borrowed_variant_plans_preserve_every_syllable_rule_without_heap_storage() {
    let _ = super::super::syllables::intact_pinyin_set();
    for syllable in super::super::syllables::intact_pinyin_list()
        .iter()
        .copied()
        .chain([
            "",
            "zh",
            "ZAN",
            "合成",
            "😀",
            "zan'hao",
            "nue",
            "长合成输入输入输入",
        ])
    {
        for mask in std::iter::once(0)
            .chain((0..11).map(|bit| 1 << bit))
            .chain(std::iter::once(fuzzy_rule::ALL))
        {
            let options = FuzzyPinyinOptions { rules: mask };
            let expected = legacy_syllables(syllable, options);
            let (actual, allocations) = count(|| fuzzy_syllables(syllable, options));
            let actual: Vec<&str> = actual.as_slice().iter().map(|item| item.as_ref()).collect();
            let expected: Vec<&str> = expected.iter().map(String::as_str).collect();
            assert_eq!(actual, expected);
            assert_eq!(allocations, 0, "变体规划仍拥有临时值: {syllable}/{mask}");
        }
    }
}

#[test]
fn borrowed_beam_variants_save_planning_and_original_only_storage() {
    let _ = super::super::syllables::intact_pinyin_set();
    let mut saw_partial_last_parent = false;
    for input in [
        vec![],
        vec![""],
        vec!["guo"],
        vec!["zan"],
        vec!["zan", "fa"],
        vec!["lan", "chuang"],
        vec!["zan", "", "hao"],
        vec!["合成", "😀"],
        vec!["ZAN", "hao"],
        vec!["zan'hao"],
        vec!["guo", "zan", "guo"],
        vec!["zan"; 8],
        vec!["guo"; 40],
    ] {
        let owned: Vec<String> = input.iter().map(|piece| (*piece).to_owned()).collect();
        for mask in [0, fuzzy_rule::Z_ZH, fuzzy_rule::ALL] {
            let options = FuzzyPinyinOptions { rules: mask };
            for limit in [0, 1, 2, 3, 5, 63, 64, 65, 128, usize::MAX] {
                let saved = storage_savings(&owned, options, limit);
                let (expected, old_count) = count(|| legacy_segmentations(&owned, options, limit));
                saw_partial_last_parent |= !expected.is_empty() && saved > owned.len();
                let (actual, new_count) = count(|| fuzzy_segmentations(&owned, options, limit));
                assert_eq!(actual, expected);
                assert_eq!(
                    new_count + saved,
                    old_count,
                    "beam {input:?}/{mask}/{limit}，目标省{saved}"
                );
                let capacity = if expected.is_empty() {
                    0
                } else {
                    expected.capacity()
                };
                assert_eq!(actual.capacity(), capacity);
                for (actual, expected) in actual.iter().zip(&expected) {
                    assert_eq!(actual.capacity(), expected.capacity());
                    for (actual, expected) in actual.iter().zip(expected) {
                        assert_eq!(actual.capacity(), expected.capacity());
                    }
                }
                let (borrowed, borrowed_count) =
                    count(|| fuzzy_segmentations(&input, options, limit));
                assert_eq!(borrowed, actual);
                assert_eq!(borrowed_count, new_count);
            }
        }
    }
    assert!(saw_partial_last_parent);
}
