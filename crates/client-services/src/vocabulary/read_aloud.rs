//! 「朗读」读哪段文字。
//!
//! 候选菜单里的「朗读」和背单词卡片上的喇叭都只读英文单词或短语，由各平台系统自带的语音合成读出。读哪段文字在这里定，七个前端经 `msime_client_read_aloud_text`（或桌面设置应用直接调用）取得同一个答案，不各写一份。决定和理由见 `.agents/notes/implemented/feature/2026-11-01-system-tts-read-aloud.md`。

/// 能交给语音合成的最长文字，与生词本词头的上限相同。
pub const MAX_READ_ALOUD_CHARS: usize = super::wordbook::MAX_WORD_CHARS;

/// `text` 是不是可以朗读的英文：去掉首尾空白后只含 ASCII 字母、空格、`-`、`'`、`.`，以字母开头，不超过 [`MAX_READ_ALOUD_CHARS`] 个字符。是的话返回去掉首尾空白的那段。
///
/// 带中文、日文、括号或数字的都不算：宁可不出「朗读」，也不让系统用别的语言的声音念一串读不通的东西。
pub fn english_text(text: &str) -> Option<&str> {
    let text = text.trim();
    let first = text.chars().next()?;
    (first.is_ascii_alphabetic()
        && text.len() <= MAX_READ_ALOUD_CHARS
        && text
            .chars()
            .all(|c| c.is_ascii_alphabetic() || matches!(c, ' ' | '-' | '\'' | '.')))
    .then_some(text)
}

/// 候选 `candidate` 在显示着释义 `gloss` 时，「朗读」要读的文字；没有能读的时为 `None`，宿主据此不出这一项。
///
/// 释义按行（换行或 U+2028，各宿主交给会话的分行符）拆开，每行取第一个义项（`;` 或 `；` 之前），第一个是英文的就是答案，和「加入生词本」取词头的方式相同；几种语言的释义里英文不在第一行也找得到。没有英文释义时，候选本身是英文（混输里的英文候选）就读候选。背单词卡片把词头当作 `candidate`、释义留空。
pub fn read_aloud_text(candidate: &str, gloss: &str) -> Option<String> {
    gloss
        .split(['\n', '\r', '\u{2028}'])
        .filter_map(|line| {
            line.split([';', '；'])
                .map(str::trim)
                .find(|sense| !sense.is_empty())
        })
        .find_map(english_text)
        .or_else(|| english_text(candidate))
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_sense_of_an_english_gloss_is_read() {
        assert_eq!(
            read_aloud_text("面试", "interview; audition").as_deref(),
            Some("interview")
        );
        assert_eq!(
            read_aloud_text("面试", "  job interview ；面谈").as_deref(),
            Some("job interview")
        );
        assert_eq!(read_aloud_text("不", "don't").as_deref(), Some("don't"));
        assert_eq!(read_aloud_text("等等", "etc.").as_deref(), Some("etc."));
        assert_eq!(read_aloud_text("电邮", "e-mail").as_deref(), Some("e-mail"));
    }

    #[test]
    fn a_later_line_is_used_when_the_first_is_not_english() {
        assert_eq!(
            read_aloud_text("面试", "面接\u{2028}interview").as_deref(),
            Some("interview")
        );
        assert_eq!(
            read_aloud_text("面试", "\n面接\ninterview\n").as_deref(),
            Some("interview")
        );
    }

    #[test]
    fn senses_with_other_scripts_digits_or_brackets_are_not_read() {
        assert_eq!(read_aloud_text("面试", "面接"), None);
        assert_eq!(read_aloud_text("面试", "interview (n.)"), None);
        assert_eq!(read_aloud_text("一", "1"), None);
        assert_eq!(read_aloud_text("面试", "-interview"), None);
        assert_eq!(read_aloud_text("咖啡", "café"), None);
        assert_eq!(read_aloud_text("面试", ""), None);
        assert_eq!(read_aloud_text("", ""), None);
    }

    #[test]
    fn an_english_candidate_is_read_when_no_gloss_is() {
        assert_eq!(read_aloud_text("hello", "").as_deref(), Some("hello"));
        assert_eq!(read_aloud_text("hello", "你好").as_deref(), Some("hello"));
        assert_eq!(read_aloud_text("你好", "hello").as_deref(), Some("hello"));
    }

    #[test]
    fn text_longer_than_a_headword_is_not_read() {
        let long = "a".repeat(MAX_READ_ALOUD_CHARS + 1);
        assert_eq!(english_text(&long), None);
        assert_eq!(
            english_text(&"a".repeat(MAX_READ_ALOUD_CHARS)).map(str::len),
            Some(MAX_READ_ALOUD_CHARS)
        );
        // 整句的 AI 译文超过上限时不读，免得一点就念上一大段。
        assert_eq!(read_aloud_text("长句", &"word ".repeat(20)), None);
    }
}
