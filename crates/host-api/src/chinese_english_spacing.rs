//! 「中英文之间加空格」：一次上屏的文本里，汉字与 ASCII 字母、数字相邻处插入一个半角空格，语义同 rime-ice 的 `cn_en_spacer.lua`（#7172）。两次上屏之间不加。

use msime_client_services::dictionary::is_han_character;
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

fn boundary(left: char, right: char) -> bool {
    (is_han_character(left) && right.is_ascii_alphanumeric())
        || (left.is_ascii_alphanumeric() && is_han_character(right))
}

/// 在 `text` 里汉字与 ASCII 字母、数字相邻的地方各插入一个半角空格；没有这样的相邻处时返回 `None`，原文不动，也不分配。
///
/// 已经隔着空格的不再插入，首尾也不加：上屏的第一个和最后一个字符保持不变，宿主的智能标点按它们判断前一个字符。汉字取 `is_han_character` 的范围（基本区、扩展 A 到 F 和兼容表意文字）；假名、谚文、全角字母和 ASCII 标点都不算。
pub(crate) fn space(text: &str) -> Option<String> {
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

#[cfg(test)]
mod cross_commit_tests {
    use super::*;

    #[test]
    fn cross_commit_boundary_covers_both_directions_without_trailing_space() {
        for (before, text, expected) in [
            ('用', "Rust", Some(" Rust")),
            ('t', "写", Some(" 写")),
            ('3', "人", Some(" 人")),
            ('𠀀', "2", Some(" 2")),
            (' ', "Rust", None),
            ('，', "Rust", None),
            ('用', "Ｒ", None),
            ('R', "ust", None),
            ('用', " Rust", None),
            ('用', "", None),
        ] {
            assert_eq!(cross_commit_space(text, Some(before)).as_deref(), expected);
        }
        assert_eq!(cross_commit_space("Rust", None), None);
    }
}

fn cross_commit_space(text: &str, preceding: Option<char>) -> Option<String> {
    let first = text.chars().next()?;
    let preceding = preceding?;
    boundary(preceding, first).then(|| format!(" {text}"))
}

#[cfg(test)]
mod plan_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn cross_commit_plan_respects_application_mode_and_privacy() {
        let mut request = json!({"config":{"enabled":true,"excluded_apps":[]},"application":"com.example.Writer","context":{"scheme":0,"local_mode":"none"},"text":"写","preceding":116});
        assert_eq!(cross_commit_plan(request.clone()).unwrap()["text"], " 写");
        for mode in [
            "none",
            "quick_phrase",
            "super_jianpin",
            "temporary_english",
            "direct_english",
        ] {
            request["context"]["local_mode"] = json!(mode);
            assert_eq!(
                cross_commit_plan(request.clone()).unwrap()["text"],
                " 写",
                "{mode}"
            );
        }
        for mode in [
            "url",
            "date_time",
            "unicode",
            "calculator",
            "command",
            "mention",
            "temporary_japanese",
            "unknown",
        ] {
            request["context"]["local_mode"] = json!(mode);
            assert_eq!(
                cross_commit_plan(request.clone()).unwrap()["text"],
                "写",
                "{mode}"
            );
        }
        request["context"]["local_mode"] = json!("none");
        request["context"]["plugin_row"] = json!(true);
        assert_eq!(
            cross_commit_plan(request.clone()).unwrap()["enabled"],
            false
        );
        request["context"]
            .as_object_mut()
            .unwrap()
            .remove("plugin_row");
        for field in ["secure", "full_width"] {
            request[field] = json!(true);
            assert_eq!(
                cross_commit_plan(request.clone()).unwrap()["enabled"],
                false
            );
            request.as_object_mut().unwrap().remove(field);
        }
        request["context"]["scheme"] = json!(3);
        assert_eq!(
            cross_commit_plan(request.clone()).unwrap()["enabled"],
            false
        );
        request["context"]["scheme"] = json!(0);
        request["config"]["excluded_apps"] = json!(["COM.EXAMPLE.WRITER"]);
        assert_eq!(
            cross_commit_plan(request.clone()).unwrap()["enabled"],
            false
        );
        request["config"]["excluded_apps"] = json!([]);
        request["config"]["enabled"] = json!(false);
        assert_eq!(
            cross_commit_plan(request.clone()).unwrap()["enabled"],
            false
        );
        request["config"]["enabled"] = json!(true);
        request["context"] = serde_json::Value::Null;
        assert_eq!(cross_commit_plan(request).unwrap()["enabled"], false);
    }
}

/// 纯策略请求：不创建引擎，不读取或保存文档。第一次可只询问 enabled，宿主获准后再读取一个前文标量。
pub(crate) fn cross_commit_plan(request: serde_json::Value) -> Result<serde_json::Value, String> {
    #[derive(serde::Deserialize)]
    struct Context {
        scheme: u8,
        local_mode: String,
        #[serde(default)]
        plugin_row: bool,
    }
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Request {
        config: msime_client_core::preferences::CrossCommitSpacing,
        application: String,
        context: Option<Context>,
        #[serde(default)]
        text: String,
        #[serde(default)]
        preceding: u32,
        #[serde(default)]
        secure: bool,
        #[serde(default)]
        full_width: bool,
    }
    let request: Request =
        serde_json::from_value(request).map_err(|_| "invalid spacing request")?;
    if !request.config.validate() {
        return Err("invalid spacing configuration".into());
    }
    let enabled = !request.secure
        && !request.full_width
        && request.config.enabled_for_application(&request.application)
        && request.context.is_some_and(|context| {
            !context.plugin_row
                && SchemeType::from_u8(context.scheme).is_some_and(SchemeType::is_chinese)
                && matches!(
                    context.local_mode.as_str(),
                    "none"
                        | "quick_phrase"
                        | "super_jianpin"
                        | "temporary_english"
                        | "direct_english"
                )
        });
    let text = if enabled {
        cross_commit_space(&request.text, char::from_u32(request.preceding)).unwrap_or(request.text)
    } else {
        request.text
    };
    Ok(serde_json::json!({"enabled":enabled,"text":text}))
}
