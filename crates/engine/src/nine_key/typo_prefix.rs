//! 九键整串切不成音节时，候选退到错键之前能切成完整音节的最长前缀。
//!
//! 打长句时多按或按错一键，整串数字就没有一条音节切分。原先这时只剩「开头一个音节」的兜底路径，候选塌成开头的单字（`6442677` 的首行是「你」），前面已经打对的词全部要重选。这里在严格切分失败之后找出能由完整音节一路切到的最远位置，候选只按这一段数字来查、来核对覆盖，与用户刚好打到那一位时查到的相同（`6442677` 的首行是「你好」）：这一段的切分路径和那时是同一批，最后一段没打完的也在内，所以那时靠补全排在首行的词现在仍在首行（`546877426` 的前 4 键既是 `ji'mu` 也是没打完的 `jin't`，首行仍是「今天」，不会换成只按完整音节切出来的「积木」）。
//!
//! 这不是纠错：不猜用户想按哪个键，不跨过错键组词，错键和它后面的键留在组字里等用户改。拼写表收了每个音节的每个前缀，所以整串一旦切不开，后面再按多少键这个最远位置都不会前进，错键之后每一键查的是同一段前缀。三种双拼网格一个音节两位，不纳入。取舍见 `.agents/notes/implemented/bug-fix/2026-11-12-nine-key-typo-reading-and-prefix-candidates.md`。

use super::{marks_agree, Path, SpellingTable, SyllablePrior, DIGIT_LIMIT, QUERY_PATH_LIMIT};

impl SpellingTable {
    /// `digits` 开头能切成若干完整音节的最长一段有几位，一个音节也切不出时是 0。规则与严格切分相同：按整组码查表、精确码的位置字母必须对上、不跨 `splits`（`digits` 里的下标）；与它不同的是每一段都必须是完整音节，最后一段也不例外。
    ///
    /// 只对按字母归组的全拼网格有效，双拼网格、空串、超过 `DIGIT_LIMIT` 的串返回 0。从前往后标出到得了的位置，最多 48 个位置各查 `longest_code` 次表，不做堆分配。
    pub(super) fn complete_prefix_len(&self, digits: &str, splits: &[usize]) -> usize {
        let length = digits.len();
        if length == 0 || length > DIGIT_LIMIT || !digits.is_ascii() || self.grid.keypad().is_none()
        {
            return 0;
        }
        let mut buffer = [0; DIGIT_LIMIT];
        let groups = self.grid.group_codes(digits, &mut buffer);
        let codes = groups.as_deref().unwrap_or(digits);
        let typed = digits.as_bytes();
        let mut reachable = [false; DIGIT_LIMIT + 1];
        reachable[0] = true;
        let mut farthest = 0;
        for offset in 0..length {
            if !reachable[offset] {
                continue;
            }
            for end in offset + 1..=length.min(offset + self.longest_code) {
                if splits.iter().any(|&split| offset < split && split < end) {
                    break;
                }
                if reachable[end] {
                    continue;
                }
                let Some(pieces) = self.by_code.get(&codes[offset..end]) else {
                    continue;
                };
                if pieces.iter().any(|(piece, complete)| {
                    *complete && (groups.is_none() || marks_agree(piece, &typed[offset..end]))
                }) {
                    reachable[end] = true;
                    farthest = farthest.max(end);
                }
            }
        }
        farthest
    }
}

/// `remaining` 整串切不开时，把它最长可切前缀的切分路径接到 `alternatives` 后面，返回这段前缀的长度；一个音节也切不出时什么都不做，返回 0。只在严格切分没有结果之后调用。
///
/// 路径就是对这段前缀做的严格切分，与用户刚好打到那一位时的是同一批：最后一段可以没打完，`splits` 里落在前缀之内（含正好在末尾）的照样守住，路径数是同一个上限。没打完的那一段查出来的是补全词，它后面的字母对不上错键，所以调用方核对覆盖时也只看这段前缀，不看它后面的键。
pub(super) fn append_prefix_paths(
    table: &SpellingTable,
    remaining: &str,
    splits: &[usize],
    prior: &SyllablePrior,
    alternatives: &mut Vec<Path>,
) -> usize {
    let prefix = table.complete_prefix_len(remaining, splits);
    if prefix == 0 {
        return 0;
    }
    let mut inner = [0usize; DIGIT_LIMIT];
    let mut count = 0;
    for &split in splits {
        if split <= prefix && count < DIGIT_LIMIT {
            inner[count] = split;
            count += 1;
        }
    }
    alternatives.extend(table.split_paths_limited(
        &remaining[..prefix],
        &inner[..count],
        prior,
        QUERY_PATH_LIMIT,
    ));
    prefix
}
