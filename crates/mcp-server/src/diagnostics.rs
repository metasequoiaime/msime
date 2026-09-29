//! The input method's diagnostic log as an agent may see it, so a user can describe a problem and have the agent read what the hosts recorded around it.
//!
//! The log is off until the user or an agent turns it on; `set` does that the way the settings page does, so an agent can go from the user's description of a problem to the log without the user touching a setting.
//!
//! Every desktop host writes the log beside its preferences when the user turns on `diagnostic_log.server` (or, on Windows, `diagnostic_log.tsf`): `diagnostic.log` on macOS and Linux, `logs\server.log` on Windows, each rotated to a `.1` copy once it grows past a few MiB. The hosts keep to event names, counts, timings and error codes, but the Windows TIP's key-trace records (`[msime][issue47]`) name the key that was pressed; those fields are blanked here before a line is returned, so what the user typed does not reach the agent.

use crate::preferences::{self, PreferencesChange};
use msime_client_core::preferences::PreferencesStore;
use rmcp::schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub const DEFAULT_LINES: usize = 200;
pub const MAX_LINES: usize = 2000;
/// The most read from the end of each file. The hosts rotate at 1 MiB (macOS, Linux) and 4 MiB (Windows), so this covers a whole file on every platform.
const READ_LIMIT: u64 = 8 << 20;
/// The fields of a Windows key-trace record that identify the key: its virtual-key code, the character it produced and that character printed.
const KEY_FIELDS: [&str; 3] = ["vk=", "wch=", "key="];

/// The local offset from UTC when the server started. On macOS and Linux it can only be read while the process has a single thread, so a change of offset while the server runs, such as the start of summer time, is not followed.
static LOCAL_OFFSET: OnceLock<time::UtcOffset> = OnceLock::new();

/// Read the local offset while it still can be; see `LOCAL_OFFSET`. Without it `now` is left out.
pub fn remember_local_offset() {
    if let Ok(offset) = time::UtcOffset::current_local_offset() {
        let _ = LOCAL_OFFSET.set(offset);
    }
}

