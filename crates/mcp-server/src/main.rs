//! `msime-mcp`: a Model Context Protocol server over stdio that lets an agent manage 水杉输入法.
//!
//! stdout carries the protocol and nothing else; anything for a person goes to stderr.

mod bounded;
mod config;
mod diagnostics;
mod preferences;
mod server;
mod statistics;
mod words;

use rmcp::transport::io::stdio;
use rmcp::ServiceExt;
use std::process::ExitCode;

fn main() -> ExitCode {
    // Before the runtime starts any thread: on macOS and Linux the offset cannot be read once the process has more than one.
    diagnostics::remember_local_offset();
    let config = match config::parse(std::env::args_os().skip(1), |name| std::env::var_os(name)) {
        Ok(config::Command::Serve(config)) => config,
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
