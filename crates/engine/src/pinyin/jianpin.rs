//! 词典级联查询的简拼谓词（quanpin.md §5.1）。全拼与双拼都接受双字母声母，双拼仍严格区分平翘舌的单字母。

use super::syllables::is_intact;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuerySource {
    Quanpin,
    Shuangpin,
}

fn is_initial_token(segment: &str) -> bool {
    matches!(segment, "zh" | "ch" | "sh")
}

/// Every segment has length 1.
pub fn is_pure_jianpin(segments: &[String]) -> bool {
    segments.iter().all(|segment| segment.len() == 1)
}

/// 至少两个音节，且非末尾音节含单字母或 `zh/ch/sh` 声母。
pub fn needs_mixed_jianpin_query(segments: &[String], _source: QuerySource) -> bool {
    let Some((_, leading)) = segments.split_last() else {
        return false;
    };
    !leading.is_empty()
        && leading
            .iter()
            .any(|segment| segment.len() == 1 || is_initial_token(segment))
}

/// `zh`/`ch`/`sh` when present, else the first letter (QQ:308-319).
pub fn extract_initial_token(syllable: &str) -> &str {
    if let Some(prefix) = syllable.get(..2) {
        if is_initial_token(prefix) {
            return prefix;
        }
    }
    syllable.get(..1).unwrap_or("")
}

/// Whether a dictionary key matches abbreviated segments position by position (QQ:321-364).
pub fn matches_mixed_segments(key: &str, segments: &[String], source: QuerySource) -> bool {
    let mut actual_segments = key.split('\'');
    segments.iter().all(|expected| {
        let Some(actual) = actual_segments.next() else {
            return false;
        };
        if expected.is_empty() || actual.is_empty() {
            return false;
        }
        if is_initial_token(expected) {
            extract_initial_token(actual) == expected
        } else if expected.len() == 1 {
            match source {
                QuerySource::Shuangpin => extract_initial_token(actual) == expected,
                QuerySource::Quanpin => actual.as_bytes()[0] == expected.as_bytes()[0],
            }
        } else {
            actual == expected
        }
    }) && actual_segments.next().is_none()
}

/// 为全拼补充双字母声母拆成两个单字母的路径；不改完整音节，最多返回 `limit` 条。
pub fn split_initial_token_paths(segments: &[String], limit: usize) -> Vec<Vec<String>> {
    if limit == 0 || !segments.iter().any(|segment| is_initial_token(segment)) {
        return Vec::new();
    }
    let mut fully_split = Vec::with_capacity(segments.len() * 2);
    for segment in segments {
        if is_initial_token(segment) {
            fully_split.extend(segment.bytes().map(|byte| char::from(byte).to_string()));
        } else {
            fully_split.push(segment.clone());
        }
    }
    // 保留全拆路径，避免多个声母的组合达到上限时把长词的纯简拼截掉。
    let mut result = vec![fully_split];
    if limit == 1 {
        return result;
    }
    let mut paths = vec![Vec::new()];
    for segment in segments {
        let mut next = Vec::new();
        for mut path in paths {
            let mut unchanged = path.clone();
            unchanged.push(segment.clone());
            next.push(unchanged);
            if next.len() == limit {
                break;
            }
            if is_initial_token(segment) {
                path.extend(segment.bytes().map(|byte| char::from(byte).to_string()));
                next.push(path);
                if next.len() == limit {
                    break;
                }
            }
        }
        paths = next;
    }
    for path in paths {
        if path != segments && path != result[0] {
            result.push(path);
            if result.len() == limit {
                break;
            }
        }
    }
    result
}

/// `max(limit * 16, 128)`, saturating (QQ:366-373).
pub fn build_mixed_jianpin_scan_limit(limit: usize) -> usize {
    // The C++ saturates at INT_MAX, which also keeps the value bindable as an SQL LIMIT.
    const SATURATED: usize = i32::MAX as usize;
    if limit > SATURATED / 16 {
        SATURATED
    } else {
        (limit * 16).max(128)
    }
}

/// All segments intact (QQ:375-385).
pub fn can_match_exact_key(segments: &[String]) -> bool {
    !segments.is_empty() && segments.iter().all(|segment| is_intact(segment))
}

/// Last and single-letter segments get a `%` suffix, joined with `'` (QQ:258-274). The range prefix is this minus its final character, so an inner single letter keeps a literal `%` and matches nothing, which is what sends such input on to the mixed-jianpin step.
pub fn build_key_like_pattern(segments: &[String]) -> String {
    let last = segments.len().saturating_sub(1);
    let capacity = segments.iter().map(String::len).sum::<usize>() + segments.len();
    let mut result = String::with_capacity(capacity);
    for (index, segment) in segments.iter().enumerate() {
        if index > 0 {
            result.push('\'');
        }
        result.push_str(segment);
        if index == last || segment.len() == 1 {
            result.push('%');
        }
    }
    result
}

