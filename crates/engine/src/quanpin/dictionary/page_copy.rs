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

fn clone_string_reusing(source: &str, mut previous: String) -> String {
    if previous.capacity() == source.len() {
        previous.clear();
        previous.push_str(source);
        previous
    } else {
        source.to_owned()
    }
}

/// 文本字段是 `SharedString`，克隆只加引用计数：缓存页与返回页共用同一份字符串，任何一边原地改写时按写时复制先复制，两页互不影响。逐字节覆写旧页的缓冲区反而要为容量对不上的字段（几乎全部）各申请一份。`sentence_words` 仍是普通 `String`，照旧复用淘汰页里容量恰好的缓冲区。
fn clone_row_reusing(source: &WordItem, previous: WordItem) -> WordItem {
    // 穷尽字段，让候选类型增加字段时在此处产生编译错误。
    let WordItem {
        pinyin: _,
        canonical_pinyin: _,
        word: _,
        weight: _,
        source: _,
        scheme: _,
        fixed_position: _,
        fuzzy: _,
        corrected_from: _,
        sentence_association: _,
        sentence_words,
        touch_supported: _,
        confident_correction: _,
    } = previous;
    WordItem {
        pinyin: source.pinyin.clone(),
        canonical_pinyin: source.canonical_pinyin.clone(),
        word: source.word.clone(),
        weight: source.weight,
        source: source.source,
        scheme: source.scheme,
        fixed_position: source.fixed_position,
        fuzzy: source.fuzzy,
        corrected_from: source.corrected_from.clone(),
        sentence_association: source.sentence_association,
        sentence_words: clone_vec_reusing(
            &source.sentence_words,
            sentence_words,
            |source, previous| clone_string_reusing(source, previous),
        ),
        touch_supported: source.touch_supported,
        confident_correction: source.confident_correction,
    }
}
