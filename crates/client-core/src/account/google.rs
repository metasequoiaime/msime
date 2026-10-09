//! Desktop Google sign-in through the system browser and a loopback redirect (RFC 8252).
//!
//! The backend owns the PKCE verifier and the client secret; this side only binds the loopback listener, checks the authorization URL the backend built for it, and waits for the browser to deliver the authorization code with the matching `state`.

use super::validate::validate_google_login;
use super::*;
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

/// The longest the loopback listener waits for the browser to come back before the sign-in is abandoned. The actual wait is also cut short so the code reaches the backend before the challenge expires; see [`google_callback_window`].
pub const GOOGLE_SIGN_IN_TIMEOUT: Duration = Duration::from_secs(300);

/// Time kept back from the challenge lifetime for posting the code and the backend's token exchange, so a code delivered at the end of the wait still finds the challenge alive.
const GOOGLE_LOGIN_MARGIN: Duration = Duration::from_secs(30);

const GOOGLE_AUTHORIZATION_PREFIX: &str = "https://accounts.google.com/";
const GOOGLE_CALLBACK_PATH: &str = "/callback";
/// 一次回环请求头的上限。宿主自己读 socket 时（鸿蒙）也按它截断，超过的连接当作不是回跳。
pub const GOOGLE_CALLBACK_MAX_REQUEST_BYTES: usize = 8 * 1024;
const MAX_CALLBACK_REQUEST_BYTES: usize = GOOGLE_CALLBACK_MAX_REQUEST_BYTES;
const MAX_STATE_BYTES: usize = 512;
/// Total time one loopback connection may take to send its request head, and to take the reply. It is short because a real browser sends the redirect at once; an idle preconnect or a slow local client must not hold the listener.
pub const GOOGLE_CALLBACK_IO_TIMEOUT: Duration = Duration::from_secs(2);
const CALLBACK_IO_TIMEOUT: Duration = GOOGLE_CALLBACK_IO_TIMEOUT;
/// How often the accept loop and a pending read look at the deadline and the cancel flag.
const ACCEPT_POLL_INTERVAL: Duration = Duration::from_millis(100);

/// How long to wait for the redirect for a challenge that lives `expires_in` seconds: the challenge lifetime minus [`GOOGLE_LOGIN_MARGIN`], capped at `cap`. `None` when the challenge is too short-lived to be worth opening the browser for.
pub(super) fn google_callback_window(expires_in: u64, cap: Duration) -> Option<Duration> {
    let window = Duration::from_secs(expires_in)
        .saturating_sub(GOOGLE_LOGIN_MARGIN)
        .min(cap);
    (!window.is_zero()).then_some(window)
}

/// The redirect target for a loopback listener on `port`, in the one shape the backend and [`google_loopback_plan`] accept.
pub fn google_loopback_target(port: u16) -> String {
    format!("http://127.0.0.1:{port}{GOOGLE_CALLBACK_PATH}")
}

/// Accepts exactly `http://127.0.0.1:<port>/callback` or `http://[::1]:<port>/callback` with a non-privileged port, the shape the backend accepts for the server-exchange flow.
pub(super) fn valid_google_loopback_target(target: &str) -> bool {
    let Some(rest) = target
        .strip_prefix("http://127.0.0.1:")
        .or_else(|| target.strip_prefix("http://[::1]:"))
    else {
        return false;
    };
    let Some(port) = rest.strip_suffix(GOOGLE_CALLBACK_PATH) else {
        return false;
    };
    !port.is_empty()
        && port.len() <= 5
        && !port.starts_with('0')
        && port.bytes().all(|byte| byte.is_ascii_digit())
        && port
            .parse::<u16>()
            .is_ok_and(|port| (1024..=u16::MAX).contains(&port))
}

/// Checks that the backend's authorization URL is a plain Google authorization URL redirecting to this listener, and returns its `state`. The URL is opened in the user's browser, so a malformed or foreign one is refused rather than opened.
pub(super) fn google_authorization_state(url: &str, target: &str) -> Result<String, AccountError> {
    if !crate::text::is_bounded_text(url, 4096)
        || !url.starts_with(GOOGLE_AUTHORIZATION_PREFIX)
        || url.bytes().any(|byte| {
            byte <= b' '
                || byte >= 0x7f
                || matches!(byte, b'"' | b'\'' | b'`' | b'|' | b'<' | b'>' | b'\\')
        })
    {
        return Err(AccountError::Unavailable);
    }
    let parsed = Url::parse(url).map_err(|_| AccountError::Unavailable)?;
    if parsed.scheme() != "https"
        || parsed.host_str() != Some("accounts.google.com")
        || parsed.port().is_some()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.fragment().is_some()
    {
        return Err(AccountError::Unavailable);
    }
    let redirect = single_query_value(&parsed, "redirect_uri").ok_or(AccountError::Unavailable)?;
    let state = single_query_value(&parsed, "state").ok_or(AccountError::Unavailable)?;
    if redirect != target || !valid_state(&state) {
        return Err(AccountError::Unavailable);
    }
    Ok(state)
}

