use super::*;
use crate::ime::personal_rerank::allocations::count;

// 固定 f355dc4d8dfdc3ca20e4365ddf99975255bd87ba 的完整 beam，只改名并移除注释；展开规则本片不变。
pub(crate) fn legacy_started_segmentations<S: AsRef<str>>(
    segments: &[S],
    options: FuzzyPinyinOptions,
    limit: usize,
) -> Vec<Vec<String>> {
    if options.rules == 0 || segments.is_empty() || limit == 0 {
        return Vec::new();
    }
    let mut paths: Vec<Vec<String>> = vec![Vec::new()];
    for syllable in segments {
        let variants = fuzzy_syllables(syllable.as_ref(), options);
        let alternatives = variants.as_slice();
        if let [alternative] = alternatives {
            for path in &mut paths {
                path.push((*alternative).to_owned());
            }
            continue;
        }
        let capacity = limit.min(paths.len().saturating_mul(alternatives.len()));
        let mut next = Vec::with_capacity(capacity);
        'beam: for mut path in paths {
            let take = (limit - next.len()).min(alternatives.len());
            for alternative in alternatives.iter().take(take.saturating_sub(1)) {
                let mut extended = Vec::with_capacity(path.len() + 1);
                extended.extend_from_slice(&path);
                extended.push((*alternative).to_owned());
                next.push(extended);
            }
            if take == 0 {
                break 'beam;
            }
            path.push(alternatives[take - 1].to_owned());
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

fn assert_lazy_matches<S: AsRef<str>>(segments: &[S], options: FuzzyPinyinOptions, limit: usize) {
    let (expected, old_count) = count(|| legacy_started_segmentations(segments, options, limit));
    let (actual, new_count) = count(|| fuzzy_segmentations(segments, options, limit));
    assert_eq!(actual, expected);
    if expected.is_empty() {
        assert_eq!(
            new_count, 0,
            "仅原切分仍有分配: rules={}, limit={limit}",
            options.rules
        );
        assert_eq!(actual.capacity(), 0);
    } else {
        assert_eq!(new_count, old_count, "实际分叉改变成本: limit={limit}");
        assert_eq!(actual.capacity(), expected.capacity());
        for (actual, expected) in actual.iter().zip(&expected) {
            assert_eq!(actual.capacity(), expected.capacity());
            for (actual, expected) in actual.iter().zip(expected) {
                assert_eq!(actual.capacity(), expected.capacity());
            }
        }
    }
}

#[test]
fn lazy_start_elides_every_original_only_syllable_and_limit_one() {
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
            for limit in [0, 1, 2, 64] {
                assert_lazy_matches(&[syllable], FuzzyPinyinOptions { rules: mask }, limit);
            }
        }
    }
}

#[test]
fn lazy_start_restores_deferred_prefixes_before_real_branches() {
    let _ = super::super::syllables::intact_pinyin_set();
    for count in [0, 1, 3, 4, 5, 8, 17, 40] {
        for prefix in ["guo", "", "合成"] {
            for suffix in [
                vec![],
                vec!["zan"],
                vec!["lan", "chuang"],
                vec!["zan", "guo", "fa"],
            ] {
                let mut input = vec![prefix; count];
                input.extend(suffix);
                let mut owned: Vec<String> =
                    input.iter().map(|value| (*value).to_owned()).collect();
                for rules in [0, fuzzy_rule::Z_ZH, fuzzy_rule::ALL] {
                    let options = FuzzyPinyinOptions { rules };
                    for limit in [0, 1, 2, 3, 5, 63, 64, 65, 128, usize::MAX] {
                        assert_lazy_matches(&input, options, limit);
                        assert_lazy_matches(&owned, options, limit);
                    }
                }
                let actual = fuzzy_segmentations(
                    &owned,
                    FuzzyPinyinOptions {
                        rules: fuzzy_rule::ALL,
                    },
                    64,
                );
                let expected = legacy_started_segmentations(
                    &owned,
                    FuzzyPinyinOptions {
                        rules: fuzzy_rule::ALL,
                    },
                    64,
                );
                owned.clear();
                assert_eq!(actual, expected);
            }
        }
    }
}
