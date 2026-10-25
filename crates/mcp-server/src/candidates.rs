//! `candidates`：一串拼音或按键在用户现在的设置下会给出哪些候选，给不改输入法源码、写测试和演示程序的人用（#9663）。
//!
//! 和 `expand` 不同，它不经 MCP 工具，也不接受 `--scheme`：候选由 host-api 的 `candidate_preview` 给出，那是桌面宿主建会话、打字的同一条路，方案、模糊音、快捷短语、插件、学过的词频和用户词都按用户自己的设置来。每一次查询都现读运行时选项和偏好，设置改了下一行就生效。
//!
//! 候选来自用户的词库，所以要用户在设置的「维护与诊断」里打开「命令行查询候选」（`diagnostic_log.command_line_candidates`），每次查询前都检查，关掉之后流式读 stdin 的进程也在下一行停下。这守的是用户的意图，不是安全边界：能以用户身份运行这个命令的程序本来就读得到词库文件。为什么在本进程里建会话而不连到正在运行的输入法，见 `.agents/notes/implemented/feature/2026-10-25-command-line-candidates.md`。
//!
//! stdout 每次查询输出一行 JSON：成功是 [`CandidatePreview`]，失败是 `{"keys": …, "error": {"code": …, "message": …}}`；给人看的原因同时写到 stderr。

use crate::config::{program, Config};
use msime_host_api::candidate_preview::{
    preview_candidates, CandidatePreview, PreviewError, MAX_PREVIEW_CANDIDATES,
};
use serde_json::{json, Value};
use std::io::{BufRead, IsTerminal, Write};
use std::process::ExitCode;

/// 不给 `--limit` 时最多列多少个候选。
pub const DEFAULT_LIMIT: usize = 50;
/// stdin 一行最长多少字节；再长的一行整行拒绝。
const LINE_LIMIT: usize = 4096;
/// 光标标记。它不是哪个方案的按键，写在键里不会和要打的键混淆。
const CARET: char = '|';

/// 查不了的几种原因，每种有自己的退出码和 JSON 里的 `code`。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    /// 用户没在设置里打开命令行查询候选。
    Disabled,
    /// 读不到输入法的运行时选项或偏好：输入法没装好、没在这个用户下设置过，或者文件坏了。
    NotSetUp,
    /// 词库正在维护（导入、恢复、换代次），稍后再试。
    Busy,
    /// 词库打不开，会话建不起来。
    Unavailable,
    /// 按键、光标或候选数不合规，或者输入法不收其中某个键。
    InvalidInput,
}

