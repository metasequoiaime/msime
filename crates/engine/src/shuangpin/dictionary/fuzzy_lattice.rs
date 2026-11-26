use crate::dictionary::{pinyin::PinyinDatabase, DictRow};
use crate::pinyin::fuzzy::{fuzzy_segmentations, FUZZY_SEGMENTATION_LIMIT};
use crate::pinyin::syllables::canonical_lattice_syllable;
use crate::types::FuzzyPinyinOptions;

// 每个跨度最多查八种读音，字面占第一种；整句话不需要枚举各音节的笛卡尔积。
const SPAN_READING_LIMIT: usize = 8;

pub(super) fn has_whole_word(
    database: &PinyinDatabase,
    segments: &[String],
    options: FuzzyPinyinOptions,
) -> bool {
    fuzzy_segmentations(segments, options, FUZZY_SEGMENTATION_LIMIT)
        .iter()
        .any(|reading| !database.query_lattice_span(reading, 1).is_empty())
}

pub(super) fn lookup(
    database: &PinyinDatabase,
    span: &[String],
    row_limit: usize,
    options: FuzzyPinyinOptions,
) -> Vec<DictRow> {
    let mut rows = database.query_lattice_span(span, row_limit);
    let variants = fuzzy_segmentations(span, options, SPAN_READING_LIMIT);
    if variants.is_empty() {
        return rows;
    }
    for variant in variants {
        let changed = variant
            .iter()
            .zip(span)
            .filter(|(candidate, typed)| !same_syllable(candidate, typed))
            .count();
        if changed == 0 {
            continue;
        }
        rows.extend(
            database
                .query_lattice_span(&variant, row_limit)
                .into_iter()
                .map(|mut row| {
                    // 每个模糊音节给词频增加一份代价；同频时优先用户直接键入的读法。
                    for _ in 0..changed {
                        let weight = row.weight.max(0);
                        row.weight -= weight / 10 + i64::from(weight % 10 != 0);
                    }
                    row
                }),
        );
    }
    rows.sort_by_key(|row| std::cmp::Reverse(row.weight));
    rows.truncate(row_limit);
    rows
}

pub(super) fn follows_typed_reading(key: &str, segments: &[String]) -> bool {
    key.split('\'').map(canonical_lattice_syllable).eq(segments
        .iter()
        .map(|segment| canonical_lattice_syllable(segment)))
}

fn same_syllable(left: &str, right: &str) -> bool {
    canonical_lattice_syllable(left) == canonical_lattice_syllable(right)
}
