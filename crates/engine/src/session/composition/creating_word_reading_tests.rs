use super::{append_canonical_pinyin, CreatingWordProgress, InputSession, SelectionTransition};
use crate::ime::personal_rerank::allocations::{count, measure};
use crate::pinyin::segment::is_complete_pinyin_input;
use crate::text::count_han_chars;

// 冻结 `f2c7d47dad0e56208cf0fa532c4a77ea0db6e859` 的完整校验和造词进度，仅适配函数名、旧调用和中文注释。
fn original_normalize_canonical_pinyin_for_word(pinyin: &str, word: &str) -> String {
    if pinyin.is_empty() {
        return String::new();
    }
    let segment_count = pinyin.bytes().filter(|&byte| byte == b'\'').count() + 1;
    if segment_count != count_han_chars(word) {
        return String::new();
    }
    if pinyin
        .split('\'')
        .any(|segment| segment.is_empty() || !is_complete_pinyin_input(segment))
    {
        return String::new();
    }
    pinyin.to_owned()
}

fn original_update_creating_word_progress(
    current_pinyin: &str,
    current_word: &str,
    selected_word: &str,
    transition: &SelectionTransition,
) -> CreatingWordProgress {
    let mut word = String::with_capacity(current_word.len() + selected_word.len());
    word.push_str(current_word);
    word.push_str(selected_word);
    if transition.wubi_native {
        // 五笔短语不由部分选择拼接：一个五笔码对应一个词。
        return CreatingWordProgress {
            pinyin: if current_pinyin.is_empty() {
                transition.full_pure_pinyin.clone()
            } else {
                current_pinyin.to_owned()
            },
            preedit: word.clone(),
            word,
            completed: true,
            can_store: false,
        };
    }
    let selected_canonical = original_normalize_canonical_pinyin_for_word(
        &transition.selected_canonical_pinyin,
        selected_word,
    );
    let prior_parts_storable = current_word.is_empty() || !current_pinyin.is_empty();
    let pinyin = if prior_parts_storable && !selected_canonical.is_empty() {
        append_canonical_pinyin(current_pinyin, &selected_canonical)
    } else {
        String::new()
    };
    let completed = !transition.continues_composition;
    let can_store =
        completed && !original_normalize_canonical_pinyin_for_word(&pinyin, &word).is_empty();
    let mut preedit =
        String::with_capacity(word.len() + transition.current_segmentation_with_cases.len());
    preedit.push_str(&word);
    preedit.push_str(&transition.current_segmentation_with_cases);
    CreatingWordProgress {
        preedit,
        pinyin,
        word,
        completed,
        can_store,
    }
}

fn assert_same(new: &CreatingWordProgress, old: &CreatingWordProgress) {
    assert_eq!(new, old);
    assert_eq!(new.pinyin.capacity(), old.pinyin.capacity());
    assert_eq!(new.word.capacity(), old.word.capacity());
    assert_eq!(new.preedit.capacity(), old.preedit.capacity());
}

fn saved_outputs(current: &CreatingWordProgress, selected: &str, word: &str, wubi: bool) -> usize {
    if wubi {
        return 0;
    }
    usize::from(!original_normalize_canonical_pinyin_for_word(selected, word).is_empty())
        + usize::from(current.can_store)
}

#[test]
fn creating_word_progress_does_not_copy_readings_for_validation() {
    assert!(is_complete_pinyin_input("ni"));
    let transition = SelectionTransition {
        selected_canonical_pinyin: "ni'hao".into(),
        ..SelectionTransition::default()
    };
    let (old, old_count) =
        count(|| original_update_creating_word_progress("", "", "合成", &transition));
    let (new, new_count) =
        count(|| InputSession::update_creating_word_progress("", "", "合成", &transition));
    assert_same(&new, &old);
    assert!(new.completed && new.can_store);
    eprintln!("progress old={old_count} new={new_count}");
    assert_eq!(new_count + 2, old_count);
    assert_eq!(new_count, 3);
}

