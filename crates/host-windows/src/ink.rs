//! Windows Ink handwriting recognition.
//!
//! Windows ships a handwriting recognizer for every installed language pack,
//! and the reference panel uses it directly. Without this, Windows had no
//! recognizer at all unless the optional packaged Engine model happened to be
//! installed, so the handwriting panel answered "unavailable" on a stock
//! machine — the one platform where a recognizer is already present.
//!
//! Nothing here reads user text beyond the strokes the panel passes in, and no
//! stroke leaves the machine: `InkRecognizerContainer` resolves locally.

use windows::Foundation::Point;
use windows::UI::Input::Inking::{
    InkPoint, InkRecognitionTarget, InkRecognizer, InkRecognizerContainer, InkStrokeBuilder,
    InkStrokeContainer,
};

/// One handwritten stroke: the points the pointer passed through, in order.
pub type Stroke = Vec<(f32, f32)>;

/// Why recognition could not answer. The panel distinguishes these because
/// they need different things from the user.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InkError {
    /// No Chinese recognizer is installed (Simplified is preferred, Traditional
    /// is the fallback). The user has to add the handwriting feature for the
    /// language; nothing we do will substitute.
    NoChineseRecognizer,
    /// Nothing to recognize.
    EmptyInput,
    /// The Ink API itself failed.
    Unavailable,
}

/// Longest run of strokes we will hand to the recognizer, matching the panel's
/// own limit. A caller that ignored its limit must not be able to make us
/// build an unbounded stroke container.
const MAX_STROKES: usize = 128;
/// Longest single stroke. A pointer stream stuck in a loop is bounded here.
const MAX_POINTS: usize = 4096;

/// How well the recognizer's name matches Simplified Chinese: 3 Simplified,
/// 2 Chinese of unstated script, 1 Traditional, 0 not Chinese.
///
/// Matching on the name is what the reference does. The recognizer list is
/// localised, so the display name arrives in the user's own UI language and no
/// single spelling covers it — hence several, including the English one.
/// Traditional still ranks above nothing: most characters are shared, and a
/// machine with only the zh-TW/zh-HK pack should keep a working panel.
fn chinese_rank(name: &str) -> u8 {
    let lowered = name.to_lowercase();
    let has = |needles: &[&str]| needles.iter().any(|needle| lowered.contains(needle));
    // Gate on Chinese first, so "Traditional Mongolian" never qualifies.
    if !has(&["中文", "chinese", "简体", "簡體", "繁體", "繁体", "zh-"]) {
        return 0;
    }
    // The script, when named, decides; a zh-TW UI spells Simplified as 簡體.
    if has(&["简体", "簡體", "simplified", "zh-hans"]) {
        return 3;
    }
    // Region-only names such as "中文(台灣)" carry no script word, and are
    // checked before the Simplified regions so "中国台湾" is not read as China.
    if has(&[
        "繁",
        "traditional",
        "zh-hant",
        "台灣",
        "臺灣",
        "台湾",
        "香港",
        "澳門",
        "澳门",
        "taiwan",
        "hong kong",
        "macao",
        "macau",
        "zh-tw",
        "zh-hk",
        "zh-mo",
    ]) {
        return 1;
    }
    if has(&[
        "中国",
        "中國",
        "china",
        "prc",
        "新加坡",
        "singapore",
        "zh-cn",
        "zh-sg",
    ]) {
        return 3;
    }
    2
}

/// True when the text contains a CJK ideograph.
fn contains_cjk(text: &str) -> bool {
    text.chars()
        .any(|ch| matches!(ch as u32, 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF))
}

/// Recognize handwritten strokes, most likely candidate first.
///
/// Chinese candidates are ordered ahead of the rest: the recognizer returns
/// Latin readings of the same strokes too, and a user writing Chinese should
/// not have to scroll past them.
pub fn recognize(strokes: &[Stroke]) -> Result<Vec<String>, InkError> {
    if strokes.is_empty() || strokes.iter().all(|stroke| stroke.len() < 2) {
        return Err(InkError::EmptyInput);
    }
    recognize_inner(strokes).unwrap_or_else(Err)
}

