//! 构建补充表与个人新词学习共用的五笔词组取码规则。

/// 为两个及以上汉字取词组码；回调必须返回该字所有最长全码一致的前缀。
/// 无法取码时返回对应字，空串或单字返回 `None`。
pub fn phrase_code(
    word: &str,
    mut prefix: impl FnMut(char, usize) -> Option<String>,
) -> Option<std::result::Result<String, char>> {
    let chars: Vec<char> = word.chars().collect();
    let parts = match chars.len() {
        0 | 1 => return None,
        2 => vec![(chars[0], 2), (chars[1], 2)],
        3 => vec![(chars[0], 1), (chars[1], 1), (chars[2], 2)],
        n => vec![
            (chars[0], 1),
            (chars[1], 1),
            (chars[2], 1),
            (chars[n - 1], 1),
        ],
    };
    let mut result = String::with_capacity(4);
    for (character, length) in parts {
        let Some(code) = prefix(character, length) else {
            return Some(Err(character));
        };
        if code.len() != length || !code.bytes().all(|byte| byte.is_ascii_lowercase()) {
            return Some(Err(character));
        }
        result.push_str(&code);
    }
    Some(Ok(result))
}
