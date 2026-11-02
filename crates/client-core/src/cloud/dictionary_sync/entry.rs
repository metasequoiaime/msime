//! 「我的词条」的身份：词库类别、编码和词，三者一起决定一个词条。权重不在里面（打字学习会改它，不同步）。规范化规则与 Engine 写入个人词库时的相同，所以从 Engine 读出来的词条和收到的记录用同一个身份比较。

use serde::{Deserialize, Serialize};
use std::fmt;

/// 编码的字节上限，与 Engine 个人词库的 `MAX_KEY_BYTES` 相同。
pub const MAX_CODE_BYTES: usize = 512;
/// 词的字节上限，与 Engine 个人词库的 `MAX_VALUE_BYTES` 相同。
pub const MAX_WORD_BYTES: usize = 4096;
/// 粤语音节的字母上限，与 Engine 个人词库的 `MAX_JYUTPING_SYLLABLE_LETTERS` 相同。
const MAX_JYUTPING_SYLLABLE_LETTERS: usize = 8;

/// 词库类别，名字与 Engine 日志里的 `dictionary` 列相同。按名字的字母顺序排列，词条因此有确定的顺序。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WordKind {
    /// 粤语个人词。只在同一台设备上的两份词库之间交换（host-api 的 `user_words_mirror`）；WebDAV 同步不收它，宿主读写本机词库时就把它挡在外面（`docs/dictionary-webdav-sync.md` 的类别表里没有它，老版本读到它会整份检查点不认）。
    Cantonese,
    English,
    Pinyin,
    Quick,
    Wubi,
    Wubi98,
}

impl WordKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Cantonese => "cantonese",
            Self::English => "english",
            Self::Pinyin => "pinyin",
            Self::Quick => "quick",
            Self::Wubi => "wubi",
            Self::Wubi98 => "wubi98",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "cantonese" => Self::Cantonese,
            "english" => Self::English,
            "pinyin" => Self::Pinyin,
            "quick" => Self::Quick,
            "wubi" => Self::Wubi,
            "wubi98" => Self::Wubi98,
            _ => return None,
        })
    }
}

/// 一个词条的身份。只能经 [`WordKey::new`] 得到，字段因此总是规范化过、合规的。
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WordKey {
    kind: WordKind,
    code: String,
    word: String,
}

// 词条是用户的私人内容，调试输出里只留类别和长度。
impl fmt::Debug for WordKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WordKey")
            .field("kind", &self.kind)
            .field("code_bytes", &self.code.len())
            .field("word_bytes", &self.word.len())
            .finish()
    }
}

impl WordKey {
    /// 规范化并校验：编码转成小写 ASCII，拼音的空格换成音节分隔符 `'`。规则按类别与 Engine 相同：拼音是用 `'` 隔开的小写字母音节；五笔 1 到 4 个字母；快捷短语 1 到 32 个字母或数字；英文最多 64 个字母、`-` 或 `'`；粤语是 Engine 存进日志的写法，不带声调、以单个空格隔开的音节，每个 1 到 8 个小写字母（声调和 `'` 分隔由 Engine 在写入时收掉，这里不再规范化）。词不能为空、不超过 [`MAX_WORD_BYTES`]，不含控制字符（快捷短语可以有换行和制表符）。拼音、粤语音节是否真的存在、个数是否与字数相同由 Engine 在写入时判断。
    pub fn new(kind: WordKind, code: &str, word: &str) -> Option<Self> {
        let mut code = code.to_ascii_lowercase();
        if kind == WordKind::Pinyin {
            code = code.replace(' ', "'");
        }
        let letter = |byte: u8| byte.is_ascii_lowercase();
        let code_ok = !code.is_empty()
            && code.len() <= MAX_CODE_BYTES
            && match kind {
                WordKind::Pinyin => code
                    .split('\'')
                    .all(|syllable| !syllable.is_empty() && syllable.bytes().all(letter)),
                WordKind::Cantonese => code.split(' ').all(|syllable| {
                    (1..=MAX_JYUTPING_SYLLABLE_LETTERS).contains(&syllable.len())
                        && syllable.bytes().all(letter)
                }),
                WordKind::Wubi | WordKind::Wubi98 => code.len() <= 4 && code.bytes().all(letter),
                WordKind::Quick => {
                    code.len() <= 32
                        && code
                            .bytes()
                            .all(|byte| letter(byte) || byte.is_ascii_digit())
                }
                WordKind::English => {
                    code.len() <= 64
                        && code
                            .bytes()
                            .all(|byte| letter(byte) || b"-'".contains(&byte))
                }
            };
        let multiline = kind == WordKind::Quick;
        let word_ok = !word.is_empty()
            && word.len() <= MAX_WORD_BYTES
            && !word.bytes().any(|byte| {
                (byte < 0x20 && !(multiline && (byte == b'\n' || byte == b'\t'))) || byte == 0x7f
            });
        (code_ok && word_ok).then(|| Self {
            kind,
            code,
            word: word.to_owned(),
        })
    }

