use crate::types::WordItem;

/// 整词按用户启用的模糊读法覆盖全部输入时，不能再被多出音节的补全词压住。
pub(super) fn lead_whole_fuzzy_before_completion(rows: &mut [WordItem], raw: &str) {
    let letters = matched_letters(raw);
    if letters == 0 {
        return;
    }
    let Some(start) = rows
        .iter()
        .position(|row| matched_letters(&row.pinyin) == letters)
    else {
        return;
    };
    let count = rows[start..]
        .iter()
        .take_while(|row| matched_letters(&row.pinyin) == letters)
        .count();
    let bucket = &mut rows[start..start + count];
    let Some(completion) = bucket.iter().position(is_longer_completion) else {
        return;
    };
    let tail = &mut bucket[completion..];
    if tail.iter().any(is_whole_fuzzy) {
        // 只提升模糊整词；精确词、整句及补全词之间的原先后关系保持。
        tail.sort_by_key(|row| !is_whole_fuzzy(row));
    }
}

fn matched_letters(pinyin: &str) -> usize {
    pinyin.bytes().filter(|&byte| byte != b'\'').count()
}

fn is_longer_completion(row: &WordItem) -> bool {
    !row.fuzzy
        && row.source.is_dictionary()
        && !row.sentence_association
        && row
            .canonical_pinyin
            .strip_prefix(row.pinyin.as_str())
            .is_some_and(|suffix| suffix.starts_with('\''))
}

fn is_whole_fuzzy(row: &WordItem) -> bool {
    row.fuzzy
        && row.source.is_dictionary()
        && !row.pinyin.is_empty()
        && !row.canonical_pinyin.is_empty()
        && row.pinyin.bytes().filter(|&byte| byte == b'\'').count()
            == row
                .canonical_pinyin
                .bytes()
                .filter(|&byte| byte == b'\'')
                .count()
}

#[cfg(test)]
mod tests {
    use crate::quanpin::fixture::Fixture;
    use crate::quanpin::QuanpinDictionary;
    use crate::types::{fuzzy_rule, FuzzyPinyinOptions};

    #[test]
    fn whole_fuzzy_words_precede_heavier_completions_on_cold_and_warm_queries() {
        let fixture = Fixture::new();
        fixture
            .insert("chi'ku", "吃苦", 100)
            .insert("chi'ku", "吃库", 1)
            .insert("chi'ku'nai'lao", "吃苦耐劳", 10000)
            .insert("chi'ku'tou", "吃苦头", 9000)
            .insert("ci'ku", "词库", 300)
            .insert("ci'ku", "辞库", 200)
            .insert("ci", "词", 100000);
        let mut dictionary = QuanpinDictionary::new(&fixture.paths);
        for _ in 0..2 {
            let rows = dictionary.query(
                "chiku",
                "chi'ku",
                0,
                FuzzyPinyinOptions {
                    rules: fuzzy_rule::C_CH,
                },
            );
            let words: Vec<_> = rows.iter().map(|row| row.word.as_str()).collect();
            assert_eq!(&words[..3], &["吃苦", "词库", "辞库"], "{words:?}");
            assert!(
                words.iter().position(|word| *word == "吃苦耐劳").unwrap()
                    < words.iter().position(|word| *word == "词").unwrap()
            );
            let row = rows.iter().find(|row| row.word == "词库").unwrap();
            assert!(row.fuzzy);
            assert_eq!(row.pinyin, "chi'ku");
            assert_eq!(row.canonical_pinyin, "ci'ku");
        }
        let plain = dictionary.query("chiku", "chi'ku", 0, FuzzyPinyinOptions { rules: 0 });
        let words: Vec<_> = plain.iter().map(|row| row.word.as_str()).collect();
        assert_eq!(&words[..3], &["吃苦", "吃苦耐劳", "吃苦头"], "{words:?}");
        assert!(!words.contains(&"词库"));
    }

    #[test]
    fn adding_a_letter_in_the_fuzzy_reading_also_leads_the_completion() {
        let fixture = Fixture::new();
        fixture
            .insert("ci'ku", "词库", 100)
            .insert("ci'ku'guan'li", "词库管理", 10000)
            .insert("chi'ku", "吃苦", 10);
        let mut dictionary = QuanpinDictionary::new(&fixture.paths);
        let rows = dictionary.query(
            "ciku",
            "ci'ku",
            0,
            FuzzyPinyinOptions {
                rules: fuzzy_rule::C_CH,
            },
        );
        let words: Vec<_> = rows.iter().map(|row| row.word.as_str()).collect();
        assert_eq!(&words[..3], &["词库", "吃苦", "词库管理"], "{words:?}");
    }
}

#[cfg(test)]
mod boundary_tests {
    use super::*;
    use crate::types::CandidateSource;

    fn row(pinyin: &str, word: &str, canonical: &str, fuzzy: bool) -> WordItem {
        let mut row = WordItem::new(pinyin, word, 100, CandidateSource::Database, canonical);
        row.fuzzy = fuzzy;
        row
    }

    fn words(rows: &[WordItem]) -> Vec<&str> {
        rows.iter().map(|row| row.word.as_str()).collect()
    }

    #[test]
    fn incomplete_fuzzy_prefix_stays_behind_whole_input_and_completions() {
        let mut rows = vec![
            row("chi'ku", "吃苦", "chi'ku", false),
            row("chi'ku", "吃苦头", "chi'ku'tou", false),
            row("chi", "词", "ci", true),
        ];
        lead_whole_fuzzy_before_completion(&mut rows, "chiku");
        assert_eq!(words(&rows), ["吃苦", "吃苦头", "词"]);
    }

    #[test]
    fn exact_rows_and_sentences_keep_their_mutual_order() {
        let mut sentence = row("chi'ku", "尺库", "chi'ku", false);
        sentence.source = CandidateSource::Generated;
        sentence.sentence_association = true;
        let mut rows = vec![
            row("chi'ku", "吃苦", "chi'ku", false),
            sentence,
            row("chi'ku", "吃苦头", "chi'ku'tou", false),
            row("chi'ku", "吃库", "chi'ku", false),
            row("chi'ku", "词库", "ci'ku", true),
            row("chi'ku", "辞库", "ci'ku", true),
            row("chi", "词", "ci", true),
        ];
        lead_whole_fuzzy_before_completion(&mut rows, "chi'ku");
        assert_eq!(
            words(&rows),
            ["吃苦", "尺库", "词库", "辞库", "吃苦头", "吃库", "词"]
        );
        lead_whole_fuzzy_before_completion(&mut rows, "chi'ku");
        assert_eq!(
            words(&rows),
            ["吃苦", "尺库", "词库", "辞库", "吃苦头", "吃库", "词"]
        );
    }

    #[test]
    fn missing_completions_and_other_coverage_groups_do_not_change() {
        let mut rows = vec![
            row("chi'ku'le", "吃苦了", "chi'ku'le", false),
            row("chi'ku", "吃苦", "chi'ku", false),
            row("chi'ku", "词库", "ci'ku", true),
            row("chi", "吃饭", "chi'fan", false),
            row("chi", "词", "ci", true),
        ];
        let before = words(&rows)
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        lead_whole_fuzzy_before_completion(&mut rows, "chiku");
        assert_eq!(words(&rows), before);
        lead_whole_fuzzy_before_completion(&mut rows, "");
        assert_eq!(words(&rows), before);
        lead_whole_fuzzy_before_completion(&mut [], "chiku");
    }
}