/// 本机装着的最合适的中文识别器（简体优先，其次繁体），没有时为 `None`。
fn best_chinese_recognizer(
    container: &InkRecognizerContainer,
) -> Result<Option<InkRecognizer>, InkError> {
    let recognizers = container
        .GetRecognizers()
        .map_err(|_| InkError::Unavailable)?;
    // The list order is arbitrary, so the first Chinese match could be the
    // Traditional pack; keep the best and stop early only on Simplified.
    let mut best = None;
    let mut best_rank = 0;
    for recognizer in recognizers {
        let name = recognizer
            .Name()
            .map(|value| value.to_string_lossy())
            .unwrap_or_default();
        let rank = chinese_rank(&name);
        if rank > best_rank {
            best_rank = rank;
            best = Some(recognizer);
            if rank == 3 {
                break;
            }
        }
    }
    Ok(best)
}

/// Windows 是否装有中文手写识别器（简体或繁体）。设置应用据此决定要不要提供手写模型的下载：有识别器时手写面板先用它，不需要模型。Ink API 本身出错时按没有处理，让用户仍能下载模型兜底。
pub fn has_chinese_recognizer() -> bool {
    InkRecognizerContainer::new()
        .map_err(|_| InkError::Unavailable)
        .and_then(|container| best_chinese_recognizer(&container))
        .is_ok_and(|recognizer| recognizer.is_some())
}