/// The current local time as the hosts write it, `YYYY-MM-DD HH:MM:SS`.
fn local_now() -> Option<String> {
    let now = time::OffsetDateTime::now_utc().to_offset(*LOCAL_OFFSET.get()?);
    Some(format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        now.year(),
        u8::from(now.month()),
        now.day(),
        now.hour(),
        now.minute(),
        now.second()
    ))
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
#[serde(deny_unknown_fields)]
pub struct LogRequest {
    /// At most this many of the most recent matching lines, 1 to 2000. Defaults to 200.
    pub lines: Option<usize>,
    /// Only lines containing this, ignoring ASCII case, such as an event name, `error`, `slow` or `candidate`.
    pub contains: Option<String>,
    /// Only lines recorded at or after this local time, `YYYY-MM-DD HH:MM:SS` or any leading part of it such as `YYYY-MM-DD HH:MM`. Lines a Windows TSF batch carries without a time of their own go with the line before them.
    pub since: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
#[schemars(crate = "rmcp::schemars")]
pub struct LogView {
    /// Whether the input method is writing its log (`diagnostic_log_server` in the preferences). Nothing is recorded while it is off.
    pub server_enabled: bool,
    /// Whether the Windows TIP adds its composition and key-latency records (`diagnostic_log_tsf`). Windows only; always false elsewhere.
    pub tsf_enabled: bool,
    /// The current local time, in the form the log's lines start with, to turn "a few minutes ago" into `since`. Absent when the local time zone could not be read.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub now: Option<String>,
    /// The log's file name within the input method's data directory.
    pub file: String,
    /// The matching lines, oldest first, with the rotated copy's lines ahead of the current file's.
    pub lines: Vec<String>,
    /// More lines matched than were returned; ask for more lines or narrow with `contains` or `since`.
    pub has_more: bool,
    /// What to do when there is nothing to read.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
#[serde(deny_unknown_fields)]
pub struct SwitchRequest {
    /// True to start recording before the user repeats what went wrong, false to stop once the problem is understood.
    pub enabled: bool,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
#[schemars(crate = "rmcp::schemars")]
pub struct SwitchView {
    /// Whether the input method now writes its log.
    pub server_enabled: bool,
    /// Whether the Windows TIP now adds its records. Windows only; always false elsewhere.
    pub tsf_enabled: bool,
}

/// Turn the log on or off. On Windows the TIP's records go with it: problems with composing text show up there, and the agent should not need to know which half of the input method to ask.
pub fn set(state_dir: &Path, options: &Path, enabled: bool) -> Result<SwitchView, String> {
    let revision = PreferencesStore::new(state_dir)
        .load()
        .map_err(|error| error.to_string())?
        .revision;
    let change = PreferencesChange {
        expected_revision: revision,
        diagnostic_log_server: Some(enabled),
        diagnostic_log_tsf: cfg!(windows).then_some(enabled),
        ..PreferencesChange::default()
    };
    let view = preferences::update(state_dir, options, &change)?;
    Ok(SwitchView {
        server_enabled: view.diagnostic_log_server,
        tsf_enabled: cfg!(windows) && view.diagnostic_log_tsf,
    })
}

/// Where the host writes its log, relative to its preferences directory.
fn relative_path() -> PathBuf {
    if cfg!(windows) {
        Path::new("logs").join("server.log")
    } else {
        PathBuf::from("diagnostic.log")
    }
}

pub fn load(state_dir: &Path, request: &LogRequest) -> Result<LogView, String> {
    let limit = request.lines.unwrap_or(DEFAULT_LINES);
    if !(1..=MAX_LINES).contains(&limit) {
        return Err(format!("lines must be between 1 and {MAX_LINES}"));
    }
    if let Some(since) = &request.since {
        if !valid_since(since) {
            return Err("since must be a local time such as 2026-09-25 14:30:00".into());
        }
    }
    let switches = PreferencesStore::new(state_dir)
        .load()
        .map_err(|error| error.to_string())?
        .preferences
        .diagnostic_log;
    let relative = relative_path();
    let path = state_dir.join(&relative);
    let mut rotated = path.clone().into_os_string();
    rotated.push(".1");
    let mut found = false;
    let mut text = String::new();
    for file in [PathBuf::from(rotated), path] {
        if let Some(content) = read_tail(&file)? {
            found = true;
            text.push_str(&content);
            if !text.ends_with('\n') {
                text.push('\n');
            }
        }
    }
    let matching = select(&text, request);
    let has_more = matching.len() > limit;
    let lines = matching[matching.len().saturating_sub(limit)..]
        .iter()
        .map(|line| redact(line))
        .collect();
    let tsf_enabled = cfg!(windows) && switches.tsf;
    let hint = if !switches.server && !tsf_enabled {
        Some("The diagnostic log is off. Turn it on with set_diagnostic_log, ask the user to do again what went wrong, then read the log again.".to_owned())
    } else if !found {
        Some("The log is on but nothing has been written yet. Ask the user to do again what went wrong, then read the log again.".to_owned())
    } else {
        None
    };
    Ok(LogView {
        server_enabled: switches.server,
        tsf_enabled,
        now: local_now(),
        file: relative.to_string_lossy().into_owned(),
        lines,
        has_more,
        hint,
    })
}

/// The last `READ_LIMIT` bytes of `path` as text, from the first whole line; `None` when there is no such file.
fn read_tail(path: &Path) -> Result<Option<String>, String> {
    let mut file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err("cannot open the diagnostic log".into()),
    };
    let length = file
        .metadata()
        .map_err(|_| "cannot read the diagnostic log")?
        .len();
    let start = length.saturating_sub(READ_LIMIT);
    file.seek(SeekFrom::Start(start))
        .map_err(|_| "cannot read the diagnostic log")?;
    let mut bytes = Vec::with_capacity(usize::try_from(length - start).unwrap_or(0));
    file.take(READ_LIMIT)
        .read_to_end(&mut bytes)
        .map_err(|_| "cannot read the diagnostic log")?;
    let mut slice = &bytes[..];
    if start > 0 {
        // Started mid-line: the partial line is dropped rather than shown cut.
        let first = slice
            .iter()
            .position(|&byte| byte == b'\n')
            .map_or(slice.len(), |index| index + 1);
        slice = &slice[first..];
    }
    // The Windows Server opens each new file with a byte-order mark.
    let slice = slice.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(slice);
    Ok(Some(String::from_utf8_lossy(slice).into_owned()))
}

/// The non-empty lines of `text` that match `request`, oldest first.
fn select<'a>(text: &'a str, request: &LogRequest) -> Vec<&'a str> {
    let needle = request.contains.as_deref().map(str::to_ascii_lowercase);
    let mut in_window = request.since.is_none();
    text.lines()
        .map(|line| line.trim_end_matches('\r'))
        .filter(|line| !line.trim().is_empty())
        .filter(|line| {
            if let (Some(since), Some(time)) = (&request.since, timestamp(line)) {
                in_window = time >= since.as_str();
            }
            in_window
        })
        .filter(|line| {
            needle
                .as_deref()
                .is_none_or(|needle| line.to_ascii_lowercase().contains(needle))
        })
        .collect()
}