impl Failure {
    pub fn code(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::NotSetUp => "not_set_up",
            Self::Busy => "busy",
            Self::Unavailable => "unavailable",
            Self::InvalidInput => "invalid_input",
        }
    }

    /// 进程的退出码。1 留给意料之外的失败，2 是命令行写错了，与其他命令相同。
    pub fn exit_code(self) -> u8 {
        match self {
            Self::Disabled => 3,
            Self::NotSetUp => 4,
            Self::Busy => 5,
            Self::Unavailable => 6,
            Self::InvalidInput => 7,
        }
    }

    /// 流式读 stdin 时，这种失败之后再读下一行也查不了：功能被关掉、或者输入法的状态读不到了。
    fn ends_the_stream(self) -> bool {
        matches!(self, Self::Disabled | Self::NotSetUp)
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Refusal {
    failure: Failure,
    message: String,
}

impl Refusal {
    fn new(failure: Failure, message: impl Into<String>) -> Self {
        Self {
            failure,
            message: message.into(),
        }
    }
}

/// 把 `ni|hao` 拆成要打的键和光标位置（它前面有几个键）；没有 `|` 时光标在末尾。
pub fn parse_keys(text: &str) -> Result<(String, usize), String> {
    let mut parts = text.split(CARET);
    let before = parts.next().unwrap_or_default();
    match (parts.next(), parts.next()) {
        (None, _) => Ok((text.to_owned(), text.len())),
        (Some(after), None) => Ok((format!("{before}{after}"), before.len())),
        (Some(_), Some(_)) => Err(format!("mark the caret with one {CARET} at most")),
    }
}

/// 偏好里用户有没有打开命令行查询候选。读不到这一项就是没打开。
fn enabled(document: &Value) -> bool {
    document["preferences"]["diagnostic_log"]["command_line_candidates"] == true
}

fn disabled_message() -> String {
    "candidates is turned off. To use it, turn on 命令行查询候选 under 维护与诊断 in the 水杉输入法 settings; it lets programs on this computer read the words in your dictionary.".into()
}

/// 查一串键。每次都现读运行时选项和偏好：设置页刚改的方案、刚关掉的开关，下一次查询就看得到。
fn query(config: &Config, text: &str, limit: usize) -> Result<CandidatePreview, Refusal> {
    let document = config
        .read_host_options()
        .map_err(|message| Refusal::new(Failure::NotSetUp, message))?;
    config
        .edition(&document)
        .map_err(|message| Refusal::new(Failure::NotSetUp, message))?;
    if !enabled(&document) {
        return Err(Refusal::new(Failure::Disabled, disabled_message()));
    }
    let (keys, caret) =
        parse_keys(text).map_err(|message| Refusal::new(Failure::InvalidInput, message))?;
    preview_candidates(&document, &keys, caret, limit).map_err(|error| {
        let failure = match error {
            PreviewError::Options(_) => Failure::NotSetUp,
            PreviewError::Busy => Failure::Busy,
            PreviewError::Unavailable(_) => Failure::Unavailable,
            PreviewError::Input(_) => Failure::InvalidInput,
        };
        Refusal::new(failure, error.to_string())
    })
}

/// 一次查询在 stdout 上的那一行。
fn answer_line(text: &str, result: &Result<CandidatePreview, Refusal>) -> String {
    match result {
        Ok(preview) => serde_json::to_string(preview).unwrap_or_else(|error| {
            json!({ "keys": text, "error": { "code": "internal", "message": error.to_string() } })
                .to_string()
        }),
        Err(refusal) => json!({
            "keys": text,
            "error": { "code": refusal.failure.code(), "message": refusal.message },
        })
        .to_string(),
    }
}

/// 写一行并立刻刷出去：流式调用方读到这一行才会送下一行。
fn emit(line: &str) -> std::io::Result<()> {
    let mut stdout = std::io::stdout().lock();
    writeln!(stdout, "{line}")?;
    stdout.flush()
}

/// 运行 `candidates`。`keys` 为 `None` 时从 stdin 逐行读。
pub fn run(config: &Config, keys: Option<String>, limit: Option<u64>) -> ExitCode {
    let limit = match limit {
        None => DEFAULT_LIMIT,
        Some(limit) => match usize::try_from(limit) {
            Ok(limit @ 1..=MAX_PREVIEW_CANDIDATES) => limit,
            _ => {
                eprintln!(
                    "{}: --limit must be between 1 and {MAX_PREVIEW_CANDIDATES}",
                    program()
                );
                return ExitCode::from(2);
            }
        },
    };
    match keys {
        Some(text) => {
            let result = query(config, &text, limit);
            if emit(&answer_line(&text, &result)).is_err() {
                return ExitCode::FAILURE;
            }
            match result {
                Ok(_) => ExitCode::SUCCESS,
                Err(refusal) => {
                    eprintln!("{}: {}", program(), refusal.message);
                    ExitCode::from(refusal.failure.exit_code())
                }
            }
        }
        None => stream(config, limit),
    }
}

/// 从 stdin 逐行读键，每行输出一行。查不了的一行输出错误行，接着读下一行；功能被关掉或读不到输入法的状态时停下，以那种失败的退出码退出。读完 stdin 以 0 退出。
fn stream(config: &Config, limit: usize) -> ExitCode {
    let stdin = std::io::stdin();
    if stdin.is_terminal() {
        eprintln!(
            "{}: type keys, one line each, such as nihao or ni|hao; end the input to stop",
            program()
        );
    }
    let mut reader = stdin.lock();
    loop {
        let line = match next_line(&mut reader) {
            Ok(Some(line)) => line,
            Ok(None) => return ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("{}: cannot read stdin: {error}", program());
                return ExitCode::FAILURE;
            }
        };
        let (text, result) = match line {
            Ok(text) => {
                let text = text.trim().to_owned();
                let result = query(config, &text, limit);
                (text, result)
            }
            Err(reason) => (
                String::new(),
                Err(Refusal::new(Failure::InvalidInput, reason)),
            ),
        };
        if emit(&answer_line(&text, &result)).is_err() {
            // 读这一端的程序已经走了。
            return ExitCode::FAILURE;
        }
        if let Err(refusal) = result {
            eprintln!("{}: {text}: {}", program(), refusal.message);
            if refusal.failure.ends_the_stream() {
                return ExitCode::from(refusal.failure.exit_code());
            }
        }
    }
}

