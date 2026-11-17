//! 混输整句只接受完整音节和严格声母，保留至少一个完整音节作为锚点。

use crate::pinyin::jianpin::extract_initial_token;
use crate::pinyin::syllables::{is_intact, is_prefix};

pub(super) fn has_mixed_initials(segments: &[String]) -> bool {
    segments.len() >= 2
        && segments.iter().any(|part| is_intact(part))
        && segments.iter().any(|part| !is_intact(part))
        && segments
            .iter()
            .all(|part| is_intact(part) || (is_prefix(part) && extract_initial_token(part) == part))
}
