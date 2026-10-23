use crate::dictionary::DictRow;
use crate::types::{CandidateSource, SchemeType, WordItem};

/// 普通页只保存变化的字段；展开页继续持有原完整候选和容量。
#[derive(Clone, Debug, PartialEq)]
pub(super) enum PathPage {
    Full(Vec<WordItem>),
    Compact(Box<CompactPage>),
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct CompactPage {
    pinyin: Option<String>,
    rows: Vec<DictRow>,
}

impl From<Vec<WordItem>> for PathPage {
    fn from(rows: Vec<WordItem>) -> Self {
        Self::Full(rows)
    }
}

impl PathPage {
    pub(super) fn from_dictionary(cache_key: &str, rows: &[WordItem]) -> Self {
        let Some(first) = rows.first() else {
            return Self::Full(Vec::new());
        };
        let separate_reading = first.pinyin != cache_key;
        if rows.len() <= 1 + usize::from(separate_reading)
            || !rows.iter().all(|row| ordinary_row(row, &first.pinyin))
        {
            return Self::Full(rows.to_vec());
        }
        let mut compact = Vec::with_capacity(rows.len());
        compact.extend(rows.iter().map(|row| DictRow {
            key: row.canonical_pinyin.clone(),
            value: row.word.clone(),
            weight: row.weight,
        }));
        Self::Compact(Box::new(CompactPage {
            pinyin: separate_reading.then(|| first.pinyin.clone()),
            rows: compact,
        }))
    }

    pub(super) fn len(&self) -> usize {
        match self {
            Self::Full(rows) => rows.len(),
            Self::Compact(page) => page.rows.len(),
        }
    }

    pub(super) fn copy_rows(&self, cache_key: &str) -> Vec<WordItem> {
        match self {
            Self::Full(rows) => rows.clone(),
            Self::Compact(page) => {
                let pinyin = page.pinyin.as_deref().unwrap_or(cache_key);
                let mut result = Vec::with_capacity(page.rows.len());
                result.extend(page.rows.iter().map(|row| copy_row(row, pinyin)));
                result
            }
        }
    }

    pub(super) fn append_to(&self, cache_key: &str, result: &mut Vec<WordItem>) {
        result.reserve(self.len());
        match self {
            Self::Full(rows) => result.extend(rows.iter().cloned()),
            Self::Compact(page) => {
                let pinyin = page.pinyin.as_deref().unwrap_or(cache_key);
                result.extend(page.rows.iter().map(|row| copy_row(row, pinyin)));
            }
        }
    }

    pub(super) fn cloned_rows<'a>(&'a self, cache_key: &'a str) -> ClonedRows<'a> {
        match self {
            Self::Full(rows) => ClonedRows::Full(rows.iter()),
            Self::Compact(page) => ClonedRows::Compact {
                pinyin: page.pinyin.as_deref().unwrap_or(cache_key),
                rows: page.rows.iter(),
            },
        }
    }
}

fn ordinary_row(row: &WordItem, reading: &str) -> bool {
    // 穷尽字段，使新增元数据必须明确选择保存或拒绝压缩。
    let WordItem {
        pinyin,
        canonical_pinyin: _,
        word: _,
        weight: _,
        source,
        scheme,
        fixed_position,
        fuzzy,
        corrected_from,
        sentence_association,
        sentence_words,
    } = row;
    pinyin == reading
        && *source == CandidateSource::Database
        && *scheme == SchemeType::Quanpin
        && *fixed_position == 0
        && !fuzzy
        && corrected_from.is_empty()
        && !sentence_association
        && sentence_words.is_empty()
}

fn copy_row(row: &DictRow, pinyin: &str) -> WordItem {
    WordItem::new(
        pinyin,
        row.value.clone(),
        row.weight,
        CandidateSource::Database,
        row.key.clone(),
    )
}

pub(super) enum ClonedRows<'a> {
    Full(std::slice::Iter<'a, WordItem>),
    Compact {
        pinyin: &'a str,
        rows: std::slice::Iter<'a, DictRow>,
    },
}

impl Iterator for ClonedRows<'_> {
    type Item = WordItem;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Full(rows) => rows.next().cloned(),
            Self::Compact { pinyin, rows } => rows.next().map(|row| copy_row(row, pinyin)),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = match self {
            Self::Full(rows) => rows.len(),
            Self::Compact { rows, .. } => rows.len(),
        };
        (len, Some(len))
    }
}

impl ExactSizeIterator for ClonedRows<'_> {}