/// `prefix + "{"`: `{` sorts right after `z` (QQ:276-279).
pub fn key_prefix_upper_bound(prefix: &str) -> String {
    let mut result = String::with_capacity(prefix.len() + 1);
    result.push_str(prefix);
    result.push('{');
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn segments(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|part| (*part).to_owned()).collect()
    }

    #[test]
    fn jianpin_shapes() {
        assert!(is_pure_jianpin(&segments(&["n", "h"])));
        assert!(!is_pure_jianpin(&segments(&["n", "hao"])));
        assert!(needs_mixed_jianpin_query(
            &segments(&["n", "hao"]),
            QuerySource::Quanpin
        ));
        assert!(!needs_mixed_jianpin_query(
            &segments(&["ni", "h"]),
            QuerySource::Quanpin
        ));
        assert!(!needs_mixed_jianpin_query(
            &segments(&["n"]),
            QuerySource::Quanpin
        ));
        assert!(needs_mixed_jianpin_query(
            &segments(&["zh", "guo"]),
            QuerySource::Quanpin
        ));
        assert!(needs_mixed_jianpin_query(
            &segments(&["zh", "guo"]),
            QuerySource::Shuangpin
        ));
        assert!(can_match_exact_key(&segments(&["ni", "hao"])));
        assert!(!can_match_exact_key(&segments(&["ni", "h"])));
        assert!(!can_match_exact_key(&[]));
    }

    #[test]
    fn initial_tokens() {
        assert_eq!(extract_initial_token("zhong"), "zh");
        assert_eq!(extract_initial_token("zong"), "z");
        assert_eq!(extract_initial_token("a"), "a");
        assert_eq!(extract_initial_token(""), "");
    }

    #[test]
    fn initial_split_paths_keep_mixed_and_fully_split_readings() {
        let typed = segments(&["zh", "sh", "da", "x"]);
        let paths = split_initial_token_paths(&typed, 32);
        assert_eq!(paths.len(), 3);
        for expected in [
            segments(&["z", "h", "s", "h", "da", "x"]),
            segments(&["zh", "s", "h", "da", "x"]),
            segments(&["z", "h", "sh", "da", "x"]),
        ] {
            assert!(paths.contains(&expected));
        }
        assert!(split_initial_token_paths(&segments(&["zhi", "chi", "shi"]), 32).is_empty());
        assert!(split_initial_token_paths(&typed, 0).is_empty());
        let long = vec!["zh".to_owned(); 64];
        for limit in [1, 2, 7, 32] {
            let paths = split_initial_token_paths(&long, limit);
            assert!(paths.len() <= limit);
            assert_eq!(paths[0], ["z", "h"].repeat(64));
            assert!(paths.iter().all(|path| path.concat() == "zh".repeat(64)));
        }
    }

    #[test]
    fn mixed_segments_match_per_position() {
        let typed = segments(&["n", "hao"]);
        assert!(matches_mixed_segments(
            "ni'hao",
            &typed,
            QuerySource::Quanpin
        ));
        assert!(!matches_mixed_segments(
            "ni'hai",
            &typed,
            QuerySource::Quanpin
        ));
        assert!(!matches_mixed_segments(
            "ni'hao'a",
            &typed,
            QuerySource::Quanpin
        ));
        let typed = segments(&["z", "guo"]);
        assert!(matches_mixed_segments(
            "zhong'guo",
            &typed,
            QuerySource::Quanpin
        ));
        assert!(matches_mixed_segments(
            "zong'guo",
            &typed,
            QuerySource::Quanpin
        ));
        // Under shuangpin a lone `z` is the flat initial and must not match `zh`.
        assert!(!matches_mixed_segments(
            "zhong'guo",
            &typed,
            QuerySource::Shuangpin
        ));
        assert!(matches_mixed_segments(
            "zong'guo",
            &typed,
            QuerySource::Shuangpin
        ));
        let typed = segments(&["zh", "guo"]);
        assert!(!matches_mixed_segments(
            "zong'guo",
            &typed,
            QuerySource::Quanpin
        ));
        assert!(matches_mixed_segments(
            "zhong'guo",
            &typed,
            QuerySource::Shuangpin
        ));
        assert!(!matches_mixed_segments(
            "zong'guo",
            &typed,
            QuerySource::Shuangpin
        ));
        assert!(matches_mixed_segments(
            "zhong'guo",
            &typed,
            QuerySource::Quanpin
        ));
        assert!(!matches_mixed_segments(
            "ni''hao",
            &segments(&["n", "", "hao"]),
            QuerySource::Quanpin
        ));
    }

    #[test]
    fn like_patterns_and_bounds() {
        assert_eq!(build_key_like_pattern(&segments(&["ni", "hao"])), "ni'hao%");
        assert_eq!(build_key_like_pattern(&segments(&["n", "hao"])), "n%'hao%");
        assert_eq!(build_key_like_pattern(&segments(&["n", "h"])), "n%'h%");
        assert_eq!(build_key_like_pattern(&[]), "");
        assert_eq!(key_prefix_upper_bound("ni'hao"), "ni'hao{");
        let prefix = "ping'guo'ji'hao";
        assert_eq!(key_prefix_upper_bound(prefix).capacity(), prefix.len() + 1);
        assert_eq!(build_mixed_jianpin_scan_limit(1), 128);
        assert_eq!(build_mixed_jianpin_scan_limit(20), 320);
        assert_eq!(
            build_mixed_jianpin_scan_limit(i32::MAX as usize),
            i32::MAX as usize
        );
        assert_eq!(
            build_mixed_jianpin_scan_limit(usize::MAX),
            i32::MAX as usize
        );
    }
}
