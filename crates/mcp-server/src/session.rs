//! `msime session` 的逐行 JSON 会话接口。
//!
//! 这是给 demo、回归脚本和编辑器集成用的完整 Host API 会话。它在本进程创建一个
//! 与宿主相同的 session，因此返回的 `view`、候选顺序和光标位置都来自同一条运行时链路。

use crate::config::Config;
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};

fn command(name: &str) -> Option<u32> {
    Some(match name {
        "backspace" => 0,
        "select" => 1,
        "commit" => 2,
        "cancel" => 3,
        "left" => 4,
        "right" => 5,
        "home" => 6,
        "end" => 7,
        "delete" => 8,
        "finish" => 9,
        "next-page" | "next_page" => 100,
        "previous-page" | "previous_page" => 101,
        "next-candidate" | "next_candidate" => 102,
        "previous-candidate" | "previous_candidate" => 103,
        "first-candidate" | "first_candidate" => 104,
        "last-candidate" | "last_candidate" => 105,
        "home-or-first" | "home_or_first" => 106,
        _ => return None,
    })
}

fn request(
    session: &msime_host_api::local_session::LocalSession,
    value: Value,
) -> Result<Value, String> {
    let object = value.as_object().ok_or("request must be a JSON object")?;
    let op = object
        .get("op")
        .and_then(Value::as_str)
        .ok_or("request needs a string op")?;
    match op {
        "focus" => {
            let focused = object
                .get("focused")
                .and_then(Value::as_bool)
                .unwrap_or(true);
            session.focus(focused)
        }
        "character" => {
            let text = object
                .get("value")
                .and_then(Value::as_str)
                .ok_or("character needs an ASCII value")?;
            let bytes = text.as_bytes();
            if bytes.len() != 1 || !bytes[0].is_ascii() {
                return Err("character needs exactly one ASCII byte".into());
            }
            let shift = object
                .get("shift")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            session.key(0);
            session.character(bytes[0], shift)
        }
        "text" => {
            let text = object
                .get("value")
                .and_then(Value::as_str)
                .ok_or("text needs a string value")?;
            let mut last = Value::Null;
            for byte in text.bytes() {
                if !byte.is_ascii() {
                    return Err("text accepts ASCII key bytes; use character for each key".into());
                }
                session.key(0);
                last = session.character(byte, false)?;
            }
            Ok(last)
        }
        "command" => {
            let name = object
                .get("value")
                .and_then(Value::as_str)
                .ok_or("command needs a name")?;
            let code = command(name).ok_or_else(|| format!("unknown command {name}"))?;
            session.key(0x80);
            session.command(code)
        }
        "select" => {
            let generation = object
                .get("generation")
                .and_then(Value::as_u64)
                .ok_or("select needs generation")?;
            let index = object
                .get("index")
                .and_then(Value::as_u64)
                .ok_or("select needs index")? as usize;
            session.key(0x80);
            session.select(generation, index)
        }
        _ => Err(format!("unknown operation {op}")),
    }
}

/// Runs until EOF. Every input line produces one JSON object, making this safe to pipe into
/// jq, Python or an editor integration without a second framing protocol.
pub fn run(config: Config) -> Result<std::process::ExitCode, String> {
    let options = config.read_host_options()?;
    let bytes = serde_json::to_vec(&options).map_err(|e| e.to_string())?;
    let session = msime_host_api::local_session::LocalSession::create(&bytes)?;
    let _ = session.focus(true);
    let stdin = io::stdin();
    let mut output = io::BufWriter::new(io::stdout().lock());
    let mut failed = false;
    for line in stdin.lock().lines() {
        let line = line.map_err(|_| "cannot read session input".to_owned())?;
        if line.trim().is_empty() {
            continue;
        }
        let result = serde_json::from_str::<Value>(&line)
            .map_err(|_| "request is not valid JSON".to_owned())
            .and_then(|value| request(&session, value));
        let output_value = match result {
            Ok(value) => json!({"ok": true, "value": value}),
            Err(error) => {
                failed = true;
                json!({"ok": false, "error": error})
            }
        };
        writeln!(output, "{}", output_value)
            .map_err(|_| "cannot write session output".to_owned())?;
    }
    output
        .flush()
        .map_err(|_| "cannot flush session output".to_owned())?;
    Ok(if failed {
        std::process::ExitCode::FAILURE
    } else {
        std::process::ExitCode::SUCCESS
    })
}
