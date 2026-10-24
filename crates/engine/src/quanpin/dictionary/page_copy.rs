use crate::shared_string::SharedString;
use crate::types::WordItem;

/// 只复用普通复制也会持有的容量，避免淘汰页的大缓冲区留在新槽中。
pub(super) fn clone_rows_reusing(rows: &[WordItem], previous: Vec<WordItem>) -> Vec<WordItem> {
    clone_vec_reusing(rows, previous, clone_row_reusing)
}

fn clone_vec_reusing<T: Clone + Default>(
    source: &[T],
    mut previous: Vec<T>,
    copy: fn(&T, T) -> T,
) -> Vec<T> {
    if previous.capacity() != source.len() {
        let mut old = previous.into_iter();
        let mut result = Vec::with_capacity(source.len());
        result.extend(source.iter().map(|item| match old.next() {
            Some(previous) => copy(item, previous),
            None => item.clone(),
        }));
        return result;
    }
    previous.truncate(source.len());
    let overlap = previous.len();
    for (item, source) in previous.iter_mut().zip(source) {
        *item = copy(source, std::mem::take(item));
    }
    previous.extend_from_slice(&source[overlap..]);
    previous
}

fn clone_shared_string_reusing(source: &str, mut previous: SharedString) -> SharedString {
    if previous.capacity() == source.len() {
        previous.clear();
        previous.push_str(source);
        previous
    } else {
        source.into()
    }
}

fn clone_string_reusing(source: &str, mut previous: String) -> String {
    if previous.capacity() == source.len() {
        previous.clear();
        previous.push_str(source);
        previous
    } else {
        source.to_owned()
    }
}

fn clone_row_reusing(source: &WordItem, previous: WordItem) -> WordItem {
    // 穷尽字段，让候选类型增加字段时在此处产生编译错误。
    let WordItem {
        pinyin,
        canonical_pinyin,
        word,
        weight: _,
        source: _,
        scheme: _,
        fixed_position: _,
        fuzzy: _,
        corrected_from,
        sentence_association: _,
        sentence_words,
    } = previous;
    WordItem {
        pinyin: clone_shared_string_reusing(&source.pinyin, pinyin),
        canonical_pinyin: clone_shared_string_reusing(&source.canonical_pinyin, canonical_pinyin),
        word: clone_shared_string_reusing(&source.word, word),
        weight: source.weight,
        source: source.source,
        scheme: source.scheme,
        fixed_position: source.fixed_position,
        fuzzy: source.fuzzy,
        corrected_from: clone_shared_string_reusing(&source.corrected_from, corrected_from),
        sentence_association: source.sentence_association,
        sentence_words: clone_vec_reusing(
            &source.sentence_words,
            sentence_words,
            |source, previous| clone_string_reusing(source, previous),
        ),
    }
}
