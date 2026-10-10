use super::normalize_canonical_pinyin_for_word;
use crate::ime::personal_rerank::allocations::{count, measure};
use crate::pinyin::segment::{is_complete_pinyin_input, join_segments, split_segments};
use crate::text::count_han_chars;

// 冻结 `6af0b0a780b23c5fd8f1fe1a25ca88720f152504` 的完整旧校验，仅改函数名。
fn original_normalize_canonical_pinyin_for_word(pinyin: &str, word: &str) -> String {
    if pinyin.is_empty() {
        return String::new();
    }
    let segments = split_segments(pinyin);
    if segments.is_empty() || segments.len() != count_han_chars(word) {
        return String::new();
    }
    if segments
        .iter()
        .any(|segment| segment.is_empty() || !is_complete_pinyin_input(segment))
    {
        return String::new();
    }
    join_segments(&segments)
}

#[test]
fn canonical_reading_does_not_copy_temporary_segments() {
    assert!(is_complete_pinyin_input("ni'hao"));
    let (old, old_count) = count(|| original_normalize_canonical_pinyin_for_word("ni'hao", "合成"));
    let (new, new_count) = count(|| normalize_canonical_pinyin_for_word("ni'hao", "合成"));
    assert_eq!(new, old);
    assert_eq!(new.capacity(), old.capacity());
    eprintln!("canonical old={old_count} new={new_count}");
    assert_eq!(new_count + 3, old_count);
    assert_eq!(new_count, 1);
}

#[test]
fn canonical_reading_preserves_characters_empty_parts_and_complete_cuts() {
    assert!(is_complete_pinyin_input("ni"));
    for (pinyin, word, expected) in [
        ("", "", ""),
        ("", "合", ""),
        ("ni", "", ""),
        ("ni", "合", "ni"),
        ("nihao", "合", "nihao"),
        ("nihao", "合成", ""),
        ("ni'hao", "合成", "ni'hao"),
        ("ni'hao", "A🦊", "ni'hao"),
        ("ni'hao", "𠀀〇", "ni'hao"),
        ("ni'hao", "a\u{301}", "ni'hao"),
        ("ni'hao", "合", ""),
        ("ni'h", "合成", ""),
        ("'ni", "合成", ""),
        ("ni'", "合成", ""),
        ("ni''hao", "合成甲", ""),
        ("'", "合成", ""),
        ("猫'hao", "合成", ""),
        ("ni’hao", "合", ""),
        ("Ni'hao", "合成", ""),
    ] {
        let (old, old_count) = count(|| original_normalize_canonical_pinyin_for_word(pinyin, word));
        let (new, new_count) = count(|| normalize_canonical_pinyin_for_word(pinyin, word));
        assert_eq!(new, expected, "pinyin={pinyin:?} word={word:?}");
        assert_eq!(new, old);
        assert_eq!(new.capacity(), old.capacity());
        let split_count = count(|| split_segments(pinyin)).1;
        assert_eq!(new_count + split_count, old_count);
    }
    // 原校验不做别名改写，完整性和原始拼写以冻结实现为准。
    for pinyin in [
        "jv", "nue", "lue", "sahng", "mihng", "ng", "v", "ni\0", "ni;--",
    ] {
        let old = original_normalize_canonical_pinyin_for_word(pinyin, "合");
        let new = normalize_canonical_pinyin_for_word(pinyin, "合");
        assert_eq!(new, old);
        assert_eq!(new.capacity(), old.capacity());
        if !new.is_empty() {
            assert_eq!(new, pinyin);
        }
    }
}

#[test]
fn canonical_reading_preserves_long_input_storage_and_short_circuiting() {
    assert!(is_complete_pinyin_input("ni"));
    let many_segments = vec!["ni"; 1025].join("'");
    let many_characters = "合".repeat(1025);
    let long_valid = "ni".repeat(33);
    let long_invalid = format!("{long_valid}z");
    let first_invalid = format!("h'{long_valid}");
    for (pinyin, word, valid) in [
        (many_segments.as_str(), many_characters.as_str(), true),
        (long_valid.as_str(), "合", true),
        (long_invalid.as_str(), "合", false),
        (long_valid.as_str(), "合成", false),
        (first_invalid.as_str(), "合成", false),
    ] {
        let (old, old_count) = count(|| original_normalize_canonical_pinyin_for_word(pinyin, word));
        let (new, new_count) = count(|| normalize_canonical_pinyin_for_word(pinyin, word));
        assert_eq!(!new.is_empty(), valid);
        assert_eq!(new, old);
        assert_eq!(new.capacity(), old.capacity());
        assert_eq!(new_count + count(|| split_segments(pinyin)).1, old_count);
        if word == "合成" {
            assert_eq!(new_count, 0, "先计数或遇到首段错误，不检查长后段");
        }
    }
    for length in [63, 64, 65] {
        let pinyin = "a".repeat(length);
        let (old, old_count) =
            count(|| original_normalize_canonical_pinyin_for_word(&pinyin, "合"));
        let (new, new_count) = count(|| normalize_canonical_pinyin_for_word(&pinyin, "合"));
        assert_eq!(new, pinyin);
        assert_eq!(new, old);
        assert_eq!(new.capacity(), old.capacity());
        assert_eq!(new_count + 2, old_count);
        assert_eq!(new_count, if length > 64 { 2 } else { 1 });
    }
}

#[test]
fn cold_canonical_reading_releases_temporary_storage_and_keeps_owned_output() {
    // 区间外初始化全局音节表；区间内仅借用外部输入，只返回独立拥有的结果。
    assert!(is_complete_pinyin_input("ni"));
    let long = "ni".repeat(33);
    for (case, pinyin, word) in [
        ("valid", "ni'hao", "合成"),
        ("mismatch", "ni'hao", "合"),
        ("empty-part", "ni''hao", "合成甲"),
        ("invalid", "ni'h", "合成"),
        ("unicode", "猫'hao", "合成"),
        ("long", long.as_str(), "合"),
        ("empty", "", "合"),
    ] {
        let (old, old_heap) =
            measure(|| original_normalize_canonical_pinyin_for_word(pinyin, word));
        let (new, new_heap) = measure(|| normalize_canonical_pinyin_for_word(pinyin, word));
        assert_eq!(new, old);
        assert_eq!(new.capacity(), old.capacity());
        assert_eq!(
            new_heap.allocations + count(|| split_segments(pinyin)).1,
            old_heap.allocations
        );
        assert_eq!(old_heap.remaining_bytes, old.capacity() as i128);
        assert_eq!(new_heap.remaining_bytes, new.capacity() as i128);
        assert_eq!(old_heap.minimum_bytes, 0);
        assert_eq!(new_heap.minimum_bytes, 0);
        assert!(new_heap.peak_bytes <= old_heap.peak_bytes);
        eprintln!("case={case} old={old_heap:?} new={new_heap:?}");
    }
    let owned = {
        let input = String::from("ni'hao");
        let word = String::from("合成");
        let result = normalize_canonical_pinyin_for_word(&input, &word);
        assert_ne!(result.as_ptr(), input.as_ptr());
        result
    };
    assert_eq!(owned, "ni'hao");
}