fn recognize_inner(strokes: &[Stroke]) -> Result<Result<Vec<String>, InkError>, InkError> {
    let container = InkRecognizerContainer::new().map_err(|_| InkError::Unavailable)?;
    let Some(recognizer) = best_chinese_recognizer(&container)? else {
        // Not a failure we can retry around: the user has to install the
        // handwriting feature for Chinese.
        return Ok(Err(InkError::NoChineseRecognizer));
    };
    container
        .SetDefaultRecognizer(&recognizer)
        .map_err(|_| InkError::Unavailable)?;

    let strokes_container = InkStrokeContainer::new().map_err(|_| InkError::Unavailable)?;
    let builder = InkStrokeBuilder::new().map_err(|_| InkError::Unavailable)?;
    // The identity transform: the panel already sends canvas-relative points.
    let identity = windows_numerics::Matrix3x2 {
        M11: 1.0,
        M12: 0.0,
        M21: 0.0,
        M22: 1.0,
        M31: 0.0,
        M32: 0.0,
    };
    for stroke in strokes.iter().take(MAX_STROKES) {
        // A single point is a dot, not a stroke; the builder rejects it and
        // would fail the whole batch over one stray tap.
        if stroke.len() < 2 {
            continue;
        }
        // The projection's iterable takes the type's default representation,
        // which for a WinRT class is Option<T>.
        let points: Vec<Option<InkPoint>> = stroke
            .iter()
            .take(MAX_POINTS)
            .filter(|(x, y)| x.is_finite() && y.is_finite())
            .map(|&(x, y)| InkPoint::CreateInkPoint(Point { X: x, Y: y }, 0.5).map(Some))
            .collect::<Result<_, _>>()
            .map_err(|_| InkError::Unavailable)?;
        if points.len() < 2 {
            continue;
        }
        let built = builder
            .CreateStrokeFromInkPoints(
                &windows_collections::IIterable::<InkPoint>::from(points),
                identity,
            )
            .map_err(|_| InkError::Unavailable)?;
        strokes_container
            .AddStroke(&built)
            .map_err(|_| InkError::Unavailable)?;
    }

    // windows-future named the blocking wait `get` through 0.2 and renamed it to `join` in 0.3, which is what windows 0.62 projects these operations through.
    let results = container
        .RecognizeAsync(&strokes_container, InkRecognitionTarget::All)
        .map_err(|_| InkError::Unavailable)?
        .join()
        .map_err(|_| InkError::Unavailable)?;

    let mut chinese: Vec<String> = Vec::with_capacity(12);
    let mut other: Vec<String> = Vec::with_capacity(12);
    for result in results {
        let Ok(candidates) = result.GetTextCandidates() else {
            continue;
        };
        for candidate in candidates {
            let text = candidate.to_string_lossy();
            if text.is_empty() {
                continue;
            }
            let bucket = if contains_cjk(&text) {
                &mut chinese
            } else {
                &mut other
            };
            // The recognizer repeats candidates across results; a duplicate
            // costs the user a candidate slot for nothing.
            if !bucket.contains(&text) {
                bucket.push(text);
            }
        }
    }
    chinese.retain(|text| !text.is_empty());
    for text in other {
        if !chinese.contains(&text) {
            chinese.push(text);
        }
    }
    Ok(Ok(chinese))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Name matching is the part that decides whether Windows recognises
    // Chinese at all, and the list arrives localised.
    fn looks_like_chinese(name: &str) -> bool {
        chinese_rank(name) > 0
    }

    #[test]
    fn recognizer_names_are_matched_in_any_ui_language() {
        assert!(looks_like_chinese("中文(简体，中国)"));
        assert!(looks_like_chinese("简体中文"));
        assert!(looks_like_chinese("Chinese (Simplified, China)"));
        assert!(looks_like_chinese("chinese"));
        assert!(looks_like_chinese("zh-CN"));
        // Case is folded, so an unexpected capitalisation still matches.
        assert!(looks_like_chinese("ZH-CN"));
        assert!(looks_like_chinese("CHINESE (SIMPLIFIED)"));
    }

    // Picking the wrong recognizer is worse than picking none: it would
    // silently return Latin readings of Chinese strokes.
    #[test]
    fn other_languages_are_not_mistaken_for_chinese() {
        assert!(!looks_like_chinese("English (United States)"));
        assert!(!looks_like_chinese("日本語"));
        assert!(!looks_like_chinese("한국어"));
        assert!(!looks_like_chinese("Deutsch"));
        assert!(!looks_like_chinese(""));
        assert!(!looks_like_chinese("Mongolian (Traditional Mongolian)"));
    }

    // With both packs installed the list order is arbitrary; Simplified has to
    // win, and Traditional must still beat having no recognizer at all.
    #[test]
    fn simplified_recognizers_rank_above_traditional() {
        let simplified = [
            "Microsoft 中文(简体)手写识别器",
            "Microsoft 中文(簡體)手寫辨識器",
            "Chinese (Simplified, China)",
            "中文(中国)",
            "zh-Hans",
        ];
        let traditional = [
            "Microsoft 中文(繁體)手寫辨識器",
            "Chinese (Traditional)",
            "Chinese (Traditional, Taiwan)",
            "中文(香港特別行政區)",
            "中文(台灣)",
            "Chinese (Taiwan)",
        ];
        for name in simplified {
            assert_eq!(chinese_rank(name), 3, "{name}");
        }
        for name in traditional {
            assert_eq!(chinese_rank(name), 1, "{name}");
        }
        assert_eq!(chinese_rank("Microsoft Chinese Handwriting Recognizer"), 2);
    }

    #[test]
    fn cjk_detection_sorts_candidates() {
        assert!(contains_cjk("你好"));
        assert!(contains_cjk("a你"));
        // Extension A and compatibility ideographs count too.
        assert!(contains_cjk("\u{3400}"));
        assert!(contains_cjk("\u{F900}"));
        assert!(!contains_cjk("hello"));
        assert!(!contains_cjk(""));
        // Punctuation and kana are not ideographs.
        assert!(!contains_cjk("，。"));
        assert!(!contains_cjk("ひらがな"));
    }

    // Empty input must be reported as such rather than reaching the Ink API,
    // which would report a generic failure the panel cannot explain.
    #[test]
    fn empty_input_is_rejected_before_calling_windows() {
        assert_eq!(recognize(&[]), Err(InkError::EmptyInput));
        // A single tap is a dot, not a stroke.
        assert_eq!(recognize(&[vec![(1.0, 1.0)]]), Err(InkError::EmptyInput));
        assert_eq!(
            recognize(&[vec![(1.0, 1.0)], vec![(2.0, 2.0)]]),
            Err(InkError::EmptyInput)
        );
    }
}
