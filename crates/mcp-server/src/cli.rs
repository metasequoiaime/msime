//! The tools and prompts from a shell, one per run, for an assistant that drives a terminal rather than speaking MCP.
//!
//! The command line does not reimplement any tool: it starts the same server in process and calls it over an in-memory pipe as an MCP client would, so the tools offered, their arguments, their limits and their answers are the server's own.

use crate::config::Config;
use crate::server::MsimeServer;
use rmcp::model::{CallToolRequestParams, GetPromptRequestParams};
use rmcp::ServiceExt;
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
        Action::Call { tool, arguments } => {
            // A tool the flags do not offer is the likeliest refusal here, and the server's own answer does not say why it is missing.
            let result = client
                .call_tool(CallToolRequestParams::new(tool).with_arguments(arguments))
                .await
                .map_err(|error| {
                    format!("{error}; `tools` lists what these flags offer: writing needs --allow-write, dictionary words --allow-dictionary-read")
                })?;
            let text = || {
                result
                    .content
                    .iter()
                    .filter_map(|block| block.as_text().map(|text| text.text.as_str()))
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            if result.is_error == Some(true) {
                eprintln!("msime-mcp: {}", text());
                ExitCode::FAILURE
            } else {
                match &result.structured_content {
                    Some(value) => println!("{}", serde_json::to_string_pretty(value)?),
                    None => println!("{}", text()),
                }
                ExitCode::SUCCESS
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
            println!("\nFrom a shell, run each tool this names as `msime-mcp <the same flags> call <tool> '<json arguments>'`.");
            ExitCode::SUCCESS
        }
    };
    client.cancel().await?;
    server.await?.map_err(|error| error.to_string())?;
    Ok(code)
}
