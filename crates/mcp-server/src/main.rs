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
use std::process::ExitCode;

const ARGUMENTS_READ_LIMIT: u64 = 8 * 1024 * 1024;

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
            Ok(config::Command::Prompts(config)) => (config, Some(cli::Action::Prompts)),
            Ok(config::Command::Prompt {
                config,
                name,
                arguments,
            }) => match call_arguments(arguments) {
                Ok(arguments) => (config, Some(cli::Action::Prompt { name, arguments })),
                Err(error) => {
                    eprintln!("msime-mcp: {error}");
                    return ExitCode::from(2);
                }
            },
            Ok(config::Command::Expand {
                config,
                code,
                scheme,
                limit,
                json,
            }) => {
                let mut arguments = serde_json::Map::new();
                arguments.insert("code".into(), code.into());
                if let Some(scheme) = scheme {
                    arguments.insert("scheme".into(), scheme.into());
                }
                if let Some(limit) = limit {
                    arguments.insert("limit".into(), limit.into());
                }
                (config, Some(cli::Action::Expand { arguments, json }))
            }
            Ok(config::Command::ShowConfig { config, json }) => {
                (config, Some(cli::Action::ShowConfig { json }))
            }
            Ok(config::Command::SetConfig {
                config,
                changes,
                json,
            }) => (
                config,
                Some(cli::Action::SetConfig {
                    changes: changes.into_iter().collect(),
                    json,
                }),
            ),
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

/// The JSON object `call` passes to the tool, or `prompt` to the prompt.
fn call_arguments(arguments: config::Arguments) -> Result<serde_json::Map<String, Value>, String> {
    let text = match arguments {
        config::Arguments::Inline(text) => text,
        config::Arguments::File(path) => {
            let file = std::fs::File::open(&path).map_err(|error| {
                format!("cannot read the arguments from {}: {error}", path.display())
            })?;
            let bytes =
                crate::bounded::read(file, ARGUMENTS_READ_LIMIT).map_err(|error| match error {
                    crate::bounded::ReadError::TooLarge => {
                        format!("the arguments from {} are too large", path.display())
                    }
                    crate::bounded::ReadError::Io => {
                        format!("cannot read the arguments from {}", path.display())
                    }
                })?;
            String::from_utf8(bytes)
                .map_err(|_| format!("the arguments from {} are not UTF-8", path.display()))?
        }
        config::Arguments::Stdin => {
            let bytes =
                crate::bounded::read(std::io::stdin(), ARGUMENTS_READ_LIMIT).map_err(|error| {
                    match error {
                        crate::bounded::ReadError::TooLarge => {
                            "the arguments from stdin are too large"
                        }
                        crate::bounded::ReadError::Io => "cannot read the arguments from stdin",
                    }
                })?;
            String::from_utf8(bytes)
                .map_err(|_| String::from("the arguments from stdin are not UTF-8"))?
        }
    };
    match serde_json::from_str(&text) {
        Ok(Value::Object(arguments)) => Ok(arguments),
        Ok(_) => Err("the arguments must be a JSON object".into()),
        Err(error) => Err(format!("the arguments are not valid JSON: {error}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn argument_files_larger_than_the_request_budget_are_rejected() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("arguments.json");
        let mut file = std::fs::File::create(&path).unwrap();
        let value = format!(r#"{{"text":"{}"}}"#, "x".repeat(9 * 1024 * 1024));
        file.write_all(value.as_bytes()).unwrap();
        let result = call_arguments(config::Arguments::File(path));
        assert!(result.is_err());
    }
}
