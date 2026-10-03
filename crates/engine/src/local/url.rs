//! 网址模式：在全拼、双拼、五笔的组字中键入 `www.`、`http:`、`https:`、`ftp.`、`ftp:` 后，后续按键原样作为 ASCII 网址输入，不再转成中文。模式没有候选，只有一行显示整段网址的兜底行。

/// 网址模式下作为输入而非标点的非字母字符：数字和网址里出现的符号。Shift+数字行的符号必须全部列出，宿主才不会把它们当成选候选。
pub const SPELLING_SYMBOLS: &str = "0123456789-._~:/?#[]@!$&'()*+,;=%^";

/// 网址的最长长度，超出后按键被吞掉。
pub const INPUT_LIMIT: usize = 512;

/// 触发词及其触发键。组字原文必须与触发词完全相同（全部小写 ASCII 字母）。
const TRIGGERS: [(&str, &str); 4] = [("www", "."), ("http", ":"), ("https", ":"), ("ftp", ".:")];

/// 组字原文为 `raw` 时能进入网址模式的按键；原文不是触发词时为空。
pub fn entry_keys(raw: &str) -> &'static str {
    TRIGGERS
        .iter()
        .find(|(word, _)| *word == raw)
        .map_or("", |(_, keys)| keys)
}

/// 组字原文为 `raw` 时按下 `key` 是否进入网址模式。
pub fn opens(raw: &str, key: u8) -> bool {
    entry_keys(raw).as_bytes().contains(&key)
}

/// 五笔码长上限为 4，`http` 之后的 `s` 会被码表拒绝，所以五笔在这第 5 个字母上直接进入网址模式。
pub fn wubi_continues(raw: &str, letter: u8) -> bool {
    raw == "http" && letter == b's'
}

/// `wubi_continues` 的逆操作：五笔网址模式删掉 `s` 后剩下 `http`，说明删掉的正是进入网址模式的那个字母，应退回组字。
pub fn wubi_reverts(remaining: &str, removed: char) -> bool {
    removed == 's' && remaining == "http"
}

/// 网址模式接受的按键：字母、数字和 `SPELLING_SYMBOLS` 中的符号。其余按键结束网址。
pub fn accepts(key: u8) -> bool {
    key.is_ascii_alphanumeric() || SPELLING_SYMBOLS.as_bytes().contains(&key)
}

#[cfg(test)]
mod tests {
    use super::{accepts, entry_keys, opens, wubi_continues, wubi_reverts, SPELLING_SYMBOLS};

    #[test]
    fn only_the_exact_scheme_words_open_the_mode() {
        assert_eq!(entry_keys("www"), ".");
        assert_eq!(entry_keys("http"), ":");
        assert_eq!(entry_keys("https"), ":");
        assert_eq!(entry_keys("ftp"), ".:");
        for raw in ["ww", "wwww", "Www", "w'ww", "ni", "", "mailto", "httpss"] {
            assert_eq!(entry_keys(raw), "", "{raw}");
        }
        assert!(opens("ftp", b':') && opens("ftp", b'.'));
        assert!(!opens("www", b':') && !opens("www", b'@'));
        assert!(wubi_continues("http", b's'));
        assert!(!wubi_continues("http", b't') && !wubi_continues("htt", b's'));
        assert!(wubi_reverts("http", 's'));
        assert!(!wubi_reverts("http", 't') && !wubi_reverts("htt", 's'));
    }

    #[test]
    fn every_listed_symbol_is_accepted_and_the_url_enders_are_not() {
        assert!(SPELLING_SYMBOLS.bytes().all(accepts));
        // Shift+数字行全部列出，宿主才不会把它们读成选候选。
        assert!("!@#$%^&*()0123456789"
            .bytes()
            .all(|key| SPELLING_SYMBOLS.as_bytes().contains(&key)));
        assert!(accepts(b'a') && accepts(b'Z') && accepts(b'7'));
        for key in *b"\"<>\\{}|` " {
            assert!(!accepts(key), "{}", key as char);
        }
    }
}