#[test]
fn creating_word_progress_preserves_gaps_unicode_invalid_readings_and_wubi() {
    assert!(is_complete_pinyin_input("ni"));
    for (prior, prior_word, word, selected, tail, continuing, wubi) in [
        ("", "", "合成", "ni'hao", "", false, false),
        ("ni", "合", "成", "hao", "ma", true, false),
        ("ni", "合", "成", "hao", "🦊", false, false),
        ("", "合", "成", "hao", "ma", false, false),
        ("h", "合", "成", "hao", "", false, false),
        ("ni", "", "合", "hao", "", false, false),
        ("ni'hao", "a🦊", "合", "ma", "", false, false),
        ("", "", "a🦊", "ni'hao", "", false, false),
        ("", "", "a\u{301}", "ni'hao", "", false, false),
        ("", "", "𠀀〇", "ni'hao", "", false, false),
        ("", "", "合", "nihao", "", false, false),
        ("", "", "合成", "ni'h", "", false, false),
        ("", "", "合成", "Ni'hao", "", false, false),
        ("", "", "合成", "'ni", "", false, false),
        ("", "", "合成", "ni'", "", false, false),
        ("", "", "合成甲", "ni''hao", "", false, false),
        ("", "", "合成", "猫'hao", "", false, false),
        ("", "", "合", "ni’hao", "", false, false),
        ("", "", "合", "", "", false, false),
        ("ni", "合", "", "", "", false, false),
        ("", "", "", "", "尾🦊", true, false),
        ("", "", "合", "ni", "", true, true),
        ("abcd", "合", "成", "hao", "", false, true),
    ] {
        let transition = SelectionTransition {
            selected_canonical_pinyin: selected.into(),
            full_pure_pinyin: "wxyz".into(),
            current_segmentation_with_cases: tail.into(),
            continues_composition: continuing,
            wubi_native: wubi,
        };
        let (old, old_count) =
            count(|| original_update_creating_word_progress(prior, prior_word, word, &transition));
        let (new, new_count) = count(|| {
            InputSession::update_creating_word_progress(prior, prior_word, word, &transition)
        });
        assert_same(&new, &old);
        assert_eq!(new.completed, wubi || !continuing);
        assert_eq!(
            new_count + saved_outputs(&old, selected, word, wubi),
            old_count
        );
    }
    for selected in [
        "jv", "nue", "lue", "sahng", "mihng", "ng", "v", "ni\0", "ni;--",
    ] {
        let transition = SelectionTransition {
            selected_canonical_pinyin: selected.into(),
            ..SelectionTransition::default()
        };
        let old = original_update_creating_word_progress("", "", "合", &transition);
        let new = InputSession::update_creating_word_progress("", "", "合", &transition);
        assert_same(&new, &old);
        if !new.pinyin.is_empty() {
            assert_eq!(new.pinyin, selected);
        }
    }
}

