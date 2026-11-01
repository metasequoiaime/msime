//! 语音结果上屏前的标点处理（`voice_input.punctuation_mode`）。
//!
//! 标点是识别器自己加的：本地模型、系统识别、豆包和整句上传的模型都会出标点，AI 润色的提示词也要求补标点。这里不改识别过程，只在最终文本上按用户选的方式处理，所以对每一种识别服务、润色前后都一样生效。中间结果不经过这里。
//!
//! 只处理识别器会写出的句读：`，。、；：？！…～`、它们的全角写法，以及半角的 `, . ; : ? !`。引号、括号、书名号、连字符、撇号、`%`、`@` 不在其中，原样保留。数字和网址里的半角点（3.5、v1.2、example.com）、数字之间的冒号和逗号（10:30、10：30、1,000）不算标点。

/// 用户选的语音标点处理方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PunctuationMode {
    /// 识别器给什么就上屏什么（出厂默认，等于加这个选项之前的行为）。
    Auto,
    /// 去掉结尾的句读，句中的保留。
    OmitTrailing,
    /// 去掉所有句读。
    OmitAll,
    /// 每一串句读换成一个空格，开头和结尾的直接去掉。
    Space,
}

impl PunctuationMode {
    /// 偏好里存的值。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::OmitTrailing => "omit_trailing",
            Self::OmitAll => "omit_all",
            Self::Space => "space",
        }
    }

    /// 偏好里选的方式。
    pub fn configured(preferences: &crate::preferences::Preferences) -> Self {
        Self::parse(&preferences.voice_input.punctuation_mode)
    }

    /// 读偏好里的值；空串和不认识的值（例如更新版本写下的新选项）按 [`Self::Auto`]，不改动识别结果。
    pub fn parse(value: &str) -> Self {
        match value.trim() {
            "omit_trailing" => Self::OmitTrailing,
            "omit_all" => Self::OmitAll,
            "space" => Self::Space,
            _ => Self::Auto,
        }
    }
}

/// 识别器会写出的句读。
fn is_sentence_punctuation(c: char) -> bool {
    matches!(
        c,
        '，' | '。'
            | '、'
            | '；'
            | '：'
            | '？'
            | '！'
            | '…'
            | '～'
            | '．'
            | '︒'
            | ','
            | '.'
            | ';'
            | ':'
            | '?'
            | '!'
    )
}

/// `chars[index]` 是句读，且不是数字或网址里的一部分。
fn is_removable(chars: &[char], index: usize) -> bool {
    let c = chars[index];
    if !is_sentence_punctuation(c) {
        return false;
    }
    let before = index.checked_sub(1).map(|i| chars[i]);
    let after = chars.get(index + 1).copied();
    let between = |test: fn(&char) -> bool| {
        before.is_some_and(|b| test(&b)) && after.is_some_and(|a| test(&a))
    };
    match c {
        // 3.5、v1.2、example.com：两边紧挨着字母或数字的半角点不是句号。
        '.' => !between(char::is_ascii_alphanumeric),
        // 10:30、10：30、1,000。
        ':' | '：' | ',' => !between(char::is_ascii_digit),
        _ => true,
    }
}

/// 成对符号的前半：空格代替标点时，空格不放在它后面。
fn is_opening(c: char) -> bool {
    matches!(
        c,
        '“' | '‘' | '「' | '『' | '（' | '(' | '【' | '[' | '《' | '〈' | '{'
    )
}

/// 成对符号的后半：空格代替标点时，空格不放在它前面。
fn is_closing(c: char) -> bool {
    matches!(
        c,
        '”' | '’' | '」' | '』' | '）' | ')' | '】' | ']' | '》' | '〉' | '}'
    )
}

/// 拉丁字母、数字这类词与词之间要靠空格分开的字符。汉字、假名、谚文虽然也算字母，删掉中间的标点后直接相连才是对的。
fn needs_word_space(c: char) -> bool {
    c.is_alphanumeric() && (c as u32) < 0x2E80
}

/// 按 `mode` 处理一段最终识别结果。[`PunctuationMode::Auto`] 原样返回；其他方式返回去掉首尾空白的结果，整段只有标点时得到空串。
pub fn apply(text: &str, mode: PunctuationMode) -> String {
    match mode {
        PunctuationMode::Auto => text.to_owned(),
        PunctuationMode::OmitTrailing => omit_trailing(text),
        PunctuationMode::OmitAll => replace_runs(text, false),
        PunctuationMode::Space => replace_runs(text, true),
    }
}

fn omit_trailing(text: &str) -> String {
    let chars: Vec<char> = text.trim().chars().collect();
    let mut end = chars.len();
    while end > 0 && (chars[end - 1].is_whitespace() || is_removable(&chars[..end], end - 1)) {
        end -= 1;
    }
    chars[..end].iter().collect()
}

