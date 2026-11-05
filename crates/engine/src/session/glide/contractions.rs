//! 常见英文缩略的字母手势与显示词形；仅供混合滑行，不改普通英文词库。

use super::*;

/// 手写的常见形式，频度从基础词取得；没有该英文基础词的词库不增加额外候选。
const FORMS: &[(&str, &str, &str)] = &[
    ("dont", "don't", "do"),
    ("doesnt", "doesn't", "does"),
    ("didnt", "didn't", "did"),
    ("cant", "can't", "can"),
    ("couldnt", "couldn't", "could"),
    ("wont", "won't", "will"),
    ("wouldnt", "wouldn't", "would"),
    ("shouldnt", "shouldn't", "should"),
    ("isnt", "isn't", "is"),
    ("arent", "aren't", "are"),
    ("wasnt", "wasn't", "was"),
    ("werent", "weren't", "were"),
    ("havent", "haven't", "have"),
    ("hasnt", "hasn't", "has"),
    ("hadnt", "hadn't", "had"),
    ("mustnt", "mustn't", "must"),
    // 随包词库没有单字母 I，用同人称 me 的频度作有折扣的代用先验。
    ("im", "I'm", "me"),
    ("ill", "I'll", "me"),
    ("ive", "I've", "me"),
    ("id", "I'd", "me"),
    ("youre", "you're", "you"),
    ("youll", "you'll", "you"),
    ("youve", "you've", "you"),
    ("youd", "you'd", "you"),
    ("were", "we're", "we"),
    ("well", "we'll", "we"),
    ("weve", "we've", "we"),
    ("wed", "we'd", "we"),
    ("theyre", "they're", "they"),
    ("theyll", "they'll", "they"),
    ("theyve", "they've", "they"),
    ("theyd", "they'd", "they"),
    ("hes", "he's", "he"),
    ("hell", "he'll", "he"),
    ("hed", "he'd", "he"),
    ("shes", "she's", "she"),
    ("shell", "she'll", "she"),
    ("shed", "she'd", "she"),
    ("its", "it's", "it"),
    ("itll", "it'll", "it"),
    ("thats", "that's", "that"),
    ("theres", "there's", "there"),
    ("whats", "what's", "what"),
    ("whos", "who's", "who"),
    ("wheres", "where's", "where"),
    ("hows", "how's", "how"),
    ("lets", "let's", "let"),
];

pub(super) fn append(
    matcher: &GlideMatcher,
    mut base_weight: impl FnMut(&str) -> Option<i64>,
    rows: &mut Vec<(String, WordItem, f32)>,
) {
    for &(letters, display, base) in FORMS {
        let Some(cost) = matcher.score(letters) else {
            continue;
        };
        let Some(weight) = base_weight(base) else {
            continue;
        };
        // 基础词只提供打折先验，不能把它本身的全部频度直接赋给缩略。
        let row = WordItem::new(
            letters,
            display,
            weight.max(16) / 16,
            CandidateSource::Generated,
            "",
        );
        rows.push((letters.to_owned(), row, cost));
    }
}