#[test]
fn creating_word_progress_keeps_long_reading_validation_and_short_circuit_order() {
    assert!(is_complete_pinyin_input("ni"));
    for length in [63, 64, 65] {
        let long = "a".repeat(length);
        for (prior, prior_word, word, selected, continuing, wubi) in [
            ("", "", "合", long.as_str(), false, false),
            ("", "合", "成", long.as_str(), false, false),
            ("", "", "合", long.as_str(), true, false),
            (long.as_str(), "合", "成", "hao", false, false),
            ("h", "合", "成", long.as_str(), false, false),
            ("", "", "合", long.as_str(), false, true),
        ] {
            let transition = SelectionTransition {
                selected_canonical_pinyin: selected.into(),
                continues_composition: continuing,
                wubi_native: wubi,
                ..SelectionTransition::default()
            };
            let (old, old_count) = count(|| {
                original_update_creating_word_progress(prior, prior_word, word, &transition)
            });
            let (new, new_count) = count(|| {
                InputSession::update_creating_word_progress(prior, prior_word, word, &transition)
            });
            assert_same(&new, &old);
            assert_eq!(
                new_count + saved_outputs(&old, selected, word, wubi),
                old_count
            );
            if prior_word == "合" && prior.is_empty() {
                // 即使先前存在缺口，原路径仍先校验选中读音；长段保留堆回退。
                assert_eq!(new_count, 2 + usize::from(length > 64));
                assert!(!new.can_store && new.pinyin.is_empty());
            }
        }
    }
    let selected = vec!["ni"; 1025].join("'");
    let word = "合".repeat(1025);
    let transition = SelectionTransition {
        selected_canonical_pinyin: selected,
        ..SelectionTransition::default()
    };
    let (old, old_count) =
        count(|| original_update_creating_word_progress("", "", &word, &transition));
    let (new, new_count) =
        count(|| InputSession::update_creating_word_progress("", "", &word, &transition));
    assert_same(&new, &old);
    assert!(new.can_store);
    assert_eq!(new_count + 2, old_count);
    assert_eq!(new_count, 3);
}

#[test]
fn cold_creating_word_progress_releases_validation_outputs_and_owns_returned_fields() {
    // 区间外初始化全局音节表；区间内只借用输入与 transition，仅返回进度的三个拥有型字段。
    assert!(is_complete_pinyin_input("ni"));
    let long = "ni".repeat(33);
    for (case, prior, prior_word, word, selected, continuing, wubi) in [
        ("complete", "", "", "合成", "ni'hao", false, false),
        ("continue", "ni", "合", "成", "hao", true, false),
        ("prior-gap", "", "合", "成", "hao", false, false),
        ("invalid", "ni", "合", "成", "h", false, false),
        ("unknown", "", "", "合成", "", false, false),
        ("long", "", "", "合", long.as_str(), false, false),
        ("wubi", "", "", "合成", "ni'hao", false, true),
    ] {
        let transition = SelectionTransition {
            selected_canonical_pinyin: selected.into(),
            full_pure_pinyin: "abcd".into(),
            current_segmentation_with_cases: "ma".into(),
            continues_composition: continuing,
            wubi_native: wubi,
        };
        let (old, old_heap) = measure(|| {
            original_update_creating_word_progress(prior, prior_word, word, &transition)
        });
        let (new, new_heap) = measure(|| {
            InputSession::update_creating_word_progress(prior, prior_word, word, &transition)
        });
        assert_same(&new, &old);
        assert_eq!(
            new_heap.allocations + saved_outputs(&old, selected, word, wubi),
            old_heap.allocations
        );
        for (result, heap) in [(&old, old_heap), (&new, new_heap)] {
            let retained =
                result.pinyin.capacity() + result.word.capacity() + result.preedit.capacity();
            assert_eq!(heap.remaining_bytes, retained as i128);
            assert_eq!(heap.minimum_bytes, 0);
        }
        assert!(new_heap.peak_bytes <= old_heap.peak_bytes);
        eprintln!("case={case} old={old_heap:?} new={new_heap:?}");
    }
    let owned = {
        let prior = String::from("ni");
        let prior_word = String::from("合");
        let selected_word = String::from("成");
        let transition = SelectionTransition {
            selected_canonical_pinyin: "hao".into(),
            current_segmentation_with_cases: "ma".into(),
            ..SelectionTransition::default()
        };
        InputSession::update_creating_word_progress(
            &prior,
            &prior_word,
            &selected_word,
            &transition,
        )
    };
    assert_eq!(owned.pinyin, "ni'hao");
    assert_eq!(owned.word, "合成");
    assert_eq!(owned.preedit, "合成ma");
    assert!(owned.completed && owned.can_store);
    assert_ne!(owned.word.as_ptr(), owned.preedit.as_ptr());
}