/// 宿主自己跑回环监听时（鸿蒙由 ArkTS 读写 socket），打开浏览器之前要知道的东西：回跳里应带的 `state`，以及最多等多久。
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct GoogleLoopbackPlan {
    pub state: String,
    pub wait: Duration,
}

/// 校验后端为 `target` 生成的授权链接，并按 challenge 的寿命算出等待时长，与桌面端 [`AccountSession::sign_in_google_with_browser`] 打开浏览器前做的检查相同。`target` 必须是 [`google_loopback_target`] 的形状；链接不合格、不指向这个监听，或 challenge 太短，都返回 [`AccountError::Unavailable`]，宿主不应打开它。
pub fn google_loopback_plan(
    authorization_url: &str,
    target: &str,
    expires_in: u64,
) -> Result<GoogleLoopbackPlan, AccountError> {
    if !valid_google_loopback_target(target) {
        return Err(AccountError::Unavailable);
    }
    let state = google_authorization_state(authorization_url, target)?;
    let wait = google_callback_window(expires_in, GOOGLE_SIGN_IN_TIMEOUT)
        .ok_or(AccountError::Unavailable)?;
    Ok(GoogleLoopbackPlan { state, wait })
}

/// 一次回环请求的结果，以及要原样写回浏览器的完整 HTTP 回应。
#[derive(Debug, Eq, PartialEq)]
pub struct GoogleLoopbackReply {
    pub outcome: GoogleCallback,
    pub response: String,
}

/// 解析一次回环请求的请求头（没读到完整请求头时传 `None`），给出结果和回应。桌面端的监听和鸿蒙的 ArkTS 监听共用它，回跳的判定和浏览器上看到的页面因此只有一份。
pub fn google_loopback_reply(head: Option<&str>, expected_state: &str) -> GoogleLoopbackReply {
    let outcome = head.map_or(GoogleCallback::Ignored, |head| {
        parse_google_callback(head, expected_state)
    });
    let response = callback_response(&outcome);
    GoogleLoopbackReply { outcome, response }
}

fn single_query_value(url: &Url, name: &str) -> Option<String> {
    let mut values = url
        .query_pairs()
        .filter(|(key, _)| key == name)
        .map(|(_, value)| value.into_owned());
    let value = values.next()?;
    values.next().is_none().then_some(value)
}

fn valid_state(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_STATE_BYTES
        && value.bytes().all(|byte| byte.is_ascii_graphic())
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right)
        .fold(0u8, |difference, (a, b)| difference | (a ^ b))
        == 0
}

/// What one loopback request means for the sign-in.
#[derive(Debug, Eq, PartialEq)]
pub enum GoogleCallback {
    /// Not the redirect, or not ours (another path such as `/favicon.ico`, or a `state` that does not match). The listener answers it and keeps waiting.
    Ignored,
    /// The redirect carried an authorization code for this sign-in.
    Code(String),
    /// The redirect ended the sign-in without a code.
    Failed(AccountError),
}

/// Interprets the request head of one loopback connection against the expected `state`.
pub(super) fn parse_google_callback(head: &str, expected_state: &str) -> GoogleCallback {
    let mut parts = head.lines().next().unwrap_or_default().split(' ');
    let (Some("GET"), Some(target), Some(version), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return GoogleCallback::Ignored;
    };
    if !target.starts_with('/') || !version.starts_with("HTTP/") {
        return GoogleCallback::Ignored;
    }
    let Ok(url) = Url::parse(&format!("http://127.0.0.1{target}")) else {
        return GoogleCallback::Ignored;
    };
    if url.path() != GOOGLE_CALLBACK_PATH {
        return GoogleCallback::Ignored;
    }
    let Some(state) = single_query_value(&url, "state") else {
        return GoogleCallback::Ignored;
    };
    if !constant_time_eq(state.as_bytes(), expected_state.as_bytes()) {
        return GoogleCallback::Ignored;
    }
    if url.query_pairs().any(|(key, _)| key == "error") {
        return GoogleCallback::Failed(AccountError::Cancelled);
    }
    match single_query_value(&url, "code") {
        Some(code) if validate_google_login("challenge", &code).is_ok() => {
            GoogleCallback::Code(code)
        }
        _ => GoogleCallback::Failed(AccountError::Unavailable),
    }
}