    /// 收到的记录必须已经是规范写法：规范化后与原文不同的不认，免得两台设备把同一个词条算成两个。
    pub fn canonical(kind: WordKind, code: &str, word: &str) -> Option<Self> {
        Self::new(kind, code, word).filter(|key| key.code == code)
    }

    pub fn kind(&self) -> WordKind {
        self.kind
    }

    pub fn code(&self) -> &str {
        &self.code
    }

    pub fn word(&self) -> &str {
        &self.word
    }

    /// 序列化后的大致字节数，用来给一批操作和检查点限大小。
    pub(crate) fn encoded_len(&self) -> usize {
        // 类别名、引号、逗号和 JSON 转义的余量。
        self.code.len() + self.word.len() * 2 + 24
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_normalised_the_way_the_engine_stores_them() {
        let key = WordKey::new(WordKind::Pinyin, "SHUI SHAN", "水杉").unwrap();
        assert_eq!(key.code(), "shui'shan");
        assert_eq!(
            WordKey::new(WordKind::Wubi, "IIEE", "水杉").unwrap().code(),
            "iiee"
        );
        assert_eq!(
            WordKey::new(WordKind::English, "Dont", "don't")
                .unwrap()
                .code(),
            "dont"
        );
        // 规范化后相同的两种写法是同一个词条。
        assert_eq!(
            WordKey::new(WordKind::Pinyin, "shui shan", "水杉"),
            WordKey::new(WordKind::Pinyin, "shui'shan", "水杉")
        );
        // 词不规范化：码位不同就是不同的词条。
        assert_ne!(
            WordKey::new(WordKind::Quick, "a", "e\u{301}"),
            WordKey::new(WordKind::Quick, "a", "\u{e9}")
        );
    }

    #[test]
    fn received_records_must_already_be_canonical() {
        assert!(WordKey::canonical(WordKind::Pinyin, "shui'shan", "水杉").is_some());
        assert!(WordKey::canonical(WordKind::Pinyin, "shui shan", "水杉").is_none());
        assert!(WordKey::canonical(WordKind::Wubi, "IIEE", "水杉").is_none());
        // 粤语照 Engine 日志里的写法：音节以单个空格隔开，不带声调。
        assert!(WordKey::canonical(WordKind::Cantonese, "nei hou", "你好").is_some());
        assert!(WordKey::canonical(WordKind::Cantonese, "NEI HOU", "你好").is_none());
    }

    #[test]
    fn rejects_codes_and_words_the_engine_would_refuse() {
        for (kind, code, word) in [
            (WordKind::Pinyin, "", "水"),
            (WordKind::Pinyin, "shui''shan", "水杉"),
            (WordKind::Pinyin, "'shui", "水"),
            (WordKind::Pinyin, "shui1", "水"),
            (WordKind::Wubi, "abcde", "词"),
            (WordKind::Wubi98, "a1", "词"),
            (WordKind::Quick, &"a".repeat(33), "词"),
            (WordKind::Quick, "a_b", "词"),
            (WordKind::English, &"a".repeat(65), "word"),
            (WordKind::English, "a b", "word"),
            (WordKind::Cantonese, "nei'hou", "你好"),
            (WordKind::Cantonese, "nei  hou", "你好"),
            (WordKind::Cantonese, "nei5 hou2", "你好"),
            (WordKind::Cantonese, " nei", "你"),
            (WordKind::Cantonese, "abcdefghi", "字"),
            (WordKind::Pinyin, "shui", ""),
            (WordKind::Pinyin, "shui", "水\n"),
            (WordKind::English, "word", "wo\u{7f}rd"),
        ] {
            assert!(
                WordKey::new(kind, code, word).is_none(),
                "{kind:?} {code:?}"
            );
        }
        assert!(WordKey::new(WordKind::Quick, "addr", "第一行\n\t第二行").is_some());
        assert!(WordKey::new(WordKind::Quick, "a", &"字".repeat(MAX_WORD_BYTES / 3)).is_some());
        assert!(WordKey::new(WordKind::Quick, "a", &"a".repeat(MAX_WORD_BYTES + 1)).is_none());
    }

    #[test]
    fn debug_output_does_not_contain_the_word_or_code() {
        let key = WordKey::new(WordKind::Pinyin, "mi'ma", "秘密词").unwrap();
        let debug = format!("{key:?}");
        assert!(
            !debug.contains("秘密词") && !debug.contains("mi'ma"),
            "{debug}"
        );
    }

    #[test]
    fn kinds_round_trip_through_their_names_and_sort_by_name() {
        let mut kinds = [
            WordKind::Wubi98,
            WordKind::Cantonese,
            WordKind::Pinyin,
            WordKind::English,
            WordKind::Wubi,
            WordKind::Quick,
        ];
        kinds.sort();
        let names: Vec<&str> = kinds.iter().map(|kind| kind.name()).collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted);
        for kind in kinds {
            assert_eq!(WordKind::from_name(kind.name()), Some(kind));
        }
        assert_eq!(WordKind::from_name("bundled"), None);
    }
}
