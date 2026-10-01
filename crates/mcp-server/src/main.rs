//! `msime-mcp`: a Model Context Protocol server over stdio that lets an agent manage 水杉输入法, and the same tools one per run from a shell.
//!
//! stdout carries the protocol, or a command's JSON result, and nothing else; anything for a person goes to stderr.

mod bounded;
mod cli;
mod config;
mod diagnostics;
mod preferences;
mod prompts;
mod server;
mod skins;
mod statistics;
mod words;

use rmcp::transport::io::stdio;
use rmcp::ServiceExt;
use serde_json::Value;
use std::io::Read;
use std::process::ExitCode;

fn main() -> ExitCode {
    // Before the runtime starts any thread: on macOS and Linux the offset cannot be read once the process has more than one.
    diagnostics::remember_local_offset();
    let (config, action) =
        match config::parse(std::env::args_os().skip(1), |name| std::env::var_os(name)) {
            Ok(config::Command::Serve(config)) => (config, None),
            Ok(config::Command::Tools(config)) => (config, Some(cli::Action::Tools)),
            Ok(config::Command::Call {
                config,
                tool,
                arguments,
            }) => match call_arguments(arguments) {
                Ok(arguments) => (config, Some(cli::Action::Call { tool, arguments })),
                Err(error) => {
                    eprintln!("msime-mcp: {error}");
                    return ExitCode::from(2);
                }
            },
            Ok(config::Command::Help) => {
                eprintln!("{}", config::USAGE);
                return ExitCode::SUCCESS;
            }
            Ok(config::Command::Version) => {
                eprintln!("msime-mcp {}", env!("CARGO_PKG_VERSION"));
                return ExitCode::SUCCESS;
            }
            Err(error) => {
                eprintln!("msime-mcp: {error}\n\n{}", config::USAGE);
                return ExitCode::from(2);
            }
        };
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("msime-mcp: cannot start: {error}");
            return ExitCode::FAILURE;
        }
    };
    if let Some(action) = action {
        return match runtime.block_on(cli::run(config, action)) {
            Ok(code) => code,
            Err(error) => {
                eprintln!("msime-mcp: {error}");
                ExitCode::FAILURE
            }
        };
    }
    let result = runtime.block_on(async {
        server::MsimeServer::new(config)
            .serve(stdio())
            .await?
            .waiting()
            .await?;
        Ok::<(), Box<dyn std::error::Error>>(())
    });
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("msime-mcp: {error}");
            ExitCode::FAILURE
        }
    }
}

/// The JSON object `call` passes to the tool.
fn call_arguments(arguments: config::Arguments) -> Result<serde_json::Map<String, Value>, String> {
    let text = match arguments {
        config::Arguments::Inline(text) => text,
        config::Arguments::Stdin => {
            let mut text = String::new();
            std::io::stdin()
                .read_to_string(&mut text)
                .map_err(|error| format!("cannot read the arguments from stdin: {error}"))?;
            text
        }
    };
    match serde_json::from_str(&text) {
        Ok(Value::Object(arguments)) => Ok(arguments),
        Ok(_) => Err("the arguments must be a JSON object".into()),
        Err(error) => Err(format!("the arguments are not valid JSON: {error}")),
    }
}