/// 读下一行，去掉行尾的 `\n` 和 `\r\n`。读完时为 `None`；一行超过 [`LINE_LIMIT`] 字节或不是 UTF-8 时整行跳过，交回原因。
fn next_line(reader: &mut impl BufRead) -> std::io::Result<Option<Result<String, &'static str>>> {
    let mut bytes = Vec::new();
    let mut too_long = false;
    let mut read_any = false;
    loop {
        let buffer = reader.fill_buf()?;
        if buffer.is_empty() {
            if !read_any {
                return Ok(None);
            }
            break;
        }
        read_any = true;
        let (taken, ends) = match buffer.iter().position(|&byte| byte == b'\n') {
            Some(end) => (end, true),
            None => (buffer.len(), false),
        };
        if !too_long {
            if bytes.len() + taken > LINE_LIMIT {
                too_long = true;
                bytes.clear();
            } else {
                bytes.extend_from_slice(&buffer[..taken]);
            }
        }
        reader.consume(if ends { taken + 1 } else { taken });
        if ends {
            break;
        }
    }
    if too_long {
        return Ok(Some(Err("the line is too long")));
    }
    if bytes.last() == Some(&b'\r') {
        bytes.pop();
    }
    Ok(Some(
        String::from_utf8(bytes).map_err(|_| "the line is not UTF-8"),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bar_marks_the_caret_and_the_end_is_the_default() {
        assert_eq!(parse_keys("nihao"), Ok(("nihao".into(), 5)));
        assert_eq!(parse_keys("ni|hao"), Ok(("nihao".into(), 2)));
        assert_eq!(parse_keys("|nihao"), Ok(("nihao".into(), 0)));
        assert_eq!(parse_keys("nihao|"), Ok(("nihao".into(), 5)));
        assert!(parse_keys("ni|ha|o").is_err());
    }

    #[test]
    fn every_failure_has_its_own_exit_code_and_name() {
        let all = [
            Failure::Disabled,
            Failure::NotSetUp,
            Failure::Busy,
            Failure::Unavailable,
            Failure::InvalidInput,
        ];
        let mut codes: Vec<u8> = all.iter().map(|failure| failure.exit_code()).collect();
        codes.sort_unstable();
        codes.dedup();
        assert_eq!(codes.len(), all.len());
        // 0 成功，1 意料之外的失败，2 命令行写错，都不能和这几种重。
        assert!(codes.iter().all(|code| *code > 2));
        let mut names: Vec<&str> = all.iter().map(|failure| failure.code()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), all.len());
        // 只有关掉开关和读不到状态才让流式读取停下。
        assert!(Failure::Disabled.ends_the_stream() && Failure::NotSetUp.ends_the_stream());
        assert!(!Failure::Busy.ends_the_stream() && !Failure::InvalidInput.ends_the_stream());
    }

    #[test]
    fn only_an_explicit_true_turns_the_command_on() {
        assert!(!enabled(&json!({})));
        assert!(!enabled(
            &json!({ "preferences": { "diagnostic_log": { "server": true } } })
        ));
        assert!(!enabled(
            &json!({ "preferences": { "diagnostic_log": { "command_line_candidates": "true" } } })
        ));
        assert!(enabled(
            &json!({ "preferences": { "diagnostic_log": { "command_line_candidates": true } } })
        ));
    }

    #[test]
    fn an_error_line_names_the_keys_and_the_failure() {
        let line = answer_line(
            "ni hao",
            &Err(Refusal::new(Failure::InvalidInput, "key 3 is a space")),
        );
        let value: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(
            value,
            json!({ "keys": "ni hao", "error": { "code": "invalid_input", "message": "key 3 is a space" } })
        );
        assert!(!line.contains('\n'));
    }

    #[test]
    fn lines_are_read_one_at_a_time_and_bounded() {
        let long = "a".repeat(LINE_LIMIT + 1);
        let input = format!("nihao\r\nni|hao\n\n{long}\nzai\n\u{ff}x\nlast");
        let mut bytes = input.into_bytes();
        // 一行不是 UTF-8。
        let invalid = bytes
            .windows(2)
            .position(|pair| pair == "\u{ff}".as_bytes())
            .unwrap();
        bytes.splice(invalid..invalid + 2, [0xff]);
        // 小缓冲区，逼着一行跨好几次 `fill_buf`。
        let mut reader = std::io::BufReader::with_capacity(7, bytes.as_slice());
        let mut lines = Vec::new();
        while let Some(line) = next_line(&mut reader).unwrap() {
            lines.push(line);
        }
        assert_eq!(
            lines,
            [
                Ok("nihao".to_owned()),
                Ok("ni|hao".to_owned()),
                Ok(String::new()),
                Err("the line is too long"),
                Ok("zai".to_owned()),
                Err("the line is not UTF-8"),
                Ok("last".to_owned()),
            ]
        );
    }
}
