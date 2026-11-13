use crate::types::WordItem;

/// 补全词能结束组字，但完整读音还有未输入的音节，不能作为整段按键的模型首选。
pub(super) fn is_longer_pinyin_completion(row: &WordItem) -> bool {
    row.source.is_dictionary()
        && !row.pinyin.is_empty()
        && row
            .canonical_pinyin
            .strip_prefix(row.pinyin.as_str())
            .is_some_and(|suffix| suffix.starts_with('\''))
}
