//! 候选释义的显示分段；范围始终对应原始 UTF-8 字符串，不改缓存和上屏文字。

use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TranslationRunRole {
    Pos,
    Separator,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TranslationRun {
    pub start: usize,
    pub length: usize,
    pub role: TranslationRunRole,
}

/// 与固定参考的词性识别相同：前缀后必须还有内容；孤立的 `n.` 是正文。
pub fn translation_runs(gloss: &str) -> Vec<TranslationRun> {
    const LABELS: [&str; 13] = [
        "proper n. ",
        "interj. ",
        "idiom. ",
        "pron. ",
        "prep. ",
        "conj. ",
        "adj. ",
        "adv. ",
        "num. ",
        "det. ",
        "aux. ",
        "n. ",
        "v. ",
    ];
    let mut runs: Vec<TranslationRun> = Vec::new();
    let mut push = |start: usize, length: usize, role| {
        if let Some(last) = runs.last_mut() {
            if last.role == role && last.start + last.length == start {
                last.length += length;
                return;
            }
        }
        runs.push(TranslationRun {
            start,
            length,
            role,
        });
    };
    let mut start = 0;
    for (end, separator_width) in gloss
        .char_indices()
        .filter(|(_, ch)| matches!(ch, ';' | '；'))
        .map(|(at, ch)| (at, ch.len_utf8()))
        .chain(std::iter::once((gloss.len(), 0)))
    {
        let sense = &gloss[start..end];
        let trimmed = sense.trim_start_matches([' ', '\t', '\r', '\n']);
        if let Some(label) = LABELS
            .iter()
            .find(|label| trimmed.len() > label.len() && trimmed.starts_with(**label))
        {
            push(
                start + sense.len() - trimmed.len(),
                label.len() - 1,
                TranslationRunRole::Pos,
            );
        }
        if separator_width != 0 {
            push(end, separator_width, TranslationRunRole::Separator);
        }
        start = end + separator_width;
    }
    runs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upstream_roles_preserve_unicode_and_whitespace_offsets() {
        for (gloss, expected) in [
            (
                "adj. spreading; n. sound",
                vec![(0, 4, "adj."), (16, 2, "n.")],
            ),
            ("proper n. Synthetic", vec![(0, 9, "proper n.")]),
            (
                " \tv. 🦀 ；；idiom. synthetic; ",
                vec![(2, 2, "v."), (16, 6, "idiom.")],
            ),
            ("苹果；家伙", vec![]),
            ("very peculiar", vec![]),
            ("n.", vec![]),
            ("n. ", vec![]),
            ("N. synthetic", vec![]),
            ("", vec![]),
        ] {
            let runs = translation_runs(gloss);
            let labels: Vec<_> = runs
                .iter()
                .filter(|run| run.role == TranslationRunRole::Pos)
                .map(|run| {
                    (
                        run.start,
                        run.length,
                        &gloss[run.start..run.start + run.length],
                    )
                })
                .collect();
            assert_eq!(labels, expected, "{gloss:?}");
            let separators: String = runs
                .iter()
                .filter(|run| run.role == TranslationRunRole::Separator)
                .map(|run| &gloss[run.start..run.start + run.length])
                .collect();
            assert_eq!(
                separators,
                gloss
                    .chars()
                    .filter(|ch| matches!(ch, ';' | '；'))
                    .collect::<String>()
            );
            assert!(runs
                .windows(2)
                .all(|pair| pair[0].start + pair[0].length <= pair[1].start));
        }
    }

    #[test]
    fn all_source_labels_require_following_text() {
        for label in [
            "proper n.",
            "interj.",
            "idiom.",
            "pron.",
            "prep.",
            "conj.",
            "adj.",
            "adv.",
            "num.",
            "det.",
            "aux.",
            "n.",
            "v.",
        ] {
            let gloss = format!("\n {label} synthetic");
            assert_eq!(
                translation_runs(&gloss),
                vec![TranslationRun {
                    start: 2,
                    length: label.len(),
                    role: TranslationRunRole::Pos
                }]
            );
            assert!(translation_runs(label).is_empty());
        }
        assert_eq!(
            translation_runs("；;；"),
            vec![TranslationRun {
                start: 0,
                length: 7,
                role: TranslationRunRole::Separator
            }]
        );
    }
}
