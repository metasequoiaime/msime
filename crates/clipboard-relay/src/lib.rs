//! 云剪贴板的参考中继：按 `docs/cloud-clipboard-relay.md` 实现 `docs/cloud-clipboard-e2e.md` 里客户端用到的那组接口，只保存密文，拿不到剪贴板密钥。
//!
//! 协议以文档为准。官方服务（msime-cloud，Go 加 PostgreSQL）是另一份独立的实现，两边各自用测试锁住同一份文档；这个 crate 的测试用真正的客户端驱动（`msime-client-services` 的 `clipboard_relay`）连它跑完整流程。
//!
//! 鉴权只有一种：管理员发的静态令牌，`Authorization: Bearer <令牌>`。每个令牌对应一个独立的剪贴板空间，空间编号是 SHA-256(令牌) 的十六进制；令牌本身不落盘，数据库里只有空间编号。中继只说 HTTP，必须放在终止 TLS 的反向代理后面。

pub mod store;

use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, State};
use axum::http::{header, HeaderMap, HeaderValue, Method, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::Router;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::net::SocketAddr;
use std::sync::Arc;
use store::{RelayStore, Request};

/// 请求体上限。驱动最大的请求是一条 16384 个字符的密文或 20 个封装，远小于它。
pub const MAX_BODY_BYTES: usize = 64 * 1024;
const MIN_TOKEN_CHARS: usize = 16;
const MAX_TOKEN_CHARS: usize = 256;

/// 令牌是否可用：16 到 256 个可打印 ASCII 字符，不含空格，与客户端的规则相同。
pub fn valid_token(token: &str) -> bool {
    (MIN_TOKEN_CHARS..=MAX_TOKEN_CHARS).contains(&token.len())
        && token.bytes().all(|byte| (33..=126).contains(&byte))
}

/// 令牌对应的空间编号：SHA-256(令牌) 的小写十六进制。
pub fn space_of(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

/// 读令牌文件：每行一个令牌，空行和 `#` 开头的行忽略。有不合规的行时报出行号，不说出内容。
pub fn parse_tokens(text: &str) -> Result<Vec<String>, String> {
    let mut tokens = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if !valid_token(line) {
            return Err(format!(
                "line {} is not a valid token (16-256 printable ASCII characters, no spaces)",
                index + 1
            ));
        }
        tokens.push(line.to_owned());
    }
    if tokens.is_empty() {
        return Err("no tokens".into());
    }
    Ok(tokens)
}

/// 生成一个新令牌：32 字节随机数的 base64url，43 个字符。
pub fn generate_token() -> Result<String, String> {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|_| "secure random source unavailable".to_owned())?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

/// 数据和认得的空间。克隆只复制两个引用。
#[derive(Clone)]
pub struct Relay {
    store: Arc<RelayStore>,
    spaces: Arc<HashSet<String>>,
}

impl Relay {
    pub fn new(store: RelayStore, tokens: &[String]) -> Self {
        Self {
            store: Arc::new(store),
            spaces: Arc::new(tokens.iter().map(|token| space_of(token)).collect()),
        }
    }

    /// 请求的令牌对应的空间；没有令牌或不认识时为 `None`。先哈希再查，比较的是摘要而不是令牌本身。
    fn space(&self, headers: &HeaderMap) -> Option<String> {
        let value = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
        let token = value.strip_prefix("Bearer ")?;
        if !valid_token(token) {
            return None;
        }
        let space = space_of(token);
        self.spaces.contains(&space).then_some(space)
    }
}

/// HTTP 层：认出空间后把请求原样交给 [`RelayStore::handle`]。路由全在 store 里，这里不分路径。
pub fn router(relay: Relay) -> Router {
    Router::new()
        .fallback(handle)
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .with_state(relay)
}

async fn handle(
    State(relay): State<Relay>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let Some(space) = relay.space(&headers) else {
        let mut response = json_response(store::Response::error(401, "unauthorized"));
        response
            .headers_mut()
            .insert(header::WWW_AUTHENTICATE, HeaderValue::from_static("Bearer"));
        return response;
    };
    let path = uri
        .path_and_query()
        .map(|path| path.as_str().to_owned())
        .unwrap_or_default();
    let store = Arc::clone(&relay.store);
    let method = method.as_str().to_owned();
    let answered = tokio::task::spawn_blocking(move || {
        store.handle(
            &space,
            &Request {
                method: &method,
                path: &path,
                body: &body,
            },
        )
    })
    .await;
    match answered {
        Ok(response) => json_response(response),
        Err(_) => json_response(store::Response::error(500, "internal")),
    }
}

fn json_response(response: store::Response) -> Response {
    let status = StatusCode::from_u16(response.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    match response.body {
        Some(body) => (status, [(header::CONTENT_TYPE, "application/json")], body).into_response(),
        None => status.into_response(),
    }
}

/// 在后台线程上跑着的中继。丢弃时停止。
pub struct Server {
    address: SocketAddr,
    runtime: Option<tokio::runtime::Runtime>,
}

impl Server {
    /// 在 `listen` 上开始服务；端口为 0 时由系统挑一个，用 [`Server::address`] 读出。
    pub fn start(listen: SocketAddr, relay: Relay) -> std::io::Result<Self> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_io()
            .build()?;
        let listener = runtime.block_on(tokio::net::TcpListener::bind(listen))?;
        let address = listener.local_addr()?;
        runtime.spawn(async move {
            let _ = axum::serve(listener, router(relay)).await;
        });
        Ok(Self {
            address,
            runtime: Some(runtime),
        })
    }

    pub fn address(&self) -> SocketAddr {
        self.address
    }

    /// 一直服务下去，直到进程被结束。
    pub fn wait(self) -> ! {
        loop {
            std::thread::park();
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_files_skip_comments_and_name_bad_lines_by_number() {
        let tokens = parse_tokens(
            "# 管理员的说明\n\nsynthetic-relay-token-0001\n  synthetic-relay-token-0002  \n",
        )
        .unwrap();
        assert_eq!(
            tokens,
            ["synthetic-relay-token-0001", "synthetic-relay-token-0002"]
        );
        let error = parse_tokens("synthetic-relay-token-0001\nshort\n").unwrap_err();
        assert!(error.contains("line 2"), "{error}");
        assert!(!error.contains("short"), "{error}");
        assert!(parse_tokens("# only comments\n").is_err());
    }

    #[test]
    fn generated_tokens_are_valid_and_distinct() {
        let first = generate_token().unwrap();
        let second = generate_token().unwrap();
        assert_eq!(first.len(), 43);
        assert!(valid_token(&first));
        assert_ne!(first, second);
    }

    #[test]
    fn only_listed_bearer_tokens_name_a_space() {
        let relay = Relay::new(
            RelayStore::in_memory().unwrap(),
            &["synthetic-relay-token-0001".to_owned()],
        );
        let headers = |value: &str| {
            let mut headers = HeaderMap::new();
            headers.insert(header::AUTHORIZATION, HeaderValue::from_str(value).unwrap());
            headers
        };
        assert_eq!(
            relay.space(&headers("Bearer synthetic-relay-token-0001")),
            Some(space_of("synthetic-relay-token-0001"))
        );
        assert_eq!(
            relay.space(&headers("Bearer synthetic-relay-token-0002")),
            None
        );
        assert_eq!(
            relay.space(&headers("Basic synthetic-relay-token-0001")),
            None
        );
        assert_eq!(relay.space(&HeaderMap::new()), None);
    }
}
