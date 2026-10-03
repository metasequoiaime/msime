//! 笔画输入：按笔顺键入横竖撇点折，读 `stroke.db`（`language_dictionary`）给出单字候选。只读，不学习，不做繁简转换。
//!
//! 键位：`h` 横、`s` 竖、`p` 撇、`n` 点（捺）、`z` 折，`x` 是匹配任意一笔的通配符。全部是小写字母，所以不占用任何标点键，也不需要宿主把它们当拼写符号转发。

pub mod scheme;

pub use scheme::{StrokeCandidate, StrokeScheme};

/// 五种笔画的键，按横竖撇点折的顺序。
pub const STROKES: &str = "hspnz";

/// 匹配任意一笔的通配键。空组合时不开始组合。
pub const WILDCARD: u8 = b'x';

/// 一个组合最多的笔画数（通配符也算一笔）；更多的键被吞掉。
pub const MAX_STROKES: usize = 64;

/// `key` 是五种笔画之一。
pub const fn is_stroke(key: u8) -> bool {
    matches!(key, b'h' | b's' | b'p' | b'n' | b'z')
}

/// `key` 是笔画或通配符，也就是组合里能出现的键。
pub const fn is_key(key: u8) -> bool {
    is_stroke(key) || key == WILDCARD
}

/// 预编辑里显示的笔画字形：h→一 s→丨 p→丿 n→丶 z→乛 x→＊。其他键没有字形。
pub const fn glyph(key: u8) -> Option<char> {
    match key {
        b'h' => Some('一'),
        b's' => Some('丨'),
        b'p' => Some('丿'),
        b'n' => Some('丶'),
        b'z' => Some('乛'),
        WILDCARD => Some('＊'),
        _ => None,
    }
}

/// 字形回到键，`glyph` 的反函数；宿主编辑时可能送回字形。
pub const fn key_of_glyph(glyph: char) -> Option<u8> {
    match glyph {
        '一' => Some(b'h'),
        '丨' => Some(b's'),
        '丿' => Some(b'p'),
        '丶' => Some(b'n'),
        '乛' => Some(b'z'),
        '＊' => Some(WILDCARD),
        _ => None,
    }
}

/// 测试用的合成 `stroke.db`：共用 `language_dictionary::SCHEMA`，权重是虚构的。
#[cfg(test)]
pub(crate) mod fixture {
    use std::path::Path;

    use rusqlite::Connection;

    use crate::language_dictionary::{
        FORMAT_VERSION, METADATA_FORMAT_VERSION, METADATA_LICENSE, METADATA_SOURCE_COMMIT, SCHEMA,
    };

    /// `土` 有两个笔画码，用来检查同一个字只列一次。
    pub(crate) const ENTRIES: [(&str, &str, i64); 11] = [
        ("h", "一", 9000),
        ("hh", "二", 5000),
        ("hhh", "三", 4000),
        ("hs", "十", 4500),
        ("hsh", "土", 2000),
        ("hshh", "土", 10),
        ("hhsh", "王", 2500),
        ("hhs", "干", 1500),
        ("hpn", "大", 5500),
        ("pn", "人", 6000),
        ("szh", "口", 3500),
    ];

    pub(crate) fn build(path: &Path) {
        let connection = Connection::open(path).unwrap();
        connection.execute_batch(SCHEMA).unwrap();
        for (name, value) in [
            (METADATA_FORMAT_VERSION, FORMAT_VERSION.to_string()),
            (
                METADATA_SOURCE_COMMIT,
                "0000000000000000000000000000000000000000".to_owned(),
            ),
            (METADATA_LICENSE, "LGPL-3.0-only".to_owned()),
        ] {
            connection
                .execute("INSERT INTO metadata VALUES (?1, ?2)", (name, value))
                .unwrap();
        }
        for stroke in super::STROKES.chars() {
            connection
                .execute("INSERT INTO syllables VALUES (?1)", (stroke.to_string(),))
                .unwrap();
        }
        for (key, text, weight) in ENTRIES {
            connection
                .execute(
                    "INSERT INTO entries VALUES (?1, ?2, ?3)",
                    (key, text, weight),
                )
                .unwrap();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_key_has_a_glyph_that_maps_back() {
        for key in STROKES.bytes().chain([WILDCARD]) {
            assert!(is_key(key));
            let glyph = glyph(key).unwrap();
            assert_eq!(key_of_glyph(glyph), Some(key));
        }
        assert!(!is_stroke(WILDCARD));
        for key in *b"aH1'*" {
            assert!(!is_key(key));
            assert_eq!(glyph(key), None);
        }
        assert_eq!(key_of_glyph('a'), None);
    }
}
