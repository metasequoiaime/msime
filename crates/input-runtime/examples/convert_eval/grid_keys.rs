//! 评测工具在触屏网格上打一条用例时送的组码，`convert_eval` 和 `rerank_latency` 共用。
//!
//! 按字母归组的网格（九键、14 键）把字母逐个换成所在键的组码。九键双拼按音节编码，而用例的输入是不带分隔的全拼（`xianzai`），要先切成音节：`xian` 和 `xi'an` 在九键双拼上是不同的数字，切错了金标准就打不出来。

use msime_engine::host::{resolve_nine_key_shuangpin, NineKeyShuangpinKind};
use msime_engine::{KeyGrid, NineKeyShuangpin};

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
    let mut found: Vec<Vec<String>> = Vec::new();
    segment(table, input, &mut Vec::new(), syllables, &mut found);
    let pieces = match syllables {
        Some(_) if found.len() == 1 => found.pop()?,
        Some(_) => return None,
        None => found.into_iter().min_by_key(Vec::len)?,
    };
    let keys = table.encode(&pieces.join("'"));
    (!keys.is_empty()).then_some(keys)
}

/// 把 `rest` 切成九键双拼认识的音节，切法写进 `found`。数了音节时找到第二种就停（已经有歧义了）。
fn segment(
    table: &NineKeyShuangpin,
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
        if table.syllable_code(piece).is_some() {
            pieces.push(piece.to_owned());
            segment(table, &rest[end..], pieces, syllables, found);
            pieces.pop();
        }
    }
}
