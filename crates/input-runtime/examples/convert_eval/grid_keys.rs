//! 评测工具在触屏网格或 26 键双拼上打一条用例时送的键，`convert_eval` 和 `rerank_latency` 共用。
//!
//! 按字母归组的网格（九键、14 键）把字母逐个换成所在键的组码。九键双拼和 26 键双拼按音节编码，而用例的输入是不带分隔的全拼（`xianzai`），要先切成音节：`xian` 和 `xi'an` 在双拼上是不同的键，切错了金标准就打不出来。

use msime_engine::host::{
    encode_shuangpin, is_shuangpin_syllable, resolve_nine_key_shuangpin, NineKeyShuangpinKind,
};
use msime_engine::KeyGrid;

/// `--grid` 的取值：`nine`、`fourteen`，或 `shuangpin-<方案>`（`shuangpin-migala2`、`shuangpin-migala3`、`shuangpin-gushi2_low`）。
pub fn parse_grid(name: &str) -> Result<KeyGrid, String> {
    match name {
        "nine" => Ok(KeyGrid::NineKey),
        "fourteen" => Ok(KeyGrid::FourteenKey),
        other => other
            .strip_prefix("shuangpin-")
            .and_then(NineKeyShuangpinKind::from_name)
            .and_then(|kind| resolve_nine_key_shuangpin(kind, None).ok())
            .map(KeyGrid::NineKeyShuangpin)
            .ok_or_else(|| {
                format!(
                    "unknown grid: {other} (expected nine, fourteen or shuangpin-migala2|migala3|gushi2_low)"
                )
            }),
    }
}

/// 一条输入在 `grid` 上的组码。按字母归组的网格跳过 `'`。九键双拼要切成音节：`syllables` 给了时只认切出这么多音节、而且只有一种切法的，否则取音节最少的切法里最先找到的那一种；切不开或有歧义时为 `None`。输入里的 `'` 是必须切开的地方。
pub fn grid_keys(grid: KeyGrid, input: &str, syllables: Option<usize>) -> Option<String> {
    let KeyGrid::NineKeyShuangpin(table) = grid else {
        return Some(grid.encode(input));
    };
    let pieces = syllable_pieces(
        &|piece| table.syllable_code(piece).is_some(),
        input,
        syllables,
    )?;
    let keys = table.encode(&pieces.join("'"));
    (!keys.is_empty()).then_some(keys)
}

/// 一条全拼输入在内置 26 键双拼方案 `profile`（方案名）上的按键：切成音节的规则同 [`grid_keys`] 的九键双拼，再逐音节编成两键码，去掉分隔。纠错整句不接受手打的 `'`，所以按键里不带它，每个音节恰好两键；切不开、有歧义或有音节在这个方案里编不出来时为 `None`。切音节按与方案无关的音节表（[`is_shuangpin_syllable`]），不按这个方案编不编得出来：否则某个方案编不出的音节会让有歧义的切法看起来唯一，各方案打的用例集合就不一样了。
pub fn shuangpin_keys(profile: &str, input: &str, syllables: Option<usize>) -> Option<String> {
    let pieces = syllable_pieces(&is_shuangpin_syllable, input, syllables)?;
    let codes = encode_shuangpin(&pieces.join("'"), profile)?;
    let keys = codes.replace('\'', "");
    (keys.len() == 2 * pieces.len()).then_some(keys)
}

/// 按 `known` 认的音节切开 `input`：`syllables` 给了时只认切出这么多音节、而且只有一种切法的，否则取音节最少的切法里最先找到的那一种。
fn syllable_pieces(
    known: &dyn Fn(&str) -> bool,
    input: &str,
    syllables: Option<usize>,
) -> Option<Vec<String>> {
    let mut found: Vec<Vec<String>> = Vec::new();
    segment(known, input, &mut Vec::new(), syllables, &mut found);
    match syllables {
        Some(_) if found.len() == 1 => found.pop(),
        Some(_) => None,
        None => found.into_iter().min_by_key(Vec::len),
    }
}

/// 把 `rest` 切成 `known` 认识的音节，切法写进 `found`。数了音节时找到第二种就停（已经有歧义了）。
fn segment(
    known: &dyn Fn(&str) -> bool,
    rest: &str,
    pieces: &mut Vec<String>,
    syllables: Option<usize>,
    found: &mut Vec<Vec<String>>,
) {
    if syllables.is_some() && found.len() > 1 {
        return;
    }
    let rest = rest.strip_prefix('\'').unwrap_or(rest);
    if rest.is_empty() {
        if syllables.is_none_or(|count| count == pieces.len()) {
            found.push(pieces.clone());
        }
        return;
    }
    if syllables.is_some_and(|count| pieces.len() >= count) {
        return;
    }
    let boundary = rest.find('\'').unwrap_or(rest.len());
    for end in 1..=boundary.min(6) {
        let piece = &rest[..end];
        if known(piece) {
            pieces.push(piece.to_owned());
            segment(known, &rest[end..], pieces, syllables, found);
            pieces.pop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shuangpin_keys_follow_the_counted_segmentation() {
        assert_eq!(
            shuangpin_keys("xiaohe", "nihao", Some(2)).as_deref(),
            Some("nihc")
        );
        // 同一串全拼，音节数决定切法：一个音节是 xian，两个是 xi'an（零声母 an 在小鹤是 an）。
        assert_eq!(
            shuangpin_keys("xiaohe", "xian", Some(1)).as_deref(),
            Some("xm")
        );
        assert_eq!(
            shuangpin_keys("xiaohe", "xian", Some(2)).as_deref(),
            Some("xian")
        );
        // fang'an 和 fan'gan 都是两个音节，金标准是哪一个说不清，不打。
        assert_eq!(shuangpin_keys("xiaohe", "fangan", Some(2)), None);
        // 手打的 `'` 是必须切开的地方。
        assert_eq!(
            shuangpin_keys("xiaohe", "fang'an", Some(2)).as_deref(),
            Some("fhan")
        );
        assert_eq!(
            shuangpin_keys("microsoft", "aming", Some(2)).as_deref(),
            Some("oam;")
        );
        assert_eq!(shuangpin_keys("custom", "nihao", Some(2)), None);
    }

    /// 首道双拼（`shoudao`）的 `en` 和 `ia` 同在 `k` 上，`den` 编出来解回 `dia`，这个方案编不出 `den`。切法的歧义仍按真实的音节表判断：`dena` 是 de'na 还是 den'a 在每个方案上都说不清，不能因为首道编不出 `den` 就把 de'na 当成唯一切法，否则各方案打的用例集合就不一样了。
    #[test]
    fn shuangpin_keys_judge_ambiguity_by_syllables_not_by_profile() {
        assert!(shuangpin_keys("shoudao", "nihao", Some(2)).is_some());
        for profile in ["xiaohe", "ziranma", "shoudao", "microsoft", "smart_abc"] {
            assert_eq!(shuangpin_keys(profile, "dena", Some(2)), None, "{profile}");
        }
        // 唯一切法里有这个方案编不出来的音节：用例丢掉，而不是改打另一种切法。
        assert!(shuangpin_keys("xiaohe", "den", Some(1)).is_some());
        assert_eq!(shuangpin_keys("shoudao", "den", Some(1)), None);
        assert_eq!(shuangpin_keys("shoudao", "den'a", Some(2)), None);
    }
}
