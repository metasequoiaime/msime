use std::borrow::Cow;

/// 快捷短语的显示／上屏文本与显式光标位置。
#[derive(Debug, PartialEq, Eq)]
pub struct QuickPhraseExpansion<'a> {
    /// 展开后的正文；没有标记时直接借用输入。
    pub text: Cow<'a, str>,
    /// 展开后正文中的 UTF-8 字节边界；`None` 表示沿用宿主的默认位置。
    pub cursor_byte: Option<usize>,
}

/// 展开首个 `${cursor}`，并把 `$${cursor}` 还原成字面的 `${cursor}`。
///
/// 后续未转义的同名标记和未知变量按字面保留。本函数不判断候选来源，
/// 调用方只能对快捷短语使用它，不能把普通候选或原样提交当成模板。
pub fn expand_quick_phrase(input: &str) -> QuickPhraseExpansion<'_> {
    const MARKER: &str = "${cursor}";
    const ESCAPED: &str = "$${cursor}";
    if !input.contains(MARKER) {
        return QuickPhraseExpansion {
            text: Cow::Borrowed(input),
            cursor_byte: None,
        };
    }
    let mut text = String::with_capacity(input.len());
    let mut cursor_byte = None;
    let mut remaining = input;
    while let Some(index) = remaining.find('$') {
        text.push_str(&remaining[..index]);
        remaining = &remaining[index..];
        if let Some(rest) = remaining.strip_prefix(ESCAPED) {
            text.push_str(MARKER);
            remaining = rest;
        } else if cursor_byte.is_none() && remaining.starts_with(MARKER) {
            cursor_byte = Some(text.len());
            remaining = &remaining[MARKER.len()..];
        } else {
            text.push('$');
            remaining = &remaining[1..];
        }
    }
    text.push_str(remaining);
    QuickPhraseExpansion {
        text: Cow::Owned(text),
        cursor_byte,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quick_phrase_expansion_preserves_literal_and_unicode_boundaries() {
        for (input, expected, suffix) in [
            ("title:\"\"", "title:\"\"", None),
            ("title:\"${cursor}\"", "title:\"\"", Some("\"")),
            ("${cursor}开头", "开头", Some("开头")),
            ("末尾${cursor}", "末尾", Some("")),
            ("${cursor}", "", Some("")),
            ("$${cursor}", "${cursor}", None),
            ("$${cursor}${cursor}尾", "${cursor}尾", Some("尾")),
            ("${cursor}a${cursor}", "a${cursor}", Some("a${cursor}")),
            ("${cursor}$${cursor}", "${cursor}", Some("${cursor}")),
            ("${date} ${unknown}", "${date} ${unknown}", None),
            (
                "中文${cursor}𠀀👩‍💻e\u{301}",
                "中文𠀀👩‍💻e\u{301}",
                Some("𠀀👩‍💻e\u{301}"),
            ),
        ] {
            let expanded = expand_quick_phrase(input);
            assert_eq!(expanded.text, expected, "{input}");
            assert_eq!(
                expanded.cursor_byte.map(|index| &expanded.text[index..]),
                suffix,
                "{input}"
            );
        }
        assert!(matches!(
            expand_quick_phrase("普通短语").text,
            Cow::Borrowed(_)
        ));
    }
}
