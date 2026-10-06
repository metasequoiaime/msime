//! The tools and prompts from a shell, one per run, for an assistant that drives a terminal rather than speaking MCP.
//!
//! The command line does not reimplement any tool: it starts the same server in process and calls it over an in-memory pipe as an MCP client would, so the tools offered, their arguments, their limits and their answers are the server's own.

use crate::config::Config;
use crate::server::MsimeServer;
use rmcp::model::{CallToolRequestParams, CallToolResult, GetPromptRequestParams};
use rmcp::service::RunningService;
use rmcp::{RoleClient, ServiceExt};
use serde_json::{Map, Value};
use std::process::ExitCode;

/// Large enough for one skin's images to cross without stalling on every chunk.
const PIPE_CAPACITY: usize = 1 << 20;

pub enum Action {
    Tools,
    Call {
        tool: String,
        arguments: Map<String, Value>,
    },
    Prompts,
    Prompt {
        name: String,
        arguments: Map<String, Value>,
    },
    /// `expand`：每串按键各调一次 lookup_candidates，`arguments` 是除编码以外的参数。`batch` 时每串的输出前标出编码，`--json` 则每串输出一行 JSON。
    Expand {
        codes: Vec<String>,
        arguments: Map<String, Value>,
        json: bool,
        batch: bool,
    },
    ShowConfig {
        json: bool,
    },
    GetConfig {
        keys: Vec<String>,
        json: bool,
    },
    /// `config set`：`changes` 是 update_preferences 的参数，revision 由这里现读现填。
    SetConfig {
        changes: Map<String, Value>,
        json: bool,
    },
}