/// Waits on `listener` for the browser redirect carrying `state`, answering every request with a small page, and returns the authorization code. The wait ends with [`AccountError::Cancelled`] when the user denies access, `cancelled` is set, or `deadline` passes; a single slow connection cannot keep it waiting past the deadline.
pub(super) fn receive_google_callback(
    listener: &TcpListener,
    state: &str,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<String, AccountError> {
    listener
        .set_nonblocking(true)
        .map_err(|_| AccountError::Unavailable)?;
    loop {
        if cancelled.load(Ordering::SeqCst) || Instant::now() >= deadline {
            return Err(AccountError::Cancelled);
        }
        let stream = match listener.accept() {
            Ok((stream, _)) => stream,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(ACCEPT_POLL_INTERVAL);
                continue;
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return Err(AccountError::Unavailable),
        };
        match answer_callback(stream, state, deadline, cancelled) {
            GoogleCallback::Ignored => continue,
            GoogleCallback::Code(code) => return Ok(code),
            GoogleCallback::Failed(error) => return Err(error),
        }
    }
}

/// The page the browser is left on after the redirect. Every field is a constant from [`answer_callback`], never text from the request, so it is substituted into the template without escaping.
struct CallbackPage {
    /// The status badge's colour: `success`, `neutral` or `warning`, a class in the template.
    tone: &'static str,
    /// Path data for the badge's 16×16 stroked glyph.
    icon: &'static str,
    title: &'static str,
    message: &'static str,
}

const CALLBACK_TEMPLATE: &str = include_str!("google_callback.html");
const CHECK_ICON: &str = "M3.5 8.5l3 3 6-7";
const DASH_ICON: &str = "M4 8h8";
const ALERT_ICON: &str = "M8 3.5v5.5M8 12.25v.25";

impl CallbackPage {
    fn render(&self) -> String {
        CALLBACK_TEMPLATE
            .replace("{{tone}}", self.tone)
            .replace("{{icon}}", self.icon)
            .replace("{{title}}", self.title)
            .replace("{{message}}", self.message)
    }
}

fn answer_callback(
    mut stream: TcpStream,
    state: &str,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> GoogleCallback {
    let head = read_request_head(&mut stream, deadline, cancelled);
    let reply = google_loopback_reply(head.as_deref(), state);
    // The browser page is a courtesy; the outcome stands even if the browser already went away.
    let _ = stream.write_all(reply.response.as_bytes());
    let _ = stream.flush();
    reply.outcome
}

/// The HTTP response the browser is left on for `outcome`.
fn callback_response(outcome: &GoogleCallback) -> String {
    let (status, page) = match outcome {
        // The backend has not exchanged the code yet, so the page cannot claim the sign-in succeeded; the app reports the outcome.
        GoogleCallback::Code(_) => (
            "200 OK",
            CallbackPage {
                tone: "success",
                icon: CHECK_ICON,
                title: "已收到 Google 授权",
                message: "请回到水杉输入法，登录会在那里完成。",
            },
        ),
        GoogleCallback::Failed(AccountError::Cancelled) => (
            "200 OK",
            CallbackPage {
                tone: "neutral",
                icon: DASH_ICON,
                title: "已取消 Google 登录",
                message: "请回到水杉输入法，需要时可以重新登录。",
            },
        ),
        GoogleCallback::Failed(_) => (
            "200 OK",
            CallbackPage {
                tone: "warning",
                icon: ALERT_ICON,
                title: "Google 登录未完成",
                message: "请回到水杉输入法重试。",
            },
        ),
        GoogleCallback::Ignored => (
            "404 Not Found",
            CallbackPage {
                tone: "neutral",
                icon: DASH_ICON,
                title: "页面不存在",
                message: "这个地址只用于接收 Google 登录的回调。",
            },
        ),
    };
    let body = page.render();
    // The page carries its own styles and an inline SVG and nothing else, so the policy allows exactly that: no script, no request off the loopback.
    format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nContent-Security-Policy: default-src 'none'; style-src 'unsafe-inline'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

/// Reads one request head within [`CALLBACK_IO_TIMEOUT`] in total (never past `deadline`), giving up early when `cancelled` is set. Reads wait in [`ACCEPT_POLL_INTERVAL`] slices so both limits are rechecked while a client trickles bytes.
fn read_request_head(
    stream: &mut TcpStream,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Option<String> {
    let budget_end = deadline.min(Instant::now() + CALLBACK_IO_TIMEOUT);
    stream.set_nonblocking(false).ok()?;
    stream.set_write_timeout(Some(CALLBACK_IO_TIMEOUT)).ok()?;
    let mut head = Vec::with_capacity(1024);
    let mut buffer = [0u8; 1024];
    while !head.windows(4).any(|window| window == b"\r\n\r\n") {
        if head.len() >= MAX_CALLBACK_REQUEST_BYTES || cancelled.load(Ordering::SeqCst) {
            return None;
        }
        let remaining = budget_end.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return None;
        }
        stream
            .set_read_timeout(Some(remaining.min(ACCEPT_POLL_INTERVAL)))
            .ok()?;
        let read = match stream.read(&mut buffer) {
            Ok(read) => read,
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock
                        | std::io::ErrorKind::TimedOut
                        | std::io::ErrorKind::Interrupted
                ) =>
            {
                continue
            }
            Err(_) => return None,
        };
        if read == 0 {
            break;
        }
        head.extend_from_slice(&buffer[..read]);
    }
    String::from_utf8(head).ok()
}
