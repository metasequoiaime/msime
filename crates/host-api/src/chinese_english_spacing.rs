//! 「中英文之间加空格」：一次上屏的文本里，汉字与 ASCII 字母、数字相邻处插入一个半角空格，语义同 rime-ice 的 `cn_en_spacer.lua`（#7172）。两次上屏之间不加。

use msime_client_core::dictionary::is_han_character;
use msime_engine::SchemeType;
use msime_input_runtime::OutputContext;

/// 这次上屏是否要加空格：只看上屏前记下的 `commit_context`，没有时不处理。
///
/// 方案必须是中文方案（日文、韩文、越南文、藏文不动）；本地模式只放行三种：普通组字 `none`（含云候选、AI 候选、五笔顶字的字母尾巴、暂存选词加原样字母）、K 模式快捷短语 `quick_phrase` 和 J 模式超级简拼 `super_jianpin`。网址、日期时间、Unicode、emoji、颜文字、计算、指令、提及、临时英文和临时日文的上屏是用户要原样得到的内容，一律不动。
pub(crate) fn applies(context: Option<&OutputContext>) -> bool {
    context.is_some_and(|context| {
        matches!(
            context.local_mode.as_str(),
            "none" | "quick_phrase" | "super_jianpin"
        ) && SchemeType::from_u8(context.scheme).is_some_and(SchemeType::is_chinese)
    })
}

/// 在 `text` 里汉字与 ASCII 字母、数字相邻的地方各插入一个半角空格；没有这样的相邻处时返回 `None`，原文不动，也不分配。
///
/// 已经隔着空格的不再插入，首尾也不加：上屏的第一个和最后一个字符保持不变，宿主的智能标点按它们判断前一个字符。汉字取 `is_han_character` 的范围（基本区、扩展 A 到 F 和兼容表意文字）；假名、谚文、全角字母和 ASCII 标点都不算。
pub(crate) fn space(text: &str) -> Option<String> {
    let boundary = |left: char, right: char| {
        (is_han_character(left) && right.is_ascii_alphanumeric())
            || (left.is_ascii_alphanumeric() && is_han_character(right))
    };
    let mut pairs = text.chars().zip(text.chars().skip(1));
    if !pairs.any(|(left, right)| boundary(left, right)) {
        return None;
    }
    let mut spaced = String::with_capacity(text.len() + 8);
    let mut previous = None;
    for character in text.chars() {
        if previous.is_some_and(|previous| boundary(previous, character)) {
            spaced.push(' ');
        }
        spaced.push(character);
        previous = Some(character);
    }
    Some(spaced)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chinese_english_spacing_inserts_between_han_and_ascii_letters_or_digits() {
        assert_eq!(space("用Rust写插件").as_deref(), Some("用 Rust 写插件"));
        assert_eq!(space("800人").as_deref(), Some("800 人"));
        assert_eq!(space("VIP中P").as_deref(), Some("VIP 中 P"));
        assert_eq!(space("第3版").as_deref(), Some("第 3 版"));
    }

    #[test]
    fn chinese_english_spacing_leaves_text_without_a_boundary_alone() {
        assert_eq!(space("用 Rust 写"), None);
        assert_eq!(space("你好世界"), None);
        assert_eq!(space("hello world 42"), None);
        assert_eq!(space("你好,"), None);
        assert_eq!(space(""), None);
        assert_eq!(space("中"), None);
    }

    #[test]
    fn chinese_english_spacing_only_counts_han_and_ascii_alphanumerics() {
        // 假名、谚文、全角字母和 ASCII 标点两侧都不加。
        assert_eq!(space("かなA"), None);
        assert_eq!(space("한A"), None);
        assert_eq!(space("用Ｒｕｓｔ写"), None);
        assert_eq!(space("中-A"), None);
        // 扩展 B 的汉字也是汉字。
        assert_eq!(space("𠀀A").as_deref(), Some("𠀀 A"));
    }

    #[test]
    fn chinese_english_spacing_never_adds_at_either_end() {
        let spaced = space("A中文B").unwrap();
        assert_eq!(spaced, "A 中文 B");
        assert!(spaced.starts_with('A') && spaced.ends_with('B'));
    }
}