/// Run `action` against a server built from `config`: the result goes to stdout as JSON, a refusal to stderr.
pub async fn run(config: Config, action: Action) -> Result<ExitCode, Box<dyn std::error::Error>> {
    let (server_io, client_io) = tokio::io::duplex(PIPE_CAPACITY);
    let server = tokio::spawn(async move {
        MsimeServer::new(config)
            .serve(server_io)
            .await?
            .waiting()
            .await?;
        Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
    });
    let client = ().serve(client_io).await?;
    let code = match action {
        Action::Tools => {
            let tools = client.list_all_tools().await?;
            println!("{}", serde_json::to_string_pretty(&tools)?);
            ExitCode::SUCCESS
        }
        Action::Call { tool, arguments } => match call(&client, tool, arguments).await? {
            Ok(result) => {
                match &result.structured_content {
                    Some(value) => println!("{}", serde_json::to_string_pretty(value)?),
                    None => println!("{}", text(&result)),
                }
                ExitCode::SUCCESS
            }
            Err(refusal) => refused(&refusal),
        },
        Action::Expand {
            codes,
            arguments,
            json,
            batch,
        } => {
            let program = crate::config::program();
            // 一串查不了不影响其余各串，全部查完再以 1 退出。
            let mut failed = false;
            for code in codes {
                let mut arguments = arguments.clone();
                arguments.insert("code".into(), code.clone().into());
                let result =
                    structured(call(&client, "lookup_candidates".into(), arguments).await?)?;
                if batch && !json {
                    println!("# {code}");
                }
                match result {
                    Ok(value) if json && batch => println!(
                        "{}",
                        serde_json::json!({ "code": code, "candidates": value["candidates"] })
                    ),
                    Ok(value) if json => {
                        print_json(&value)?;
                    }
                    Ok(value) => {
                        let lines = candidate_lines(&value);
                        // stdout 留给候选本身；一串时空输出容易被当成命令出了问题，在 stderr 说一声。
                        if lines.is_empty() && !batch {
                            eprintln!("{program}: {code} offers no candidates");
                        }
                        print_lines(lines);
                    }
                    Err(refusal) => {
                        failed = true;
                        if json && batch {
                            println!("{}", serde_json::json!({ "code": code, "error": refusal }));
                        }
                        eprintln!("{program}: {code}: {refusal}");
                    }
                }
            }
            if failed {
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            }
        }
        Action::GetConfig { keys, json } => {
            match structured(call(&client, "get_preferences".into(), Map::new()).await?)? {
                Ok(value) => match picked_preferences(&value, &keys) {
                    Ok(picked) if json => print_json(&Value::Object(picked.into_iter().collect()))?,
                    Ok(picked) => {
                        print_lines(picked.iter().map(|(_, value)| plain(value)).collect())
                    }
                    Err(missing) => refused(&format!(
                        "no preference named {missing}; `{} config` lists them",
                        crate::config::program()
                    )),
                },
                Err(refusal) => refused(&refusal),
            }
        }
        Action::ShowConfig { json } => {
            match structured(call(&client, "get_preferences".into(), Map::new()).await?)? {
                Ok(value) if json => print_json(&value)?,
                Ok(value) => print_lines(preference_lines(&value)),
                Err(refusal) => refused(&refusal),
            }
        }
        Action::SetConfig { mut changes, json } => {
            match structured(call(&client, "get_preferences".into(), Map::new()).await?)? {
                Ok(current) => {
                    changes.insert("expected_revision".into(), current["revision"].clone());
                    match structured(call(&client, "update_preferences".into(), changes).await?)? {
                        Ok(value) if json => print_json(&value)?,
                        Ok(value) => print_lines(preference_lines(&value)),
                        Err(refusal) => refused(&refusal),
                    }
                }
                Err(refusal) => refused(&refusal),
            }
        }
        Action::Prompts => {
            let prompts = client.list_all_prompts().await?;
            println!("{}", serde_json::to_string_pretty(&prompts)?);
            ExitCode::SUCCESS
        }
        Action::Prompt { name, arguments } => {
            // A prompt the flags do not offer is the likeliest refusal, as for a tool.
            let prompt = client
                .get_prompt(GetPromptRequestParams::new(name).with_arguments(arguments))
                .await
                .map_err(|error| {
                    format!("{error}; `prompts` lists what these flags offer: make_skin needs --allow-write")
                })?;
            // The instructions are written for an assistant that calls the tools over MCP; from a shell each tool is one `call`.
            for message in &prompt.messages {
                if let Some(text) = message.content.as_text() {
                    println!("{}", text.text);
                }
            }
            println!("\nFrom a shell, run each tool this names as `{} <the same flags> call <tool> '<json arguments>'`.", crate::config::program());
            ExitCode::SUCCESS
        }
    };
    client.cancel().await?;
    server.await?.map_err(|error| error.to_string())?;
    Ok(code)
}

/// 调用一个工具。外层的错误是调用没能进行，多半是这组开关没有提供这个工具；内层的 `Err` 是工具拒绝时给出的原因。
async fn call(
    client: &RunningService<RoleClient, ()>,
    tool: String,
    arguments: Map<String, Value>,
) -> Result<Result<CallToolResult, String>, Box<dyn std::error::Error>> {
    // A tool the flags do not offer is the likeliest refusal here, and the server's own answer does not say why it is missing.
    let result = client
        .call_tool(CallToolRequestParams::new(tool).with_arguments(arguments))
        .await
        .map_err(|error| {
            format!("{error}; `tools` lists what these flags offer: writing needs --allow-write, dictionary words --allow-dictionary-read")
        })?;
    Ok(if result.is_error == Some(true) {
        Err(text(&result))
    } else {
        Ok(result)
    })
}

/// 结果里的文本块，拒绝时就是原因。
fn text(result: &CallToolResult) -> String {
    result
        .content
        .iter()
        .filter_map(|block| block.as_text().map(|text| text.text.as_str()))
        .collect::<Vec<_>>()
        .join("\n")
}

