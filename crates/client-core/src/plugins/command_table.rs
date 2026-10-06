//! `kind = "command_table"`: `/` commands, each a trigger, a title and a template.
//!
//! ```toml
//! [[commands]]
//! trigger = "sig"
//! title = "签名"
//! template = "{date:%Y-%m-%d} 张三"
//! ```
//!
//! A template is literal text and three placeholders: `{date}` or `{date:FORMAT}`, `{time}` or `{time:FORMAT}`, and `{weekday}`, with FORMAT a strftime description. Nothing else - no clipboard, no environment, no nesting - so a table can only ever produce text. The Engine expands templates (`crates/engine/src/local/command.rs`) and silently drops a row it cannot use; the rules are repeated here, where client-core cannot reach the Engine, so a pack is refused with the reason instead of losing rows nobody is told about.

use serde::Serialize;
use time::format_description::parse_strftime_borrowed;
use time::{Date, Month, PrimitiveDateTime, Time};
use toml::Value;

use super::{only_keys, PluginKind};

pub(crate) const MANIFEST_KEYS: [&str; 1] = ["commands"];

/// Commands in one pack, and in every enabled pack together: the Engine keeps no more.
pub const MAX_COMMANDS: usize = 256;
/// Trigger letters.
pub const MAX_TRIGGER_BYTES: usize = 32;
/// Title shown beside a row, in bytes.
pub const MAX_TITLE_BYTES: usize = 48;
/// A template, and the text it expands to, in UTF-16 units: the Windows candidate pipe's text field, the Engine's `TEXT_UTF16_LIMIT`.
pub const MAX_TEXT_UTF16: usize = 199;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CommandRow {
    /// Lowercase ASCII letters typed after `/`.
    pub trigger: String,
    pub title: String,
    pub template: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CommandTable {
    pub commands: Vec<CommandRow>,
}

pub(crate) fn parse(table: &toml::map::Map<String, Value>) -> Result<CommandTable, String> {
    let items = table
        .get("commands")
        .and_then(Value::as_array)
        .ok_or("指令表缺少 commands")?;
    if items.is_empty() || items.len() > MAX_COMMANDS {
        return Err("指令表的指令条数不在允许范围内".into());
    }
    let mut commands: Vec<CommandRow> = Vec::with_capacity(items.len());
    for item in items {
        let row = item.as_table().ok_or("每条指令都必须是一个表")?;
        only_keys(row, &["trigger", "title", "template"], "a command")?;
        let field = |key: &str| {
            row.get(key)
                .and_then(Value::as_str)
                .map(str::to_owned)
                .ok_or_else(|| format!("每条指令都需要 {key}"))
        };
        let command = CommandRow {
            trigger: field("trigger")?,
            title: field("title")?,
            template: field("template")?,
        };
        validate(&command)?;
        if commands.iter().any(|kept| kept.trigger == command.trigger) {
            return Err(format!("指令 {} 重复了", command.trigger));
        }
        commands.push(command);
    }
    Ok(CommandTable { commands })
}

/// Why the Engine could not use `command`, if it could not.
pub fn validate(command: &CommandRow) -> Result<(), String> {
    let trigger = &command.trigger;
    if trigger.is_empty()
        || trigger.len() > MAX_TRIGGER_BYTES
        || !trigger.bytes().all(|byte| byte.is_ascii_lowercase())
    {
        return Err(format!("指令 {trigger} 必须是 1 到 32 个小写字母"));
    }
    if command.title.trim().is_empty()
        || !crate::text::is_bounded_text(&command.title, MAX_TITLE_BYTES)
    {
        return Err(format!("指令 {trigger} 的标题为空或太长"));
    }
    let template = &command.template;
    if template.trim().is_empty() || !crate::text::is_bounded_utf16(template, MAX_TEXT_UTF16) {
        return Err(format!("指令 {trigger} 的模板为空或太长"));
    }
    // A candidate row shows one line, so a newline or tab would reach the application unseen: a line break in a terminal runs whatever follows it. `%n` and `%t` expand to them, so the expansion is checked too.
    if crate::text::has_disallowed_control_with_allowed(template, &[]) {
        return Err(format!("指令 {trigger} 的模板含有换行、制表符等控制字符"));
    }
    let expanded =
        expand_longest(template).ok_or_else(|| format!("指令 {trigger} 的模板有不认识的占位符"))?;
    if crate::text::has_disallowed_control_with_allowed(&expanded, &[]) {
        return Err(format!("指令 {trigger} 的模板含有换行、制表符等控制字符"));
    }
    if !crate::text::is_bounded_utf16(&expanded, MAX_TEXT_UTF16) {
        return Err(format!("指令 {trigger} 的模板展开后太长"));
    }
    Ok(())
}

/// The template expanded at the widest of two instants - both a Wednesday with a two-digit day and hour, one in September for the longest English names, one in December for the two-digit month an unpadded `%-m` gives - so a template that fits here fits on every day. `None` when a placeholder is not one of the three or its format does not parse.
fn expand_longest(template: &str) -> Option<String> {
    let time = Time::from_hms(23, 59, 59).ok()?;
    let september = expand_at(
        template,
        PrimitiveDateTime::new(
            Date::from_calendar_date(2026, Month::September, 30).ok()?,
            time,
        ),
    )?;
    let december = expand_at(
        template,
        PrimitiveDateTime::new(
            Date::from_calendar_date(2026, Month::December, 30).ok()?,
            time,
        ),
    )?;
    let utf16 = |text: &str| text.encode_utf16().count();
    Some(if utf16(&december) > utf16(&september) {
        december
    } else {
        september
    })
}

fn expand_at(template: &str, instant: PrimitiveDateTime) -> Option<String> {
    let mut output = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(open) = rest.find(['{', '}']) {
        if rest.as_bytes()[open] == b'}' {
            return None;
        }
        output.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let close = after.find(['{', '}'])?;
        if after.as_bytes()[close] == b'{' {
            return None;
        }
        let placeholder = &after[..close];
        rest = &after[close + 1..];
        if placeholder == "weekday" {
            output.push_str("星期三");
            continue;
        }
        let format = match placeholder.split_once(':').unwrap_or((placeholder, "")) {
            ("date", "") => "%Y-%m-%d",
            ("time", "") => "%H:%M",
            ("date" | "time", format) => format,
            _ => return None,
        };
        let items = parse_strftime_borrowed(format).ok()?;
        output.push_str(&instant.format(&items).ok()?);
    }
    output.push_str(rest);
    Some(output)
}

/// The rows of the enabled command-table packs under `root`, in the order the ids are listed, keeping the first command of each trigger and at most `MAX_COMMANDS`: the table a host hands to the Engine. A pack that is missing or does not load contributes nothing; the settings page reports it.
pub fn enabled_commands(root: &std::path::Path, enabled: &[String]) -> Vec<CommandRow> {
    let mut rows: Vec<CommandRow> = Vec::with_capacity(MAX_COMMANDS);
    for id in enabled {
        let Ok(package) = super::load_package(root, None, PluginKind::CommandTable, id) else {
            continue;
        };
        let super::PluginContent::CommandTable(table) = package.content else {
            continue;
        };
        for row in table.commands {
            if rows.len() == MAX_COMMANDS {
                return rows;
            }
            if !rows.iter().any(|kept| kept.trigger == row.trigger) {
                rows.push(row);
            }
        }
    }
    rows
}
