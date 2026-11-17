use super::{
    is_pinyin_word, satisfies_syllable_helpcodes, HelpcodeKeymap, SyllableHelpcode, WordItem,
};

/// 句中辅助码也让原首选退到其他命中行之后；越出词长的约束没有筛过这条前缀，不算命中。
pub(super) fn filter_and_demote(
    candidates: &mut Vec<WordItem>,
    helpcodes: &[SyllableHelpcode],
    keymap: &HelpcodeKeymap,
) {
    if helpcodes.is_empty() {
        return;
    }
    let constrained = |item: &WordItem| {
        is_pinyin_word(item)
            && helpcodes
                .iter()
                .any(|code| code.syllable < item.word.chars().count())
    };
    let demote_first = candidates.first().is_some_and(|item| {
        constrained(item) && satisfies_syllable_helpcodes(helpcodes, &item.word, keymap)
    });
    candidates.retain(|item| {
        !is_pinyin_word(item) || satisfies_syllable_helpcodes(helpcodes, &item.word, keymap)
    });
    if demote_first {
        if let Some(last) = candidates.iter().rposition(constrained) {
            candidates[..=last].rotate_left(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::CandidateSource;

    #[test]
    fn syllable_helpcode_demotion_keeps_unique_hits_and_unconstrained_prefixes() {
        let keymap = HelpcodeKeymap::from_codes(
            [("甲", "ab"), ("乙", "ab"), ("丙", "cd")]
                .map(|(word, code)| (word.to_owned(), code.to_owned()))
                .into_iter()
                .collect(),
        );
        let code = SyllableHelpcode {
            syllable: 1,
            first: b'a',
            second: b'b',
        };
        for (input, expected) in [
            (vec!["甲甲", "甲乙", "甲"], vec!["甲乙", "甲甲", "甲"]),
            (vec!["甲丙", "甲甲", "甲乙"], vec!["甲甲", "甲乙"]),
            (vec!["甲甲", "甲丙", "甲"], vec!["甲甲", "甲"]),
            (vec!["甲", "甲甲", "甲乙"], vec!["甲", "甲甲", "甲乙"]),
        ] {
            let mut rows: Vec<_> = input
                .iter()
                .map(|word| WordItem::new("nini", *word, 100, CandidateSource::Database, "ni'ni"))
                .collect();
            filter_and_demote(&mut rows, &[code], &keymap);
            assert_eq!(
                rows.iter().map(|row| row.word.as_str()).collect::<Vec<_>>(),
                expected,
                "{input:?}",
            );
        }
    }
}