/// expand 和 config 用到的工具都返回结构化结果；没有就是服务器的回答不对，按调用失败处理。
fn structured(
    result: Result<CallToolResult, String>,
) -> Result<Result<Value, String>, Box<dyn std::error::Error>> {
    match result {
        Ok(result) => Ok(Ok(result
            .structured_content
            .ok_or("the tool returned no structured result")?)),
        Err(refusal) => Ok(Err(refusal)),
    }
}

fn refused(reason: &str) -> ExitCode {
    eprintln!("{}: {reason}", crate::config::program());
    ExitCode::FAILURE
}

fn print_json(value: &Value) -> Result<ExitCode, serde_json::Error> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(ExitCode::SUCCESS)
}

fn print_lines(lines: Vec<String>) -> ExitCode {
    for line in lines {
        println!("{line}");
    }
    ExitCode::SUCCESS
}

/// 每个候选一行，制表符分隔：名次、文字、编码、来源，词库词再加权重。便于 diff、grep 和 cut。
fn candidate_lines(view: &Value) -> Vec<String> {
    let field = |candidate: &Value, name: &str| match &candidate[name] {
        Value::String(text) => text.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    };
    view["candidates"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .enumerate()
        .map(|(index, candidate)| {
            let mut line = format!(
                "{}\t{}\t{}\t{}",
                index + 1,
                field(candidate, "text"),
                field(candidate, "code"),
                field(candidate, "origin")
            );
            if !candidate["weight"].is_null() {
                line.push('\t');
                line.push_str(&field(candidate, "weight"));
            }
            line
        })
        .collect()
}

/// 每项偏好一行 `key = value`，字符串不带引号，正好是 `config set` 接受的写法。
fn preference_lines(view: &Value) -> Vec<String> {
    view.as_object()
        .into_iter()
        .flatten()
        .map(|(key, value)| format!("{key} = {}", plain(value)))
        .collect()
}

/// 一个值在终端里的写法：字符串不带引号，其余照 JSON。
fn plain(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

/// `config get` 要的几项，按要的顺序；有一项不存在就整体拒绝，报出它的名字。
fn picked_preferences(view: &Value, keys: &[String]) -> Result<Vec<(String, Value)>, String> {
    keys.iter()
        .map(|key| {
            view.get(key)
                .map(|value| (key.clone(), value.clone()))
                .ok_or_else(|| key.clone())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn candidates_print_one_per_line_with_the_weight_only_when_there_is_one() {
        let view = json!({ "candidates": [
            { "text": "你好", "code": "ni'hao", "origin": "dictionary", "weight": 300 },
            { "text": "倪好", "code": "ni'hao", "origin": "generated" },
        ]});
        assert_eq!(
            candidate_lines(&view),
            [
                "1\t你好\tni'hao\tdictionary\t300",
                "2\t倪好\tni'hao\tgenerated"
            ]
        );
        assert!(candidate_lines(&json!({ "candidates": [] })).is_empty());
    }

    #[test]
    fn config_get_picks_the_keys_in_the_order_asked() {
        let view = json!({ "candidate_page_size": 6, "scheme": "quanpin" });
        let keys = ["scheme".to_owned(), "candidate_page_size".to_owned()];
        let picked = picked_preferences(&view, &keys).unwrap();
        assert_eq!(
            picked
                .iter()
                .map(|(_, value)| plain(value))
                .collect::<Vec<_>>(),
            ["quanpin", "6"]
        );
        assert_eq!(
            picked_preferences(&view, &["no_such".to_owned()]).unwrap_err(),
            "no_such"
        );
    }

    #[test]
    fn preferences_print_as_config_set_reads_them() {
        let view = json!({ "candidate_corner_radius": null, "candidate_page_size": 6, "fuzzy_pinyin": false, "scheme": "quanpin" });
        assert_eq!(
            preference_lines(&view),
            [
                "candidate_corner_radius = null",
                "candidate_page_size = 6",
                "fuzzy_pinyin = false",
                "scheme = quanpin"
            ]
        );
    }
}