/// 把每一串句读（连同夹在其中、紧挨着它的空白）换掉。`space` 为真时换成一个空格，紧挨着引号、括号内侧时不加；否则只在原来就隔着空白、或两边是要用空格分词的字母数字时留一个空格（`Hello, world` 和 `Hello,world` 都得到 `Hello world`，`你好，世界` 得到 `你好世界`）。
fn replace_runs(text: &str, space: bool) -> String {
    let chars: Vec<char> = text.trim().chars().collect();
    let mut output = String::with_capacity(text.len());
    let mut index = 0;
    while index < chars.len() {
        if !is_removable(&chars, index) {
            output.push(chars[index]);
            index += 1;
            continue;
        }
        let trimmed_len = output.trim_end().len();
        let mut spaced = trimmed_len != output.len();
        output.truncate(trimmed_len);
        while index < chars.len() && (chars[index].is_whitespace() || is_removable(&chars, index)) {
            spaced |= chars[index].is_whitespace();
            index += 1;
        }
        let Some(&next) = chars.get(index) else {
            break;
        };
        let Some(previous) = output.chars().next_back() else {
            continue;
        };
        let between_pair = is_opening(previous) || is_closing(next);
        if (space && !between_pair)
            || spaced
            || (needs_word_space(previous) && needs_word_space(next))
        {
            output.push(' ');
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all(text: &str) -> [String; 3] {
        [
            apply(text, PunctuationMode::OmitTrailing),
            apply(text, PunctuationMode::OmitAll),
            apply(text, PunctuationMode::Space),
        ]
    }

    #[test]
    fn parse_round_trips_and_falls_back_to_auto() {
        for mode in [
            PunctuationMode::Auto,
            PunctuationMode::OmitTrailing,
            PunctuationMode::OmitAll,
            PunctuationMode::Space,
        ] {
            assert_eq!(PunctuationMode::parse(mode.as_str()), mode);
        }
        assert_eq!(PunctuationMode::parse(""), PunctuationMode::Auto);
        assert_eq!(
            PunctuationMode::parse("strip_quotes"),
            PunctuationMode::Auto
        );
        let mut preferences = crate::preferences::Preferences::default();
        assert_eq!(
            PunctuationMode::configured(&preferences),
            PunctuationMode::Auto
        );
        preferences.voice_input.punctuation_mode = "omit_trailing".into();
        assert_eq!(
            PunctuationMode::configured(&preferences),
            PunctuationMode::OmitTrailing
        );
    }

    #[test]
    fn auto_keeps_the_recognizer_output() {
        assert_eq!(apply(" 你好。 ", PunctuationMode::Auto), " 你好。 ");
    }

    #[test]
    fn issue_examples() {
        assert_eq!(apply("你好。", PunctuationMode::OmitTrailing), "你好");
        assert_eq!(
            apply("明天去那里玩？", PunctuationMode::OmitTrailing),
            "明天去那里玩"
        );
        assert_eq!(
            all("天啊！他是谁？"),
            ["天啊！他是谁", "天啊他是谁", "天啊 他是谁"].map(String::from)
        );
    }

    #[test]
    fn trailing_runs_and_whitespace_are_removed_together() {
        assert_eq!(
            apply("真的吗？！……", PunctuationMode::OmitTrailing),
            "真的吗"
        );
        assert_eq!(apply("Really?! ", PunctuationMode::OmitTrailing), "Really");
        assert_eq!(
            apply("好的，我知道了。", PunctuationMode::OmitTrailing),
            "好的，我知道了"
        );
    }

    #[test]
    fn quotes_and_brackets_are_kept() {
        assert_eq!(
            all("他说：“好的。”（笑）"),
            [
                "他说：“好的。”（笑）",
                "他说“好的”（笑）",
                "他说 “好的”（笑）"
            ]
            .map(String::from)
        );
    }

    #[test]
    fn numbers_and_addresses_keep_their_dots_and_colons() {
        assert_eq!(
            all("涨了3.5%，10:30和10：30见，共1,000元。"),
            [
                "涨了3.5%，10:30和10：30见，共1,000元",
                "涨了3.5%10:30和10：30见共1,000元",
                "涨了3.5% 10:30和10：30见 共1,000元",
            ]
            .map(String::from)
        );
        assert_eq!(
            apply("打开 example.com 看 v1.2.", PunctuationMode::OmitAll),
            "打开 example.com 看 v1.2"
        );
    }

    #[test]
    fn latin_words_stay_apart() {
        assert_eq!(
            apply("Hello, world.", PunctuationMode::OmitAll),
            "Hello world"
        );
        assert_eq!(
            apply("Hello,world", PunctuationMode::OmitAll),
            "Hello world"
        );
        assert_eq!(
            apply("Hello , world", PunctuationMode::Space),
            "Hello world"
        );
        assert_eq!(apply("你好，世界", PunctuationMode::OmitAll), "你好世界");
        assert_eq!(
            apply("我用iPhone，很好用", PunctuationMode::OmitAll),
            "我用iPhone很好用"
        );
    }

    #[test]
    fn leading_punctuation_leaves_no_space() {
        assert_eq!(apply("，好的", PunctuationMode::Space), "好的");
        assert_eq!(apply("…好的", PunctuationMode::OmitAll), "好的");
    }

    #[test]
    fn punctuation_only_becomes_empty() {
        assert_eq!(all("。"), ["", "", ""].map(String::from));
        assert_eq!(all(" ？！ "), ["", "", ""].map(String::from));
    }

    #[test]
    fn other_symbols_are_untouched() {
        assert_eq!(
            apply("don't 用 user@mail 和 co-op~", PunctuationMode::OmitAll),
            "don't 用 user@mail 和 co-op~"
        );
    }
}