/// The `YYYY-MM-DD HH:MM:SS` a host line starts with. Windows adds milliseconds after it, which a comparison against a `since` of the same or shorter length ignores.
fn timestamp(line: &str) -> Option<&str> {
    let time = line.get(..19)?;
    let shape = time.bytes().enumerate().all(|(index, byte)| match index {
        4 | 7 => byte == b'-',
        10 => byte == b' ',
        13 | 16 => byte == b':',
        _ => byte.is_ascii_digit(),
    });
    shape.then_some(time)
}

/// Whether `since` is a leading part of `YYYY-MM-DD HH:MM:SS`, at least the date.
fn valid_since(since: &str) -> bool {
    const SHAPE: &[u8] = b"0000-00-00 00:00:00";
    (10..=SHAPE.len()).contains(&since.len())
        && since.bytes().zip(SHAPE).all(|(byte, &expected)| {
            if expected == b'0' {
                byte.is_ascii_digit()
            } else {
                byte == expected
            }
        })
}

/// Blank the fields that name a pressed key. They only appear in the Windows TIP's key-trace records, which separate fields with single spaces.
fn redact(line: &str) -> String {
    line.split(' ')
        .map(|field| {
            KEY_FIELDS
                .iter()
                .find(|name| field.starts_with(*name))
                .map_or_else(|| field.to_owned(), |name| format!("{name}-"))
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(lines: Option<usize>, contains: Option<&str>, since: Option<&str>) -> LogRequest {
        LogRequest {
            lines,
            contains: contains.map(Into::into),
            since: since.map(Into::into),
        }
    }

    #[test]
    fn a_key_trace_keeps_its_shape_but_not_its_key() {
        let line = "2026-09-25 10:00:00.123 [p1:t2] [msime][issue47] seq=3 stage=key-down request=9 vk=0x41 wch=U+0061 key=a key_class=letter category=0 composing=1 buffer_len=1 result=0x00000000 process=notepad.exe";
        assert_eq!(
            redact(line),
            "2026-09-25 10:00:00.123 [p1:t2] [msime][issue47] seq=3 stage=key-down request=9 vk=- wch=- key=- key_class=letter category=0 composing=1 buffer_len=1 result=0x00000000 process=notepad.exe"
        );
        let plain = "2026-09-25 10:00:00 [p7] focus_in";
        assert_eq!(redact(plain), plain);
    }

    #[test]
    fn lines_are_filtered_by_time_and_text_and_the_newest_are_kept() {
        let text = "2026-09-25 09:59:59 [p1] focus_in\n\
                    2026-09-25 10:00:00.001 [p1:t1] TSF diagnostics pid=5 records=2\r\n\
                    [msime][key-latency] side=tsf stage=send elapsed_ms=12.000\r\n\
                    [msime][composition-recovery] reason=lost\r\n\
                    \n\
                    2026-09-25 10:00:05 [p1] candidate_window_slow\n\
                    2026-09-25 10:01:00 [p1] focus_out\n";
        assert_eq!(
            select(text, &request(None, None, Some("2026-09-25 10:00"))),
            [
                "2026-09-25 10:00:00.001 [p1:t1] TSF diagnostics pid=5 records=2",
                "[msime][key-latency] side=tsf stage=send elapsed_ms=12.000",
                "[msime][composition-recovery] reason=lost",
                "2026-09-25 10:00:05 [p1] candidate_window_slow",
                "2026-09-25 10:01:00 [p1] focus_out",
            ]
        );
        assert_eq!(
            select(text, &request(None, Some("FOCUS"), None)),
            [
                "2026-09-25 09:59:59 [p1] focus_in",
                "2026-09-25 10:01:00 [p1] focus_out"
            ]
        );
        assert!(valid_since("2026-09-25"));
        assert!(valid_since("2026-09-25 10:00:00"));
        assert!(!valid_since("2026-09"));
        assert!(!valid_since("yesterday"));
        assert!(!valid_since("2026-09-25 10:00:00.000"));
    }

    #[test]
    fn the_rotated_copy_comes_first_and_the_log_state_is_reported() {
        let directory = tempfile::tempdir().unwrap();
        let state = directory.path();
        let path = state.join(relative_path());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();

        let view = load(state, &LogRequest::default()).unwrap();
        assert!(!view.server_enabled && view.lines.is_empty() && !view.has_more);
        assert!(view.hint.unwrap().contains("off"));

        let store = PreferencesStore::new(state);
        let mut preferences = store.load().unwrap().preferences;
        preferences.diagnostic_log.server = true;
        store.save(0, preferences).unwrap();
        let view = load(state, &LogRequest::default()).unwrap();
        assert!(view.server_enabled);
        assert!(view.hint.unwrap().contains("nothing has been written"));

        let mut rotated = path.clone().into_os_string();
        rotated.push(".1");
        std::fs::write(&rotated, "2026-09-25 09:00:00 [p1] old").unwrap();
        std::fs::write(
            &path,
            b"\xEF\xBB\xBF2026-09-25 10:00:00 [p1] new_one\n2026-09-25 10:00:01 [p1] new_two\n",
        )
        .unwrap();
        let view = load(state, &LogRequest::default()).unwrap();
        assert_eq!(
            view.lines,
            [
                "2026-09-25 09:00:00 [p1] old",
                "2026-09-25 10:00:00 [p1] new_one",
                "2026-09-25 10:00:01 [p1] new_two"
            ]
        );
        assert_eq!(view.hint, None);
        let view = load(state, &request(Some(2), None, None)).unwrap();
        assert!(view.has_more);
        assert_eq!(view.lines[0], "2026-09-25 10:00:00 [p1] new_one");

        assert!(load(state, &request(Some(0), None, None)).is_err());
        assert!(load(state, &request(Some(MAX_LINES + 1), None, None)).is_err());
        assert!(load(state, &request(None, None, Some("now"))).is_err());
    }

    #[test]
    fn a_long_log_is_read_from_its_last_whole_line() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("diagnostic.log");
        let line = "2026-09-25 10:00:00 [p1] padding_padding_padding_padding\n";
        let count = (READ_LIMIT as usize / line.len()) + 10;
        std::fs::write(&path, line.repeat(count)).unwrap();
        let text = read_tail(&path).unwrap().unwrap();
        assert!(text.len() as u64 <= READ_LIMIT);
        assert!(text.lines().all(|read| read == line.trim_end()));
        assert_eq!(
            read_tail(&directory.path().join("absent.log")).unwrap(),
            None
        );
    }
}
