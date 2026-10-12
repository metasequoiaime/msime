//! `msime-clipboard-relay`：参考中继的命令行。
//!
//! ```text
//! msime-clipboard-relay gen-token
//! msime-clipboard-relay --listen 127.0.0.1:8787 --data /var/lib/msime-relay/relay.db --tokens /etc/msime-relay/tokens
//! ```
//!
//! 令牌文件每行一个令牌（`gen-token` 生成），改了令牌文件要重启。中继只说 HTTP，部署时放在终止 TLS 的反向代理后面，见 `docs/cloud-clipboard-relay.md`。

use msime_clipboard_relay::{generate_token, parse_tokens, store::RelayStore, Relay, Server};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "usage:
  msime-clipboard-relay gen-token
  msime-clipboard-relay --listen <address:port> --data <database file> --tokens <token file>";
/// 令牌文件最多这么大；每行一个不超过 256 个字符的令牌，一千个令牌也只有几百 KiB。
const MAX_TOKEN_FILE_BYTES: u64 = 1024 * 1024;

struct Options {
    listen: SocketAddr,
    data: PathBuf,
    tokens: PathBuf,
}

fn parse(arguments: &[String]) -> Result<Options, String> {
    let mut listen = None;
    let mut data = None;
    let mut tokens = None;
    let mut rest = arguments.iter();
    while let Some(flag) = rest.next() {
        let value = rest.next().ok_or_else(|| format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--listen" => {
                listen = Some(
                    value
                        .parse::<SocketAddr>()
                        .map_err(|_| format!("--listen {value} is not an address:port"))?,
                )
            }
            "--data" => data = Some(PathBuf::from(value)),
            "--tokens" => tokens = Some(PathBuf::from(value)),
            _ => return Err(format!("unknown option {flag}")),
        }
    }
    Ok(Options {
        listen: listen.ok_or("--listen is required")?,
        data: data.ok_or("--data is required")?,
        tokens: tokens.ok_or("--tokens is required")?,
    })
}

fn run(arguments: &[String]) -> Result<(), String> {
    if arguments.first().map(String::as_str) == Some("gen-token") {
        if arguments.len() != 1 {
            return Err(USAGE.into());
        }
        println!("{}", generate_token()?);
        return Ok(());
    }
    let options = parse(arguments)?;
    let metadata = std::fs::metadata(&options.tokens)
        .map_err(|error| format!("cannot read {}: {error}", options.tokens.display()))?;
    if metadata.len() > MAX_TOKEN_FILE_BYTES {
        return Err(format!("{} is too large", options.tokens.display()));
    }
    let text = std::fs::read_to_string(&options.tokens)
        .map_err(|error| format!("cannot read {}: {error}", options.tokens.display()))?;
    let tokens =
        parse_tokens(&text).map_err(|error| format!("{}: {error}", options.tokens.display()))?;
    let store = RelayStore::open(&options.data)
        .map_err(|error| format!("cannot open {}: {error}", options.data.display()))?;
    let server = Server::start(options.listen, Relay::new(store, &tokens))
        .map_err(|error| format!("cannot listen on {}: {error}", options.listen))?;
    eprintln!(
        "msime-clipboard-relay: {} token space(s), listening on http://{}",
        tokens.len(),
        server.address()
    );
    if !server.address().ip().is_loopback() {
        eprintln!("msime-clipboard-relay: this relay speaks plain HTTP; put it behind a reverse proxy that terminates TLS");
    }
    server.wait()
}

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if arguments.is_empty()
        || arguments
            .iter()
            .any(|argument| argument == "--help" || argument == "-h")
    {
        eprintln!("{USAGE}");
        return if arguments.is_empty() {
            ExitCode::FAILURE
        } else {
            ExitCode::SUCCESS
        };
    }
    match run(&arguments) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("msime-clipboard-relay: {error}");
            ExitCode::FAILURE
        }
    }
}
