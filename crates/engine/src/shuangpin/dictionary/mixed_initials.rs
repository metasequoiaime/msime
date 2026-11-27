//! 混输整句只接受完整音节和严格声母；纯声母串（每段都是严格声母、没有任何完整音节）同样
//! 走混合跨度查询，这样 7 个声母能拼出整句，而不是退回到前缀组的三字词（#11766）。

use crate::pinyin::jianpin::extract_initial_token;
use crate::pinyin::syllables::{is_intact, is_prefix};

pub(super) fn has_mixed_initials(segments: &[String]) -> bool {
    segments.len() >= 2
        && segments.iter().any(|part| !is_intact(part))
        && segments
            .iter()
            .all(|part| is_intact(part) || (is_prefix(part) && extract_initial_token(part) == part))
}
