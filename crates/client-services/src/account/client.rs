//! `BackendAccountClient`: one HTTPS call per account operation, over the
//! transport the host injected.

use super::clipboard::{CLIPBOARD_PATH, MAX_CLIPBOARD_RESPONSE_BYTES};
use super::validate::*;
use super::*;
use crate::cloud::clipboard_e2e::{HttpRequest, HttpResponse};

type SharedAccountHttp = std::sync::Arc<
    std::sync::Mutex<Option<(msime_client_core::network::NetworkPreferences, Client)>>,
>;

#[derive(Clone)]
pub struct BackendAccountClient {
    client: SharedAccountHttp,
    agent: String,
    origin: Url,
}

/// 没有宿主自报的 User-Agent 时发的值，与改动前相同。
const DEFAULT_USER_AGENT: &str = "MSIME/Android";
/// 服务端认的平台 ID（msime-cloud `internal/account/sessions.go` 的 `sessionPlatforms`）。
const CLIENT_AGENT_PLATFORMS: [&str; 6] =
    ["android", "ios", "macos", "windows", "linux", "harmony"];
const MAX_CLIENT_AGENT_BYTES: usize = 256;
const MAX_CLIENT_AGENT_VERSION_BYTES: usize = 32;

/// 宿主自报的 User-Agent 是否是服务端「我的设备」认得出的形状：`msime-<平台>/<版本> (<设备>; <系统>; edition=<id>)`，括号部分可省略。与服务端的 `msimeAgent` 正则 `^msime-([a-z]+)/([0-9A-Za-z.+_-]{1,32})(?: \(([^()]*)\))?$` 等价，另外要求平台是六个之一、整串不超过 256 字节且全是可打印 ASCII。用字符串切分实现，不用正则。
pub fn valid_client_agent(agent: &str) -> bool {
    if agent.len() > MAX_CLIENT_AGENT_BYTES
        || !agent.bytes().all(|byte| (0x20..=0x7e).contains(&byte))
    {
        return false;
    }
    let Some(rest) = agent.strip_prefix("msime-") else {
        return false;
    };
    let Some((platform, rest)) = rest.split_once('/') else {
        return false;
    };
    if !CLIENT_AGENT_PLATFORMS.contains(&platform) {
        return false;
    }
    let (version, detail) = match rest.split_once(' ') {
        Some((version, detail)) => (version, Some(detail)),
        None => (rest, None),
    };
    let version_valid = !version.is_empty()
        && version.len() <= MAX_CLIENT_AGENT_VERSION_BYTES
        && version
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b".+_-".contains(&byte));
    let detail_valid = detail.is_none_or(|detail| {
        detail
            .strip_prefix('(')
            .and_then(|inner| inner.strip_suffix(')'))
            .is_some_and(|inner| !inner.contains(['(', ')']))
    });
    version_valid && detail_valid
}

/// [`client_agent`] 括号里的设备、系统和 edition 每段最多保留的字符数（都是 ASCII，也就是字节数）。
const MAX_CLIENT_AGENT_PART_BYTES: usize = 64;

/// 宿主自报的 User-Agent 由这里统一拼出：`msime-<platform>/<version> (<device>; <system>; edition=<edition>)`，用例在 `shared/contracts/client-agent/cases.json`。`platform` 必须是服务端认的六个平台之一（原样比较，不改大小写），否则是 [`AccountError::Invalid`]；其余各段只清洗、不拒绝，结果总能通过 [`valid_client_agent`]。
///
/// 清洗与 Android 原来的 `BackendAccount.loginUserAgent` 逐字相同：去掉首尾的空白和控制字符（码位不大于空格的），去掉不是可打印 ASCII 的字符和 `(`、`)`、`;`，保留到 64 个字符为止，再去掉一次首尾空白；结果为空时 `device` 和 `system` 用 `unknown`，`edition` 用 `full`。`device`、`system` 为 `None` 时与空串相同。`system` 是系统名加版本（Android 传 `Android <Build.VERSION.RELEASE>`）。`version` 要符合服务端的版本号写法，所以只保留 `[0-9A-Za-z.+_-]`、最多 32 个，为空时用 `0`。
pub fn client_agent(
    platform: &str,
    version: &str,
    device: Option<&str>,
    system: Option<&str>,
    edition: &str,
) -> Result<String, AccountError> {
    if !CLIENT_AGENT_PLATFORMS.contains(&platform) {
        return Err(AccountError::Invalid);
    }
    let version: String = version
        .chars()
        .filter(|character| character.is_ascii_alphanumeric() || ".+_-".contains(*character))
        .take(MAX_CLIENT_AGENT_VERSION_BYTES)
        .collect();
    let version = if version.is_empty() { "0" } else { &version };
    let agent = format!(
        "msime-{platform}/{version} ({}; {}; edition={})",
        client_agent_part(device.unwrap_or(""), "unknown"),
        client_agent_part(system.unwrap_or(""), "unknown"),
        client_agent_part(edition, "full"),
    );
    debug_assert!(valid_client_agent(&agent), "{agent}");
    Ok(agent)
}

/// [`client_agent`] 括号里的一段，规则见那里。去首尾空白用的是 Java `String.trim` 的口径（码位不大于空格），不是 Unicode 空白。
fn client_agent_part(value: &str, fallback: &str) -> String {
    let java_trim = |text: &str| text.trim_matches(|character| character <= ' ').to_owned();
    let cleaned: String = java_trim(value)
        .chars()
        .filter(|character| (' '..='~').contains(character) && !"();".contains(*character))
        .take(MAX_CLIENT_AGENT_PART_BYTES)
        .collect();
    let cleaned = java_trim(&cleaned);
    if cleaned.is_empty() {
        fallback.to_owned()
    } else {
        cleaned
    }
}

fn build_http_client(
    agent: &str,
    origin: &Url,
    network: &msime_client_core::network::NetworkPreferences,
) -> Result<Client, AccountError> {
    // 测试回环和正式源站都按目标判据配置，系统模式也不能把本机令牌交给代理。
    crate::proxy::configure_blocking_for_with_preferences(Client::builder(), origin, network)
        .map_err(|_| AccountError::Unavailable)?
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(30))
        .user_agent(agent)
        .build()
        .map_err(|_| AccountError::Unavailable)
}

/// 默认账号共享连接池；长期会话每次请求检查代理版本，变更后替换池，而不是只在构造会话时更新。
/// 自报 User-Agent 和测试源站各自持有缓存；令牌只写在请求上，不进入连接池。
fn shared_default_client() -> SharedAccountHttp {
    static SHARED: std::sync::OnceLock<SharedAccountHttp> = std::sync::OnceLock::new();
    SHARED
        .get_or_init(|| std::sync::Arc::new(std::sync::Mutex::new(None)))
        .clone()
}

impl BackendAccountClient {
    pub fn new() -> Result<Self, AccountError> {
        Self::with_origin(ACCOUNT_ORIGIN, false)
    }

    /// 与 [`Self::new`] 相同，只是登录、刷新和其余请求都以 `agent` 作 User-Agent，服务端「我的设备」据此显示平台、设备和版本。`agent` 不是 [`valid_client_agent`] 认的形状时是 [`AccountError::Invalid`]。
    pub fn with_user_agent(agent: &str) -> Result<Self, AccountError> {
        if !valid_client_agent(agent) {
            return Err(AccountError::Invalid);
        }
        Self::with_origin_and_agent(ACCOUNT_ORIGIN, false, agent)
    }

    fn with_origin(origin: &str, allow_http_loopback: bool) -> Result<Self, AccountError> {
        Self::with_origin_and_agent(origin, allow_http_loopback, DEFAULT_USER_AGENT)
    }

    fn with_origin_and_agent(
        origin: &str,
        allow_http_loopback: bool,
        agent: &str,
    ) -> Result<Self, AccountError> {
        let origin = Url::parse(origin).map_err(|_| AccountError::Invalid)?;
        let valid_scheme = origin.scheme() == "https"
            || (allow_http_loopback
                && origin.scheme() == "http"
                && origin.host_str() == Some("127.0.0.1"));
        if !valid_scheme
            || origin.cannot_be_a_base()
            || origin.username() != ""
            || origin.password().is_some()
            || origin.query().is_some()
            || origin.fragment().is_some()
        {
            return Err(AccountError::Invalid);
        }
        // 正式服务端、默认 User-Agent 的客户端整个进程共用一个；自报 User-Agent 的和测试连本机回环的各建各的。
        let client = if agent == DEFAULT_USER_AGENT && !allow_http_loopback {
            shared_default_client()
        } else {
            std::sync::Arc::new(std::sync::Mutex::new(None))
        };
        let account = Self {
            client,
            origin,
            agent: agent.to_owned(),
        };
        account.http()?;
        Ok(account)
    }

    #[cfg(test)]
    pub(crate) fn loopback(origin: &str) -> Result<Self, AccountError> {
        Self::with_origin(origin, true)
    }

    /// 测试用：连 `http://127.0.0.1:<端口>` 上的假服务端，以 `agent` 作 User-Agent（不校验形状，默认的 `MSIME/Android` 也可以）。其他 crate 的测试经 `test-support` 特性用它。
    #[cfg(any(test, feature = "test-support"))]
    pub fn loopback_with_agent(origin: &str, agent: &str) -> Result<Self, AccountError> {
        Self::with_origin_and_agent(origin, true, agent)
    }

    #[cfg(any(test, feature = "telemetry-loopback"))]
    pub(crate) fn telemetry_loopback(origin: &str) -> Result<Self, AccountError> {
        Self::with_origin(origin, true)
    }

    fn request(
        &self,
        method: Method,
        path: &str,
        token: Option<&str>,
        body: Option<Vec<u8>>,
    ) -> Result<Vec<u8>, AccountError> {
        self.request_with_limit(method, path, token, body, MAX_JSON_BYTES)
    }

    pub(crate) fn request_with_limit(
        &self,
        method: Method,
        path: &str,
        token: Option<&str>,
        body: Option<Vec<u8>>,
        maximum_response_bytes: usize,
    ) -> Result<Vec<u8>, AccountError> {
        self.request_with_limit_timeout(
            method,
            path,
            token,
            body,
            maximum_response_bytes,
            Duration::from_secs(30),
        )
    }

    pub(crate) fn request_with_limit_timeout(
        &self,
        method: Method,
        path: &str,
        token: Option<&str>,
        body: Option<Vec<u8>>,
        maximum_response_bytes: usize,
        timeout: Duration,
    ) -> Result<Vec<u8>, AccountError> {
        self.request_with_limit_timeout_accept(
            method,
            path,
            token,
            body,
            MAX_JSON_BYTES,
            maximum_response_bytes,
            timeout,
            "application/json",
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn request_with_limit_timeout_accept(
        &self,
        method: Method,
        path: &str,
        token: Option<&str>,
        body: Option<Vec<u8>>,
        maximum_request_bytes: usize,
        maximum_response_bytes: usize,
        timeout: Duration,
        accept: &str,
    ) -> Result<Vec<u8>, AccountError> {
        let response = self
            .prepare(method, path, token, body, maximum_request_bytes, accept)?
            .timeout(timeout)
            .send()
            .map_err(|_| AccountError::Unavailable)?;
        read_bounded_response(response, maximum_response_bytes)
    }

    /// 探测同源接口明确返回的能力头，仍完整读取有界正文并处理 HTTP 错误。
    pub(crate) fn supports_response_header(
        &self,
        path: &str,
        token: Option<&str>,
        header: &str,
        expected: &str,
    ) -> Result<bool, AccountError> {
        let response = self
            .prepare(
                Method::GET,
                path,
                token,
                None,
                MAX_JSON_BYTES,
                "application/json",
            )?
            .timeout(Duration::from_secs(30))
            .send()
            .map_err(|_| AccountError::Unavailable)?;
        let supported = response
            .headers()
            .get(header)
            .and_then(|value| value.to_str().ok())
            == Some(expected);
        read_bounded_response(response, MAX_JSON_BYTES)?;
        Ok(supported)
    }

    /// 不带请求体的 JSON 请求，另外要求成功的响应带着能力头 `header: expected`。服务端认得请求里新加的查询参数时才回这个头，旧部署会忽略不认识的参数照常作答，所以没有这个头的成功响应按 [`AccountError::Unavailable`] 处理，正文不交给调用方；失败的响应照常按状态码和服务端的错误码报告。
    pub(crate) fn json_with_required_header<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        token: Option<&str>,
        header: &str,
        expected: &str,
    ) -> Result<T, AccountError> {
        let response = self
            .prepare(
                method,
                path,
                token,
                None,
                MAX_JSON_BYTES,
                "application/json",
            )?
            .timeout(Duration::from_secs(30))
            .send()
            .map_err(|_| AccountError::Unavailable)?;
        let supported = response
            .headers()
            .get(header)
            .and_then(|value| value.to_str().ok())
            == Some(expected);
        let bytes = read_bounded_response(response, MAX_JSON_BYTES)?;
        if !supported {
            return Err(AccountError::Unavailable);
        }
        serde_json::from_slice(&bytes).map_err(|_| AccountError::Unavailable)
    }

    /// 校验路径、凭据和请求体后组装请求；请求只会发往构造时给定的源站。
    pub(super) fn prepare(
        &self,
        method: Method,
        path: &str,
        token: Option<&str>,
        body: Option<Vec<u8>>,
        maximum_request_bytes: usize,
        accept: &str,
    ) -> Result<reqwest::blocking::RequestBuilder, AccountError> {
        if !path.starts_with("/v1/") || path.contains('\\') {
            return Err(AccountError::Invalid);
        }
        if body
            .as_ref()
            .is_some_and(|value| value.len() > maximum_request_bytes)
            || token.is_some_and(|value| value.is_empty() || value.chars().any(char::is_whitespace))
        {
            return Err(AccountError::Invalid);
        }
        let url = self.origin.join(path).map_err(|_| AccountError::Invalid)?;
        if url.scheme() != self.origin.scheme()
            || url.host_str() != self.origin.host_str()
            || url.port_or_known_default() != self.origin.port_or_known_default()
            || url.username() != ""
            || url.password().is_some()
            || url.fragment().is_some()
        {
            return Err(AccountError::Invalid);
        }
        let mut request = self
            .http()?
            .request(method, url)
            .header(reqwest::header::ACCEPT, accept);
        if let Some(token) = token {
            request = request.bearer_auth(token);
        }
        if let Some(body) = body {
            request = request
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(body);
        }
        Ok(request)
    }

    pub(crate) fn json<T: DeserializeOwned, B: Serialize>(
        &self,
        method: Method,
        path: &str,
        token: Option<&str>,
        body: Option<&B>,
    ) -> Result<T, AccountError> {
        let body = body
            .map(serde_json::to_vec)
            .transpose()
            .map_err(|_| AccountError::Invalid)?;
        let bytes = self.request(method, path, token, body)?;
        serde_json::from_slice(&bytes).map_err(|_| AccountError::Unavailable)
    }

    pub(crate) fn json_with_limit<T: DeserializeOwned, B: Serialize>(
        &self,
        method: Method,
        path: &str,
        token: Option<&str>,
        body: Option<&B>,
        maximum_response_bytes: usize,
    ) -> Result<T, AccountError> {
        let body = body
            .map(serde_json::to_vec)
            .transpose()
            .map_err(|_| AccountError::Invalid)?;
        let bytes = self.request_with_limit(method, path, token, body, maximum_response_bytes)?;
        serde_json::from_slice(&bytes).map_err(|_| AccountError::Unavailable)
    }

    pub(crate) fn json_with_limit_timeout<T: DeserializeOwned, B: Serialize>(
        &self,
        method: Method,
        path: &str,
        token: Option<&str>,
        body: Option<&B>,
        maximum_response_bytes: usize,
        timeout: Duration,
    ) -> Result<T, AccountError> {
        let body = body
            .map(serde_json::to_vec)
            .transpose()
            .map_err(|_| AccountError::Invalid)?;
        let bytes = self.request_with_limit_timeout(
            method,
            path,
            token,
            body,
            maximum_response_bytes,
            timeout,
        )?;
        serde_json::from_slice(&bytes).map_err(|_| AccountError::Unavailable)
    }

    /// Like [`Self::json_with_limit_timeout`], for the few requests whose body may exceed the 1 MiB every other account request is held to. The caller names the request bound explicitly, so nothing else inherits it.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn json_with_limits_timeout<T: DeserializeOwned, B: Serialize>(
        &self,
        method: Method,
        path: &str,
        token: Option<&str>,
        body: Option<&B>,
        maximum_request_bytes: usize,
        maximum_response_bytes: usize,
        timeout: Duration,
    ) -> Result<T, AccountError> {
        let body = body
            .map(serde_json::to_vec)
            .transpose()
            .map_err(|_| AccountError::Invalid)?;
        let bytes = self.request_with_limit_timeout_accept(
            method,
            path,
            token,
            body,
            maximum_request_bytes,
            maximum_response_bytes,
            timeout,
            "application/json",
        )?;
        serde_json::from_slice(&bytes).map_err(|_| AccountError::Unavailable)
    }

    /// 返回 [`AccountFailure`] 的新接口发请求用：与 [`Self::request_with_limit_timeout_accept`] 相同，失败时带着状态码、服务端错误码和 `Retry-After`，没拿到响应时是 [`AccountFailure::network`]。请求体超过 `maximum_request_bytes` 时在本地以 [`AccountFailure::too_large`] 拒绝，不发出去。
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn request_detailed(
        &self,
        method: Method,
        path: &str,
        token: Option<&str>,
        body: Option<Vec<u8>>,
        maximum_request_bytes: usize,
        maximum_response_bytes: usize,
        timeout: Duration,
        accept: &str,
    ) -> Result<Vec<u8>, AccountFailure> {
        if body
            .as_ref()
            .is_some_and(|body| body.len() > maximum_request_bytes)
        {
            return Err(AccountFailure::too_large());
        }
        let response = self
            .prepare(method, path, token, body, maximum_request_bytes, accept)?
            .timeout(timeout)
            .send()
            .map_err(|error| AccountFailure::from_send_error(&error))?;
        read_detailed_response(response, maximum_response_bytes)
    }

    /// [`Self::request_detailed`] 的 JSON 版本：请求体由 `body` 序列化，2xx 的正文解成 `T`，解不出是 `account_unavailable`。
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn json_detailed<T: DeserializeOwned, B: Serialize>(
        &self,
        method: Method,
        path: &str,
        token: Option<&str>,
        body: Option<&B>,
        maximum_request_bytes: usize,
        maximum_response_bytes: usize,
        timeout: Duration,
    ) -> Result<T, AccountFailure> {
        let body = body
            .map(serde_json::to_vec)
            .transpose()
            .map_err(|_| AccountError::Invalid)?;
        let bytes = self.request_detailed(
            method,
            path,
            token,
            body,
            maximum_request_bytes,
            maximum_response_bytes,
            timeout,
            "application/json",
        )?;
        serde_json::from_slice(&bytes).map_err(|_| AccountError::Unavailable.into())
    }

    /// 给不经 [`Self::prepare`] 的请求（multipart 上传、流式下载）拼出地址，只允许发往构造时给定的源站。
    pub(super) fn url(&self, path: &str) -> Result<Url, AccountError> {
        if !path.starts_with("/v1/") || path.contains('\\') {
            return Err(AccountError::Invalid);
        }
        let url = self.origin.join(path).map_err(|_| AccountError::Invalid)?;
        if url.scheme() != self.origin.scheme()
            || url.host_str() != self.origin.host_str()
            || url.port_or_known_default() != self.origin.port_or_known_default()
        {
            return Err(AccountError::Invalid);
        }
        Ok(url)
    }

    /// 给下一次请求领取当前代理设置对应的连接池；已经开始的请求继续使用原连接池。
    pub(super) fn http(&self) -> Result<Client, AccountError> {
        let network = msime_client_core::network::current();
        let mut cached = self.client.lock().map_err(|_| AccountError::Unavailable)?;
        if let Some((installed, client)) = cached.as_ref() {
            if installed == &network {
                return Ok(client.clone());
            }
        }
        let client = build_http_client(&self.agent, &self.origin, &network)?;
        *cached = Some((network, client.clone()));
        Ok(client)
    }

    /// POSTs a JSON `body` to `path` without credentials and reports the response status with its `Retry-After` delay (seconds form only), for callers whose handling depends on the status itself rather than on a decoded document, such as telemetry delivery. The response body is discarded. A request that never got a response is `Err(Unavailable)`.
    pub(crate) fn post_for_status(
        &self,
        path: &str,
        body: Vec<u8>,
        maximum_request_bytes: usize,
        timeout: Duration,
    ) -> Result<(StatusCode, Option<Duration>), AccountError> {
        if !path.starts_with("/v1/") || path.contains('\\') || body.len() > maximum_request_bytes {
            return Err(AccountError::Invalid);
        }
        let url = self.origin.join(path).map_err(|_| AccountError::Invalid)?;
        let response = self
            .http()?
            .post(url)
            .header(reqwest::header::ACCEPT, "application/json")
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(body)
            .timeout(timeout)
            .send()
            .map_err(|_| AccountError::Unavailable)?;
        let retry_after = response
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.trim().parse::<u64>().ok())
            .map(Duration::from_secs);
        Ok((response.status(), retry_after))
    }

    fn empty<B: Serialize>(
        &self,
        method: Method,
        path: &str,
        token: Option<&str>,
        body: Option<&B>,
    ) -> Result<(), AccountError> {
        let body = body
            .map(serde_json::to_vec)
            .transpose()
            .map_err(|_| AccountError::Invalid)?;
        self.request(method, path, token, body).map(|_| ())
    }

    pub fn set_clipboard_enabled(
        &self,
        enabled: bool,
        access_token: &str,
    ) -> Result<(), AccountError> {
        #[derive(Serialize)]
        struct Body {
            enabled: bool,
        }
        self.empty(
            Method::PUT,
            "/v1/users/me/clipboard/settings",
            Some(access_token),
            Some(&Body { enabled }),
        )
    }

    /// 发出云剪贴板驱动要求的请求，状态码和正文原样交回（见 [`AccountApi::clipboard_e2e`]）。只接受云剪贴板自己的路径，响应上限放宽到 [`MAX_CLIPBOARD_RESPONSE_BYTES`]，装得下 50 条最长的密文。
    pub fn clipboard_e2e(
        &self,
        request: &HttpRequest,
        access_token: &str,
    ) -> Result<HttpResponse, AccountError> {
        let method = match request.method.as_str() {
            "GET" => Method::GET,
            "PUT" => Method::PUT,
            "POST" => Method::POST,
            "DELETE" => Method::DELETE,
            _ => return Err(AccountError::Invalid),
        };
        let path = request.path.as_str();
        let in_scope = path == CLIPBOARD_PATH
            || path
                .strip_prefix(CLIPBOARD_PATH)
                .is_some_and(|rest| rest.starts_with('/'));
        if !in_scope || path.contains("..") {
            return Err(AccountError::Invalid);
        }
        let body = request.body.as_ref().map(|body| body.as_bytes().to_vec());
        let response = self
            .prepare(
                method,
                path,
                Some(access_token),
                body,
                MAX_JSON_BYTES,
                "application/json",
            )?
            .timeout(Duration::from_secs(30))
            .send()
            .map_err(|_| AccountError::Unavailable)?;
        let status = response.status().as_u16();
        if !response.status().is_success() {
            // 错误正文只用来读 `error.code`，和其他账号请求一样只读开头几 KiB；读不出来时驱动按状态码处理。
            let body = crate::bounded_io::read_bounded(response, MAX_ERROR_BODY_BYTES)
                .ok()
                .and_then(|bytes| String::from_utf8(bytes).ok())
                .unwrap_or_default();
            return Ok(HttpResponse { status, body });
        }
        let bytes = read_bounded_response(response, MAX_CLIPBOARD_RESPONSE_BYTES)?;
        let body = String::from_utf8(bytes).map_err(|_| AccountError::Unavailable)?;
        Ok(HttpResponse { status, body })
    }

    pub fn delete_clipboard(
        &self,
        id: Option<&str>,
        access_token: &str,
    ) -> Result<(), AccountError> {
        if let Some(id) = id {
            validate_clipboard_id(id)?;
        }
        let path = id
            .map(|value| format!("/v1/users/me/clipboard/{value}"))
            .unwrap_or_else(|| "/v1/users/me/clipboard".to_owned());
        self.empty::<()>(Method::DELETE, &path, Some(access_token), None)
    }

    /// 置顶或取消置顶一条云剪贴板条目（见 [`AccountApi::pin_clipboard`]）。`id` 不是 64 位小写十六进制时不联网，答 `Invalid`。
    pub fn pin_clipboard(
        &self,
        id: &str,
        pinned: bool,
        access_token: &str,
    ) -> Result<(), AccountError> {
        #[derive(Serialize)]
        struct Body {
            pinned: bool,
        }
        validate_clipboard_id(id)?;
        self.empty(
            Method::PUT,
            &format!("{CLIPBOARD_PATH}/{id}/pin"),
            Some(access_token),
            Some(&Body { pinned }),
        )
    }

    /// 设置云剪贴板的保留天数（见 [`AccountApi::set_clipboard_retention`]）。不在 [`crate::clipboard_relay::RETENTION_DAYS`] 里的天数不联网，答 `Invalid`。
    pub fn set_clipboard_retention(
        &self,
        days: u32,
        access_token: &str,
    ) -> Result<(), AccountError> {
        #[derive(Serialize)]
        struct Body {
            days: u32,
        }
        if !crate::clipboard_relay::RETENTION_DAYS.contains(&days) {
            return Err(AccountError::Invalid);
        }
        self.empty(
            Method::PUT,
            &format!("{CLIPBOARD_PATH}/retention"),
            Some(access_token),
            Some(&Body { days }),
        )
    }

    pub fn dictionary(
        &self,
        kind: DictionaryKind,
        search: &str,
        offset: usize,
        access_token: &str,
    ) -> Result<AccountDictionaryPage, AccountError> {
        let path = crate::cloud::dictionary::dictionary_path(kind, offset, search)
            .ok_or(AccountError::Invalid)?;
        let page =
            self.json::<AccountDictionaryPage, ()>(Method::GET, &path, Some(access_token), None)?;
        validate_dictionary_page(&page, kind)?;
        Ok(page)
    }

    pub fn dictionary_catalog(
        &self,
        kind: DictionaryKind,
        code: &str,
        offset: usize,
        scheme: &str,
        profile: &str,
        access_token: &str,
    ) -> Result<AccountDictionaryCatalogPage, AccountError> {
        validate_dictionary_catalog_query(code, offset, scheme, profile)?;
        let path = dictionary_catalog_path(kind, code, offset, scheme, profile)?;
        let page = self.json::<AccountDictionaryCatalogPage, ()>(
            Method::GET,
            &path,
            Some(access_token),
            None,
        )?;
        validate_dictionary_catalog_page(&page, kind)?;
        Ok(page)
    }

    pub fn dictionary_changes(
        &self,
        after: i64,
        limit: usize,
        access_token: &str,
    ) -> Result<AccountDictionaryChangePage, AccountError> {
        if after < 0 || !(1..=100).contains(&limit) {
            return Err(AccountError::Invalid);
        }
        let page = self.json::<AccountDictionaryChangePage, ()>(
            Method::GET,
            &format!("/v1/users/me/dictionary/changes?after={after}&limit={limit}"),
            Some(access_token),
            None,
        )?;
        if page.changes.len() > limit {
            return Err(AccountError::Unavailable);
        }
        let mut cursor = after;
        for change in &page.changes {
            if change.revision <= cursor {
                return Err(AccountError::Unavailable);
            }
            if change.previous.as_ref().is_some_and(|entry| {
                validate_dictionary_entry(entry, entry.kind).is_err()
                    || entry.revision > change.revision
            }) || change.replacement.as_ref().is_some_and(|entry| {
                validate_dictionary_entry(entry, entry.kind).is_err()
                    || entry.revision > change.revision
            }) {
                return Err(AccountError::Unavailable);
            }
            cursor = change.revision;
        }
        if page.next != cursor || (page.has_more && page.changes.is_empty()) {
            return Err(AccountError::Unavailable);
        }
        Ok(page)
    }

    /// 云同步的词库探测：只取自 `after` 以来的第一条改动。翻页的 `next` 不能后退，有改动时必须前进，没有改动时必须停在 `after`，与 Android `SyncApi.dictionaryChangedSince` 的检查相同；不合规的响应是 `account_unavailable`。
    pub fn dictionary_probe(
        &self,
        after: i64,
        access_token: &str,
    ) -> Result<AccountDictionaryProbe, AccountFailure> {
        #[derive(Deserialize)]
        struct Change {
            revision: i64,
            #[serde(default)]
            reset: bool,
        }
        #[derive(Deserialize)]
        struct Page {
            changes: Vec<Change>,
            next: i64,
        }
        if after < 0 || !crate::text::is_lower_hex(access_token, 64) {
            return Err(AccountError::Invalid.into());
        }
        let page = self.json_detailed::<Page, ()>(
            Method::GET,
            &format!("/v1/users/me/dictionary/changes?after={after}&limit=1"),
            Some(access_token),
            None,
            MAX_JSON_BYTES,
            MAX_JSON_BYTES,
            Duration::from_secs(30),
        )?;
        let first = page.changes.first();
        let valid = match first {
            Some(change) => {
                page.changes.len() == 1 && change.revision > after && page.next == change.revision
            }
            None => page.next == after,
        };
        if !valid {
            return Err(AccountError::Unavailable.into());
        }
        Ok(AccountDictionaryProbe {
            changed: first.is_some(),
            revision: page.next,
            reset: first.is_some_and(|change| change.reset),
        })
    }

    pub fn dictionary_snapshot(&self, access_token: &str) -> Result<Vec<u8>, AccountError> {
        self.request_with_limit_timeout_accept(
            Method::GET,
            "/v1/users/me/dictionary/snapshot",
            Some(access_token),
            None,
            MAX_JSON_BYTES,
            MAX_DICTIONARY_SNAPSHOT_BYTES,
            Duration::from_secs(120),
            "application/x-ndjson",
        )
    }

    pub fn dictionary_snapshot_to_file(
        &self,
        destination: &Path,
        access_token: &str,
    ) -> Result<u64, AccountError> {
        self.snapshot_to_file_with(destination, access_token, error_from_response)
    }

    /// [`Self::dictionary_snapshot_to_file`] 的实现：非 2xx 的响应交给 `failed` 读成错误，两种错误类型（[`AccountError`] 和云同步用的 [`AccountFailure`]）共用同一段下载和落盘代码。
    fn snapshot_to_file_with<E: From<AccountError>>(
        &self,
        destination: &Path,
        access_token: &str,
        failed: impl FnOnce(Response) -> E,
    ) -> Result<u64, E> {
        if !destination.is_absolute() || !crate::text::is_lower_hex(access_token, 64) {
            return Err(AccountError::Invalid.into());
        }
        let parent = destination.parent().ok_or(AccountError::Invalid)?;
        if !parent.is_absolute() {
            return Err(AccountError::Invalid.into());
        }
        crate::storage::reject_symlink(parent).map_err(|_| AccountError::Invalid)?;
        let url = self
            .origin
            .join("/v1/users/me/dictionary/snapshot")
            .map_err(|_| AccountError::Invalid)?;
        let mut response = self
            .http()?
            .get(url)
            .header(reqwest::header::ACCEPT, "application/x-ndjson")
            .bearer_auth(access_token)
            .timeout(Duration::from_secs(120))
            .send()
            .map_err(|_| AccountError::Unavailable)?;
        if !response.status().is_success() {
            return Err(failed(response));
        }
        if response
            .content_length()
            .is_some_and(|length| length > MAX_DICTIONARY_SNAPSHOT_BYTES as u64)
        {
            return Err(AccountError::Unavailable.into());
        }
        #[cfg(unix)]
        {
            let directory = crate::storage::open_private_directory(parent)
                .map_err(|_| AccountError::Unavailable)?;
            let name = destination.file_name().ok_or(AccountError::Invalid)?;
            let (_, bytes) =
                crate::storage::write_private_file_at_with(&directory, name, |mut file| {
                    let bytes = std::io::copy(
                        &mut response
                            .by_ref()
                            .take((MAX_DICTIONARY_SNAPSHOT_BYTES + 1) as u64),
                        &mut file,
                    )?;
                    if bytes == 0 || bytes > MAX_DICTIONARY_SNAPSHOT_BYTES as u64 {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "dictionary snapshot size is invalid",
                        ));
                    }
                    file.flush()?;
                    Ok((file, bytes))
                })
                .map_err(|_| AccountError::Unavailable)?;
            Ok(bytes)
        }
        #[cfg(not(unix))]
        {
            let mut temporary = tempfile::Builder::new()
                .prefix("msime-snapshot-")
                .tempfile_in(parent)
                .map_err(|_| AccountError::Unavailable)?;
            let bytes = std::io::copy(
                &mut response
                    .by_ref()
                    .take((MAX_DICTIONARY_SNAPSHOT_BYTES + 1) as u64),
                temporary.as_file_mut(),
            )
            .map_err(|_| AccountError::Unavailable)?;
            if bytes == 0 || bytes > MAX_DICTIONARY_SNAPSHOT_BYTES as u64 {
                return Err(AccountError::Unavailable.into());
            }
            temporary
                .as_file_mut()
                .flush()
                .and_then(|_| temporary.as_file().sync_all())
                .map_err(|_| AccountError::Unavailable)?;
            crate::storage::persist_replacing(temporary, destination)
                .map_err(|_| AccountError::Unavailable)?;
            Ok(bytes)
        }
    }

    pub fn restore_dictionary_snapshot(
        &self,
        snapshot: &[u8],
        revision: i64,
        access_token: &str,
    ) -> Result<AccountDictionarySnapshotRestore, AccountError> {
        if revision < 0
            || snapshot.is_empty()
            || snapshot.len() > MAX_DICTIONARY_SNAPSHOT_BYTES
            || snapshot.contains(&0)
            || !crate::text::is_lower_hex(access_token, 64)
        {
            return Err(AccountError::Invalid);
        }
        let url = self
            .origin
            .join(&format!(
                "/v1/users/me/dictionary/snapshot?revision={revision}"
            ))
            .map_err(|_| AccountError::Invalid)?;
        let response = self
            .http()?
            .put(url)
            .header(reqwest::header::ACCEPT, "application/json")
            .header(reqwest::header::CONTENT_TYPE, "application/x-ndjson")
            .bearer_auth(access_token)
            .timeout(Duration::from_secs(130))
            .body(snapshot.to_vec())
            .send()
            .map_err(|_| AccountError::Unavailable)?;
        Self::decode_snapshot_restore(response, revision)
    }

    /// Stream a validated private snapshot file as the request body without loading it in memory.
    pub fn restore_dictionary_snapshot_file(
        &self,
        snapshot: &Path,
        revision: i64,
        access_token: &str,
    ) -> Result<AccountDictionarySnapshotRestore, AccountError> {
        let response = self.send_snapshot_file(snapshot, revision, access_token)?;
        Self::decode_snapshot_restore(response, revision)
    }

    /// 与 [`Self::restore_dictionary_snapshot_file`] 相同，失败时是带状态码、服务端错误码和 `Retry-After` 的 [`AccountFailure`]：云同步要分清 409 `revision_conflict`、429 限流和 503 `snapshot_restore_busy`。
    pub fn restore_dictionary_snapshot_file_detailed(
        &self,
        snapshot: &Path,
        revision: i64,
        access_token: &str,
    ) -> Result<AccountDictionarySnapshotRestore, AccountFailure> {
        let response = self.send_snapshot_file(snapshot, revision, access_token)?;
        if !response.status().is_success() {
            return Err(failure_from_response(response));
        }
        Self::decode_snapshot_restore(response, revision).map_err(Into::into)
    }

    /// 校验快照文件并把它作为请求体流式发出去，返回原样的响应。
    fn send_snapshot_file(
        &self,
        snapshot: &Path,
        revision: i64,
        access_token: &str,
    ) -> Result<Response, AccountError> {
        if revision < 0 || !snapshot.is_absolute() || !crate::text::is_lower_hex(access_token, 64) {
            return Err(AccountError::Invalid);
        }
        let parent = snapshot.parent().ok_or(AccountError::Invalid)?;
        if !parent.is_absolute() {
            return Err(AccountError::Invalid);
        }
        crate::storage::reject_symlink(parent).map_err(|_| AccountError::Invalid)?;
        let metadata = std::fs::symlink_metadata(snapshot).map_err(|_| AccountError::Invalid)?;
        if !metadata.file_type().is_file() {
            return Err(AccountError::Invalid);
        }
        let file =
            crate::storage::open_private_file_in(snapshot).map_err(|_| AccountError::Invalid)?;
        let bytes = file.metadata().map_err(|_| AccountError::Invalid)?.len();
        if bytes == 0 || bytes > MAX_DICTIONARY_SNAPSHOT_BYTES as u64 {
            return Err(AccountError::Invalid);
        }
        let url = self
            .origin
            .join(&format!(
                "/v1/users/me/dictionary/snapshot?revision={revision}"
            ))
            .map_err(|_| AccountError::Invalid)?;
        self.http()?
            .put(url)
            .header(reqwest::header::ACCEPT, "application/json")
            .header(reqwest::header::CONTENT_TYPE, "application/x-ndjson")
            .bearer_auth(access_token)
            .timeout(Duration::from_secs(600))
            .body(reqwest::blocking::Body::sized(file, bytes))
            .send()
            .map_err(|_| AccountError::Unavailable)
    }

    fn decode_snapshot_restore(
        response: Response,
        revision: i64,
    ) -> Result<AccountDictionarySnapshotRestore, AccountError> {
        let media_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.split(';').next())
            .map(str::trim);
        if !media_type.is_some_and(|value| value.eq_ignore_ascii_case("application/json")) {
            return Err(AccountError::Unavailable);
        }
        let bytes = read_bounded_response(response, MAX_JSON_BYTES)?;
        let result: AccountDictionarySnapshotRestore =
            serde_json::from_slice(&bytes).map_err(|_| AccountError::Unavailable)?;
        if !result.reset || result.revision <= revision {
            return Err(AccountError::Unavailable);
        }
        Ok(result)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn edit_dictionary_catalog(
        &self,
        kind: DictionaryKind,
        code: &str,
        word: &str,
        revision: i64,
        replacement: Option<(&str, &str, i64)>,
        access_token: &str,
    ) -> Result<AccountDictionaryChange, AccountError> {
        validate_dictionary_catalog_identity(kind, code, word)?;
        if revision < 0 {
            return Err(AccountError::Invalid);
        }
        if let Some((replacement_code, replacement_word, replacement_weight)) = replacement {
            validate_new_dictionary_value(
                kind,
                replacement_code,
                replacement_word,
                replacement_weight,
            )?;
        }
        #[derive(Serialize)]
        struct Identity<'a> {
            code: &'a str,
            word: &'a str,
        }
        #[derive(Serialize)]
        struct Body<'a> {
            revision: i64,
            previous: Identity<'a>,
            replacement: Option<Replacement<'a>>,
        }
        #[derive(Serialize)]
        struct Replacement<'a> {
            code: &'a str,
            word: &'a str,
            weight: i64,
        }
        let replacement_body =
            replacement.map(|(replacement_code, replacement_word, weight)| Replacement {
                code: replacement_code,
                word: replacement_word,
                weight,
            });
        let change = self.json(
            Method::POST,
            &format!(
                "/v1/users/me/dictionaries/{}/edit",
                crate::cloud::dictionary::kind_path(kind)
            ),
            Some(access_token),
            Some(&Body {
                revision,
                previous: Identity { code, word },
                replacement: replacement_body,
            }),
        )?;
        validate_dictionary_change(&change, kind)?;
        Ok(change)
    }

    pub fn personal_candidates(
        &self,
        query: &AccountCandidateQuery,
        access_token: &str,
    ) -> Result<AccountPersonalCandidates, AccountError> {
        validate_candidate_query(query)?;
        let result: AccountPersonalCandidates = self.json(
            Method::POST,
            "/v1/users/me/dictionary/candidates",
            Some(access_token),
            Some(query),
        )?;
        validate_personal_candidates(&result)?;
        Ok(result)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn rank_candidate(
        &self,
        query: &AccountCandidateQuery,
        code: &str,
        word: &str,
        revision: i64,
        mode: &str,
        linear_step: i64,
        trigger_count: i64,
        force_top: bool,
        access_token: &str,
    ) -> Result<AccountRankingResult, AccountError> {
        validate_candidate_query(query)?;
        validate_candidate_value(query, code, word)?;
        validate_ranking_arguments(query, revision, mode, linear_step, trigger_count)?;
        #[derive(Serialize)]
        struct Action<'a> {
            code: &'a str,
            word: &'a str,
            mode: &'a str,
            linear_step: i64,
            trigger_count: i64,
            force_top: bool,
        }
        #[derive(Serialize)]
        struct Body<'a> {
            revision: i64,
            query: &'a AccountCandidateQuery,
            action: Action<'a>,
        }
        let result = self.json(
            Method::POST,
            "/v1/users/me/dictionary/ranking",
            Some(access_token),
            Some(&Body {
                revision,
                query,
                action: Action {
                    code,
                    word,
                    mode,
                    linear_step,
                    trigger_count,
                    force_top,
                },
            }),
        )?;
        validate_ranking_result(&result)?;
        Ok(result)
    }

    pub fn remove_candidate(
        &self,
        query: &AccountCandidateQuery,
        code: &str,
        word: &str,
        revision: i64,
        access_token: &str,
    ) -> Result<AccountDictionaryChange, AccountError> {
        validate_candidate_query(query)?;
        validate_candidate_value(query, code, word)?;
        if revision < 0 || query.kind == "quick" {
            return Err(AccountError::Invalid);
        }
        #[derive(Serialize)]
        struct Body<'a> {
            revision: i64,
            query: &'a AccountCandidateQuery,
            code: &'a str,
            word: &'a str,
        }
        let change = self.json(
            Method::DELETE,
            "/v1/users/me/dictionary/candidates",
            Some(access_token),
            Some(&Body {
                revision,
                query,
                code,
                word,
            }),
        )?;
        validate_dictionary_change(&change, dictionary_kind_for_candidate(query)?)?;
        Ok(change)
    }

    pub fn fixed_positions(
        &self,
        context: &str,
        offset: usize,
        access_token: &str,
    ) -> Result<AccountFixedPositions, AccountError> {
        validate_bounded_text(context, 1024)?;
        if offset > 1_000_000 {
            return Err(AccountError::Invalid);
        }
        let path = format!(
            "/v1/users/me/dictionary/positions?context={}&offset={offset}&limit=100",
            crate::cloud::dictionary::percent_encode(context)
        );
        let result =
            self.json::<AccountFixedPositions, ()>(Method::GET, &path, Some(access_token), None)?;
        validate_fixed_positions(&result)?;
        Ok(result)
    }

    pub fn set_fixed_position(
        &self,
        context: &str,
        code: &str,
        word: &str,
        position: Option<i64>,
        revision: i64,
        access_token: &str,
    ) -> Result<AccountDictionaryRevision, AccountError> {
        validate_bounded_text(context, 1024)?;
        validate_bounded_text(code, 256)?;
        validate_bounded_text(word, 1024)?;
        if revision < 0 || position.is_some_and(|value| !(1..=5).contains(&value)) {
            return Err(AccountError::Invalid);
        }
        #[derive(Serialize)]
        struct Body<'a> {
            context: &'a str,
            code: &'a str,
            word: &'a str,
            position: Option<i64>,
            revision: i64,
        }
        let result: AccountDictionaryRevision = self.json(
            if position.is_some() {
                Method::PUT
            } else {
                Method::DELETE
            },
            "/v1/users/me/dictionary/positions",
            Some(access_token),
            Some(&Body {
                context,
                code,
                word,
                position,
                revision,
            }),
        )?;
        if result.revision < 0 {
            return Err(AccountError::Unavailable);
        }
        Ok(result)
    }

    pub fn add_dictionary(
        &self,
        kind: DictionaryKind,
        code: &str,
        word: &str,
        weight: i64,
        access_token: &str,
    ) -> Result<AccountDictionaryChange, AccountError> {
        validate_new_dictionary_value(kind, code, word, weight)?;
        #[derive(Serialize)]
        struct Body<'a> {
            code: &'a str,
            word: &'a str,
            weight: i64,
        }
        let path =
            crate::cloud::dictionary::mutation_path(kind, "add").ok_or(AccountError::Invalid)?;
        let change = self.json(
            Method::POST,
            &path,
            Some(access_token),
            Some(&Body { code, word, weight }),
        )?;
        validate_dictionary_change(&change, kind)?;
        Ok(change)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update_dictionary(
        &self,
        kind: DictionaryKind,
        id: &str,
        code: &str,
        word: &str,
        weight: i64,
        revision: i64,
        access_token: &str,
    ) -> Result<AccountDictionaryChange, AccountError> {
        validate_dictionary_id(id)?;
        validate_new_dictionary_value(kind, code, word, weight)?;
        if revision <= 0 {
            return Err(AccountError::Invalid);
        }
        #[derive(Serialize)]
        struct Body<'a> {
            code: &'a str,
            word: &'a str,
            weight: i64,
            revision: i64,
        }
        let path = format!(
            "/v1/users/me/dictionaries/{}/{}",
            crate::cloud::dictionary::kind_path(kind),
            id
        );
        let change = self.json(
            Method::PUT,
            &path,
            Some(access_token),
            Some(&Body {
                code,
                word,
                weight,
                revision,
            }),
        )?;
        validate_dictionary_change(&change, kind)?;
        Ok(change)
    }

    pub fn delete_dictionary(
        &self,
        kind: DictionaryKind,
        id: &str,
        revision: i64,
        access_token: &str,
    ) -> Result<AccountDictionaryChange, AccountError> {
        validate_dictionary_id(id)?;
        if revision <= 0 {
            return Err(AccountError::Invalid);
        }
        #[derive(Serialize)]
        struct Body {
            revision: i64,
        }
        let path = format!(
            "/v1/users/me/dictionaries/{}/{}",
            crate::cloud::dictionary::kind_path(kind),
            id
        );
        let change = self.json(
            Method::DELETE,
            &path,
            Some(access_token),
            Some(&Body { revision }),
        )?;
        validate_dictionary_change(&change, kind)?;
        Ok(change)
    }

    pub fn import_dictionary(
        &self,
        kind: DictionaryKind,
        format: &str,
        text: &str,
        access_token: &str,
    ) -> Result<AccountDictionaryImportResult, AccountError> {
        validate_dictionary_import(kind, format, text)?;
        let (path, body) = if format == "hans" {
            (
                crate::cloud::dictionary::mutation_path(kind, "import-hans")
                    .ok_or(AccountError::Invalid)?,
                serde_json::json!({ "text": text, "weight": 100000_i64 }),
            )
        } else {
            (
                crate::cloud::dictionary::mutation_path(kind, "import")
                    .ok_or(AccountError::Invalid)?,
                serde_json::json!({ "text": text, "format": format }),
            )
        };
        let result = self.json(Method::POST, &path, Some(access_token), Some(&body))?;
        validate_dictionary_import_result(&result)?;
        Ok(result)
    }

    pub fn export_dictionary(
        &self,
        kind: DictionaryKind,
        format: &str,
        access_token: &str,
    ) -> Result<AccountDictionaryExport, AccountError> {
        if !matches!(format, "standard" | "windows") {
            return Err(AccountError::Invalid);
        }
        let path = format!(
            "/v1/users/me/dictionaries/{}/export?format={format}",
            crate::cloud::dictionary::kind_path(kind)
        );
        let bytes = self.request_with_limit_timeout_accept(
            Method::GET,
            &path,
            Some(access_token),
            None,
            MAX_JSON_BYTES,
            MAX_DICTIONARY_EXPORT_BYTES,
            Duration::from_secs(600),
            "text/plain",
        )?;
        let text = String::from_utf8(bytes).map_err(|_| AccountError::Unavailable)?;
        if text.is_empty() || text.contains('\0') {
            return Err(AccountError::Unavailable);
        }
        Ok(AccountDictionaryExport {
            text,
            filename: format!(
                "dictionary-{}.tsv",
                crate::cloud::dictionary::kind_path(kind)
            ),
        })
    }
}

/// `/v1/chat/completions` 的请求体。[`BackendAccountClient`] 发请求和会话层的本地预检用同一份，量的是同一串字节。
#[derive(Serialize)]
struct ChatBody<'a> {
    messages: &'a [AccountChatMessage],
    model: &'a str,
    max_tokens: u16,
    stream: bool,
}

impl<'a> ChatBody<'a> {
    fn new(messages: &'a [AccountChatMessage], model: &'a str, stream: bool) -> Self {
        Self {
            messages,
            model,
            max_tokens: 2048,
            stream,
        }
    }
}

/// 序列化后的聊天请求体（非流式）。流式的请求体只是 `false` 换成 `true`，短一个字节，按这份量出的上限对两者都成立。
pub(super) fn chat_request_body(
    messages: &[AccountChatMessage],
    model: &str,
) -> Result<Vec<u8>, AccountError> {
    chat_body_bytes(messages, model, false)
}

/// 序列化后的聊天请求体，`stream` 决定服务端回 JSON 还是 SSE。
pub(super) fn chat_body_bytes(
    messages: &[AccountChatMessage],
    model: &str,
    stream: bool,
) -> Result<Vec<u8>, AccountError> {
    serde_json::to_vec(&ChatBody::new(messages, model, stream)).map_err(|_| AccountError::Invalid)
}

/// 非流式回答 `{choices:[{message:{role,content}}]}` 里的回复：取第一个选项，角色必须是 assistant，正文非空白、不超过 16 KiB、不含换行、回车和制表符以外的控制字符，否则是 `account_unavailable`。流式请求遇到没按 SSE 回答的响应时也按它读。
pub(super) fn chat_reply(bytes: &[u8]) -> Result<String, AccountFailure> {
    #[derive(Deserialize)]
    struct Response {
        choices: Vec<Choice>,
    }
    #[derive(Deserialize)]
    struct Choice {
        message: AccountChatMessage,
    }
    let response: Response =
        serde_json::from_slice(bytes).map_err(|_| AccountError::Unavailable)?;
    let reply = response
        .choices
        .into_iter()
        .next()
        .map(|choice| choice.message)
        .ok_or(AccountError::Unavailable)?;
    if reply.role != "assistant"
        || reply.content.trim().is_empty()
        || reply.content.len() > MAX_CHAT_RESPONSE_BYTES
        || crate::text::has_disallowed_control(&reply.content)
    {
        return Err(AccountError::Unavailable.into());
    }
    Ok(reply.content)
}

/// 对话请求在取令牌之前的检查，与 [`BackendAccountSession::chat_checked`] 相同：不合法是 `account_invalid`，序列化后的请求体超过服务端 64 KiB 的整包上限是 `account_too_large`。
pub fn prepare_chat(messages: &[AccountChatMessage], model: &str) -> Result<(), AccountFailure> {
    validate_chat_request(messages, model)?;
    if chat_request_body(messages, model)?.len() > MAX_CHAT_REQUEST_BYTES {
        return Err(AccountFailure::too_large());
    }
    Ok(())
}

impl AccountApi for BackendAccountClient {
    fn providers(&self) -> Result<std::collections::HashMap<String, bool>, AccountError> {
        #[derive(Deserialize)]
        struct Providers {
            providers: std::collections::HashMap<String, bool>,
        }
        let value = self.json::<Providers, ()>(Method::GET, "/v1/auth/providers", None, None)?;
        validate_providers(&value.providers)?;
        Ok(value.providers)
    }

    fn challenge(&self, provider: &str, target: &str) -> Result<AccountChallenge, AccountError> {
        validate_challenge_request(provider, target)?;
        #[derive(Serialize)]
        struct Body<'a> {
            provider: &'a str,
            target: &'a str,
            purpose: &'static str,
        }
        let challenge = self.json(
            Method::POST,
            "/v1/auth/challenges",
            None,
            Some(&Body {
                provider,
                target,
                purpose: "login",
            }),
        )?;
        validate_challenge(&challenge)?;
        Ok(challenge)
    }

    fn login(&self, challenge: &str, credential: &str) -> Result<AccountTokens, AccountError> {
        validate_login_request(challenge, credential)?;
        #[derive(Serialize)]
        struct Body<'a> {
            challenge_id: &'a str,
            credential: &'a str,
        }
        let tokens = self.json(
            Method::POST,
            "/v1/auth/login",
            None,
            Some(&Body {
                challenge_id: challenge,
                credential,
            }),
        )?;
        validate_tokens(&tokens)?;
        Ok(tokens)
    }

    fn apple_web_challenge(
        &self,
        code_challenge: &str,
        app: &str,
    ) -> Result<AccountAppleWebChallenge, AccountError> {
        if !super::apple_web::valid_base64url_token(code_challenge, 32)
            || !super::apple_web::valid_apple_web_app(app)
        {
            return Err(AccountError::Invalid);
        }
        #[derive(Serialize)]
        struct Body<'a> {
            code_challenge: &'a str,
            code_challenge_method: &'static str,
            app: &'a str,
            purpose: &'static str,
        }
        let challenge = self.json(
            Method::POST,
            "/v1/auth/apple/web",
            None,
            Some(&Body {
                code_challenge,
                code_challenge_method: "S256",
                app,
                purpose: "login",
            }),
        )?;
        super::apple_web::validate_apple_web_challenge(&challenge)?;
        Ok(challenge)
    }

    fn apple_web_login(
        &self,
        grant: &str,
        code_verifier: &str,
    ) -> Result<AccountTokens, AccountError> {
        if !super::apple_web::valid_apple_web_grant(grant)
            || !super::apple_web::valid_code_verifier(code_verifier)
        {
            return Err(AccountError::Invalid);
        }
        #[derive(Serialize)]
        struct Body<'a> {
            grant: &'a str,
            code_verifier: &'a str,
        }
        let tokens = self.json(
            Method::POST,
            "/v1/auth/apple/web/login",
            None,
            Some(&Body {
                grant,
                code_verifier,
            }),
        )?;
        validate_tokens(&tokens)?;
        Ok(tokens)
    }

    fn link_challenge(
        &self,
        provider: &str,
        target: &str,
        access_token: &str,
    ) -> Result<AccountChallenge, AccountFailure> {
        validate_challenge_request(provider, target)?;
        #[derive(Serialize)]
        struct Body<'a> {
            provider: &'a str,
            target: &'a str,
            purpose: &'static str,
        }
        let challenge = self.json_detailed(
            Method::POST,
            "/v1/auth/challenges",
            Some(access_token),
            Some(&Body {
                provider,
                target,
                purpose: "link",
            }),
            MAX_JSON_BYTES,
            MAX_JSON_BYTES,
            Duration::from_secs(30),
        )?;
        validate_challenge(&challenge)?;
        Ok(challenge)
    }

    fn link_login(
        &self,
        challenge: &str,
        credential: &str,
        access_token: &str,
    ) -> Result<AccountTokens, AccountFailure> {
        validate_login_request(challenge, credential)?;
        #[derive(Serialize)]
        struct Body<'a> {
            challenge_id: &'a str,
            credential: &'a str,
        }
        let tokens = self.json_detailed(
            Method::POST,
            "/v1/auth/login",
            Some(access_token),
            Some(&Body {
                challenge_id: challenge,
                credential,
            }),
            MAX_JSON_BYTES,
            MAX_JSON_BYTES,
            Duration::from_secs(30),
        )?;
        validate_tokens(&tokens)?;
        Ok(tokens)
    }

    fn apple_web_link_challenge(
        &self,
        code_challenge: &str,
        app: &str,
        access_token: &str,
    ) -> Result<AccountAppleWebChallenge, AccountFailure> {
        if !super::apple_web::valid_base64url_token(code_challenge, 32)
            || !super::apple_web::valid_apple_web_app(app)
        {
            return Err(AccountError::Invalid.into());
        }
        #[derive(Serialize)]
        struct Body<'a> {
            code_challenge: &'a str,
            code_challenge_method: &'static str,
            app: &'a str,
            purpose: &'static str,
        }
        let challenge = self.json_detailed(
            Method::POST,
            "/v1/auth/apple/web",
            Some(access_token),
            Some(&Body {
                code_challenge,
                code_challenge_method: "S256",
                app,
                purpose: "link",
            }),
            MAX_JSON_BYTES,
            MAX_JSON_BYTES,
            Duration::from_secs(30),
        )?;
        super::apple_web::validate_apple_web_challenge(&challenge)?;
        Ok(challenge)
    }

    fn apple_web_link_login(
        &self,
        grant: &str,
        code_verifier: &str,
        access_token: &str,
    ) -> Result<AccountTokens, AccountFailure> {
        if !super::apple_web::valid_apple_web_grant(grant)
            || !super::apple_web::valid_code_verifier(code_verifier)
        {
            return Err(AccountError::Invalid.into());
        }
        #[derive(Serialize)]
        struct Body<'a> {
            grant: &'a str,
            code_verifier: &'a str,
        }
        let tokens = self.json_detailed(
            Method::POST,
            "/v1/auth/apple/web/login",
            Some(access_token),
            Some(&Body {
                grant,
                code_verifier,
            }),
            MAX_JSON_BYTES,
            MAX_JSON_BYTES,
            Duration::from_secs(30),
        )?;
        validate_tokens(&tokens)?;
        Ok(tokens)
    }

    fn refresh(&self, refresh_token: &str) -> Result<AccountTokens, AccountError> {
        if !crate::text::is_lower_hex(refresh_token, 64) {
            return Err(AccountError::Invalid);
        }
        #[derive(Serialize)]
        struct Body<'a> {
            refresh_token: &'a str,
        }
        let tokens = self.json(
            Method::POST,
            "/v1/auth/refresh",
            None,
            Some(&Body { refresh_token }),
        )?;
        validate_tokens(&tokens)?;
        Ok(tokens)
    }

    fn profile(&self, access_token: &str) -> Result<AccountProfile, AccountError> {
        if !crate::text::is_lower_hex(access_token, 64) {
            return Err(AccountError::Invalid);
        }
        let profile =
            self.json::<AccountProfile, ()>(Method::GET, "/v1/users/me", Some(access_token), None)?;
        validate_profile(&profile)?;
        Ok(profile)
    }

    fn rename(&self, display_name: &str, access_token: &str) -> Result<(), AccountError> {
        validate_display_name(display_name)?;
        #[derive(Serialize)]
        struct Body<'a> {
            display_name: &'a str,
        }
        self.empty(
            Method::PATCH,
            "/v1/users/me",
            Some(access_token),
            Some(&Body { display_name }),
        )
    }

    fn logout(&self, access_token: &str, all: bool) -> Result<(), AccountError> {
        #[derive(Serialize)]
        struct Body {
            all: bool,
        }
        self.empty(
            Method::POST,
            "/v1/auth/logout",
            Some(access_token),
            Some(&Body { all }),
        )
    }

    fn delete_account(&self, access_token: &str) -> Result<(), AccountError> {
        self.delete_account_detailed(access_token)
            .map_err(|failure| failure.error)
    }

    fn delete_account_detailed(&self, access_token: &str) -> Result<(), AccountFailure> {
        self.request_detailed(
            Method::DELETE,
            "/v1/users/me",
            Some(access_token),
            None,
            MAX_JSON_BYTES,
            MAX_JSON_BYTES,
            Duration::from_secs(30),
            "application/json",
        )
        .map(|_| ())
    }

    fn upload_avatar(
        &self,
        image: &AccountAvatarImage,
        access_token: &str,
    ) -> Result<(), AccountError> {
        if !crate::text::is_lower_hex(access_token, 64)
            || !matches!(image.content_type, "image/png" | "image/jpeg")
            || image.bytes.is_empty()
            || image.bytes.len() as u64 > MAX_ACCOUNT_AVATAR_UPLOAD_BYTES
        {
            return Err(AccountError::Invalid);
        }
        let url = self
            .origin
            .join("/v1/users/me/avatar")
            .map_err(|_| AccountError::Invalid)?;
        // The body is the image itself, not JSON, so this does not go through `request`, which only sends JSON.
        let response = self
            .http()?
            .put(url)
            .header(reqwest::header::ACCEPT, "application/json")
            .header(reqwest::header::CONTENT_TYPE, image.content_type)
            .bearer_auth(access_token)
            .timeout(Duration::from_secs(60))
            .body(image.bytes.clone())
            .send()
            .map_err(|_| AccountError::Unavailable)?;
        read_bounded_response(response, MAX_JSON_BYTES).map(|_| ())
    }

    fn delete_avatar(&self, access_token: &str) -> Result<(), AccountError> {
        self.empty::<()>(
            Method::DELETE,
            "/v1/users/me/avatar",
            Some(access_token),
            None,
        )
    }

    fn chat_models(&self, access_token: &str) -> Result<AccountChatModels, AccountError> {
        self.chat_models_detailed(access_token)
            .map_err(|failure| failure.error)
    }

    fn chat(
        &self,
        messages: &[AccountChatMessage],
        model: &str,
        access_token: &str,
    ) -> Result<String, AccountError> {
        self.chat_detailed(messages, model, access_token)
            .map_err(|failure| failure.error)
    }

    fn chat_models_detailed(
        &self,
        access_token: &str,
    ) -> Result<AccountChatModels, AccountFailure> {
        if !crate::text::is_lower_hex(access_token, 64) {
            return Err(AccountError::Invalid.into());
        }
        let models = self.json_detailed::<AccountChatModels, ()>(
            Method::GET,
            "/v1/models",
            Some(access_token),
            None,
            MAX_JSON_BYTES,
            MAX_JSON_BYTES,
            Duration::from_secs(30),
        )?;
        validate_chat_models(&models)?;
        Ok(models)
    }

    fn chat_detailed(
        &self,
        messages: &[AccountChatMessage],
        model: &str,
        access_token: &str,
    ) -> Result<String, AccountFailure> {
        if !crate::text::is_lower_hex(access_token, 64) {
            return Err(AccountError::Invalid.into());
        }
        validate_chat_request(messages, model)?;
        let body = chat_request_body(messages, model)?;
        if body.len() > MAX_CHAT_REQUEST_BYTES {
            return Err(AccountFailure::too_large());
        }
        let bytes = self.request_detailed(
            Method::POST,
            CHAT_PATH,
            Some(access_token),
            Some(body),
            MAX_CHAT_REQUEST_BYTES,
            MAX_CHAT_RESPONSE_BYTES,
            CHAT_TIMEOUT,
            "application/json",
        )?;
        chat_reply(&bytes)
    }

    fn chat_stream_detailed(
        &self,
        messages: &[AccountChatMessage],
        model: &str,
        access_token: &str,
        cancel: &ChatStreamCancel,
        on_delta: &mut dyn FnMut(&str),
    ) -> Result<String, AccountFailure> {
        self.stream_chat(messages, model, access_token, cancel, on_delta)
    }

    fn preference_schema(
        &self,
        access_token: &str,
    ) -> Result<AccountPreferenceSchema, AccountError> {
        if !crate::text::is_lower_hex(access_token, 64) {
            return Err(AccountError::Invalid);
        }
        let schema = self.json::<AccountPreferenceSchema, ()>(
            Method::GET,
            "/v1/users/me/preferences/schema",
            Some(access_token),
            None,
        )?;
        validate_preference_schema(&schema)?;
        Ok(schema)
    }

    fn preferences(&self, access_token: &str) -> Result<AccountPreferences, AccountError> {
        if !crate::text::is_lower_hex(access_token, 64) {
            return Err(AccountError::Invalid);
        }
        let preferences = self.json::<AccountPreferences, ()>(
            Method::GET,
            "/v1/users/me/preferences",
            Some(access_token),
            None,
        )?;
        validate_account_preferences(&preferences)?;
        Ok(preferences)
    }

    fn put_preferences(
        &self,
        preferences: &AccountPreferences,
        access_token: &str,
    ) -> Result<AccountPreferences, AccountError> {
        if !crate::text::is_lower_hex(access_token, 64) {
            return Err(AccountError::Invalid);
        }
        validate_account_preferences(preferences)?;
        let updated = self.json(
            Method::PUT,
            "/v1/users/me/preferences",
            Some(access_token),
            Some(preferences),
        )?;
        validate_account_preferences(&updated)?;
        Ok(updated)
    }

    fn clipboard_e2e(
        &self,
        request: &HttpRequest,
        access_token: &str,
    ) -> Result<HttpResponse, AccountError> {
        self.clipboard_e2e(request, access_token)
    }

    fn set_clipboard_enabled(&self, enabled: bool, access_token: &str) -> Result<(), AccountError> {
        self.set_clipboard_enabled(enabled, access_token)
    }

    fn delete_clipboard(&self, id: Option<&str>, access_token: &str) -> Result<(), AccountError> {
        self.delete_clipboard(id, access_token)
    }

    fn pin_clipboard(
        &self,
        id: &str,
        pinned: bool,
        access_token: &str,
    ) -> Result<(), AccountError> {
        self.pin_clipboard(id, pinned, access_token)
    }

    fn set_clipboard_retention(&self, days: u32, access_token: &str) -> Result<(), AccountError> {
        self.set_clipboard_retention(days, access_token)
    }

    fn dictionary(
        &self,
        kind: DictionaryKind,
        search: &str,
        offset: usize,
        access_token: &str,
    ) -> Result<AccountDictionaryPage, AccountError> {
        self.dictionary(kind, search, offset, access_token)
    }

    fn dictionary_catalog(
        &self,
        kind: DictionaryKind,
        code: &str,
        offset: usize,
        scheme: &str,
        profile: &str,
        access_token: &str,
    ) -> Result<AccountDictionaryCatalogPage, AccountError> {
        self.dictionary_catalog(kind, code, offset, scheme, profile, access_token)
    }

    #[allow(clippy::too_many_arguments)]
    fn edit_dictionary_catalog(
        &self,
        kind: DictionaryKind,
        code: &str,
        word: &str,
        revision: i64,
        replacement: Option<(&str, &str, i64)>,
        access_token: &str,
    ) -> Result<AccountDictionaryChange, AccountError> {
        self.edit_dictionary_catalog(kind, code, word, revision, replacement, access_token)
    }

    fn personal_candidates(
        &self,
        query: &AccountCandidateQuery,
        access_token: &str,
    ) -> Result<AccountPersonalCandidates, AccountError> {
        self.personal_candidates(query, access_token)
    }

    #[allow(clippy::too_many_arguments)]
    fn rank_candidate(
        &self,
        query: &AccountCandidateQuery,
        code: &str,
        word: &str,
        revision: i64,
        mode: &str,
        linear_step: i64,
        trigger_count: i64,
        force_top: bool,
        access_token: &str,
    ) -> Result<AccountRankingResult, AccountError> {
        self.rank_candidate(
            query,
            code,
            word,
            revision,
            mode,
            linear_step,
            trigger_count,
            force_top,
            access_token,
        )
    }

    fn remove_candidate(
        &self,
        query: &AccountCandidateQuery,
        code: &str,
        word: &str,
        revision: i64,
        access_token: &str,
    ) -> Result<AccountDictionaryChange, AccountError> {
        self.remove_candidate(query, code, word, revision, access_token)
    }

    fn fixed_positions(
        &self,
        context: &str,
        offset: usize,
        access_token: &str,
    ) -> Result<AccountFixedPositions, AccountError> {
        self.fixed_positions(context, offset, access_token)
    }

    fn set_fixed_position(
        &self,
        context: &str,
        code: &str,
        word: &str,
        position: Option<i64>,
        revision: i64,
        access_token: &str,
    ) -> Result<AccountDictionaryRevision, AccountError> {
        self.set_fixed_position(context, code, word, position, revision, access_token)
    }

    fn add_dictionary(
        &self,
        kind: DictionaryKind,
        code: &str,
        word: &str,
        weight: i64,
        access_token: &str,
    ) -> Result<AccountDictionaryChange, AccountError> {
        self.add_dictionary(kind, code, word, weight, access_token)
    }

    fn update_dictionary(
        &self,
        kind: DictionaryKind,
        id: &str,
        code: &str,
        word: &str,
        weight: i64,
        revision: i64,
        access_token: &str,
    ) -> Result<AccountDictionaryChange, AccountError> {
        self.update_dictionary(kind, id, code, word, weight, revision, access_token)
    }

    fn delete_dictionary(
        &self,
        kind: DictionaryKind,
        id: &str,
        revision: i64,
        access_token: &str,
    ) -> Result<AccountDictionaryChange, AccountError> {
        self.delete_dictionary(kind, id, revision, access_token)
    }

    fn import_dictionary(
        &self,
        kind: DictionaryKind,
        format: &str,
        text: &str,
        access_token: &str,
    ) -> Result<AccountDictionaryImportResult, AccountError> {
        self.import_dictionary(kind, format, text, access_token)
    }

    fn export_dictionary(
        &self,
        kind: DictionaryKind,
        format: &str,
        access_token: &str,
    ) -> Result<AccountDictionaryExport, AccountError> {
        self.export_dictionary(kind, format, access_token)
    }

    fn export_dictionary_to_file(
        &self,
        kind: DictionaryKind,
        format: &str,
        destination: &Path,
        access_token: &str,
    ) -> Result<u64, AccountError> {
        self.export_dictionary_to_file(kind, format, destination, access_token)
    }

    fn dictionary_changes(
        &self,
        after: i64,
        limit: usize,
        access_token: &str,
    ) -> Result<AccountDictionaryChangePage, AccountError> {
        self.dictionary_changes(after, limit, access_token)
    }

    fn dictionary_snapshot(&self, access_token: &str) -> Result<Vec<u8>, AccountError> {
        self.dictionary_snapshot(access_token)
    }

    fn dictionary_snapshot_to_file(
        &self,
        destination: &Path,
        access_token: &str,
    ) -> Result<u64, AccountError> {
        self.dictionary_snapshot_to_file(destination, access_token)
    }

    fn restore_dictionary_snapshot(
        &self,
        snapshot: &[u8],
        revision: i64,
        access_token: &str,
    ) -> Result<AccountDictionarySnapshotRestore, AccountError> {
        self.restore_dictionary_snapshot(snapshot, revision, access_token)
    }

    fn translate(
        &self,
        request: &AccountTranslateRequest,
        timeout: Duration,
        access_token: &str,
    ) -> Result<AccountTranslations, AccountFailure> {
        self.translate_texts(request, timeout, access_token)
    }

    fn phrases(&self, access_token: &str) -> Result<AccountPhrases, AccountFailure> {
        self.load_phrases(access_token)
    }

    fn put_phrases(
        &self,
        phrases: &AccountPhrases,
        access_token: &str,
    ) -> Result<AccountPhrases, AccountFailure> {
        self.store_phrases(phrases, access_token)
    }

    fn devices(&self, access_token: &str) -> Result<AccountDevices, AccountFailure> {
        self.list_devices(access_token)
    }

    fn revoke_device(&self, id: &str, access_token: &str) -> Result<(), AccountFailure> {
        self.revoke_device(id, access_token)
    }

    fn data_summary(&self, access_token: &str) -> Result<AccountDataSummary, AccountFailure> {
        self.data_summary(access_token)
    }

    fn data_export(
        &self,
        destination: &Path,
        access_token: &str,
    ) -> Result<AccountDataExport, AccountFailure> {
        self.data_export_to_file(destination, access_token)
    }

    fn data_delete(&self, sections: &[String], access_token: &str) -> Result<(), AccountFailure> {
        self.data_delete(sections, access_token)
    }

    fn feedback(
        &self,
        feedback: &AccountFeedback,
        access_token: &str,
    ) -> Result<AccountFeedbackReceipt, AccountFailure> {
        self.send_feedback(feedback, access_token)
    }

    fn diagnostics_upload(
        &self,
        upload: &crate::diagnostics::upload::DiagnosticUpload,
        access_token: &str,
    ) -> Result<crate::diagnostics::upload::DiagnosticUploadReceipt, AccountFailure> {
        self.diagnostics_upload(upload, access_token)
    }

    fn diagnostics_state(
        &self,
        access_token: &str,
    ) -> Result<crate::diagnostics::upload::DiagnosticState, AccountFailure> {
        self.diagnostics_state(access_token)
    }

    fn diagnostics_regenerate_token(&self, access_token: &str) -> Result<String, AccountFailure> {
        self.diagnostics_regenerate_token(access_token)
    }

    fn diagnostics_delete(&self, access_token: &str) -> Result<(), AccountFailure> {
        self.diagnostics_delete(access_token)
    }

    fn download_link(
        &self,
        platform: &str,
        access_token: &str,
    ) -> Result<AccountDownloadLink, AccountFailure> {
        self.send_download_link(platform, access_token)
    }

    fn voice_contribution(
        &self,
        contribution: &AccountVoiceContribution,
        access_token: &str,
    ) -> Result<AccountVoiceContributionReceipt, AccountFailure> {
        self.send_voice_contribution(contribution, access_token)
    }

    fn put_typing_statistics(
        &self,
        upload: &crate::typing_statistics::upload::TypingStatisticsUpload,
        access_token: &str,
    ) -> Result<(), AccountFailure> {
        self.put_typing_statistics(upload, access_token)
    }

    fn delete_typing_statistics(&self, access_token: &str) -> Result<(), AccountFailure> {
        self.delete_typing_statistics(access_token)
    }

    fn qq_links(&self, access_token: &str) -> Result<AccountQqLinks, AccountFailure> {
        self.list_qq_links(access_token)
    }

    fn redeem_qq_link(
        &self,
        code: &str,
        access_token: &str,
    ) -> Result<AccountQqLink, AccountFailure> {
        self.redeem_qq_link(code, access_token)
    }

    fn delete_qq_link(&self, id: &str, access_token: &str) -> Result<(), AccountFailure> {
        self.delete_qq_link(id, access_token)
    }

    fn preference_schema_detailed(
        &self,
        access_token: &str,
    ) -> Result<AccountPreferenceSchema, AccountFailure> {
        if !crate::text::is_lower_hex(access_token, 64) {
            return Err(AccountError::Invalid.into());
        }
        let schema = self.json_detailed::<AccountPreferenceSchema, ()>(
            Method::GET,
            "/v1/users/me/preferences/schema",
            Some(access_token),
            None,
            MAX_JSON_BYTES,
            MAX_JSON_BYTES,
            Duration::from_secs(30),
        )?;
        validate_preference_schema(&schema)?;
        Ok(schema)
    }

    fn preferences_detailed(
        &self,
        access_token: &str,
    ) -> Result<AccountPreferences, AccountFailure> {
        if !crate::text::is_lower_hex(access_token, 64) {
            return Err(AccountError::Invalid.into());
        }
        let preferences = self.json_detailed::<AccountPreferences, ()>(
            Method::GET,
            "/v1/users/me/preferences",
            Some(access_token),
            None,
            MAX_JSON_BYTES,
            MAX_JSON_BYTES,
            Duration::from_secs(30),
        )?;
        validate_account_preferences(&preferences)?;
        Ok(preferences)
    }

    fn put_preferences_detailed(
        &self,
        preferences: &AccountPreferences,
        access_token: &str,
    ) -> Result<AccountPreferences, AccountFailure> {
        if !crate::text::is_lower_hex(access_token, 64) {
            return Err(AccountError::Invalid.into());
        }
        validate_account_preferences(preferences)?;
        let updated = self.json_detailed::<AccountPreferences, _>(
            Method::PUT,
            "/v1/users/me/preferences",
            Some(access_token),
            Some(preferences),
            MAX_JSON_BYTES,
            MAX_JSON_BYTES,
            Duration::from_secs(30),
        )?;
        validate_account_preferences(&updated)?;
        Ok(updated)
    }

    fn dictionary_probe(
        &self,
        after: i64,
        access_token: &str,
    ) -> Result<AccountDictionaryProbe, AccountFailure> {
        self.dictionary_probe(after, access_token)
    }

    fn dictionary_snapshot_download(
        &self,
        destination: &Path,
        access_token: &str,
    ) -> Result<u64, AccountFailure> {
        self.snapshot_to_file_with(destination, access_token, failure_from_response)
    }

    fn dictionary_snapshot_upload(
        &self,
        snapshot: &Path,
        revision: i64,
        access_token: &str,
    ) -> Result<AccountDictionarySnapshotRestore, AccountFailure> {
        self.restore_dictionary_snapshot_file_detailed(snapshot, revision, access_token)
    }
}

#[cfg(test)]
mod proxy_cache_tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    #[test]
    fn long_lived_account_clients_switch_proxy_on_the_next_request() {
        const CHILD: &str = "MSIME_ACCOUNT_PROXY_CACHE_TEST_CHILD";
        if std::env::var_os(CHILD).is_none() {
            // 偏好读取也会安装进程级代理，隔离后才不会与其他账号用例互相覆盖。
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "account::client::proxy_cache_tests::long_lived_account_clients_switch_proxy_on_the_next_request",
                ])
                .env(CHILD, "1")
                .status()
                .unwrap();
            assert!(status.success());
            return;
        }
        use msime_client_core::network::{self, NetworkPreferences, ProxyMode};
        struct RestoreNetwork(NetworkPreferences);
        impl Drop for RestoreNetwork {
            fn drop(&mut self) {
                network::install(&self.0);
            }
        }
        let _restore = RestoreNetwork(network::current());
        let proxy = || {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            listener.set_nonblocking(true).unwrap();
            let address = listener.local_addr().unwrap().to_string();
            let worker = std::thread::spawn(move || {
                let deadline = std::time::Instant::now() + Duration::from_secs(5);
                let mut requests = Vec::new();
                while requests.len() < 2 && std::time::Instant::now() < deadline {
                    match listener.accept() {
                        Ok((mut stream, _)) => {
                            stream.set_nonblocking(false).unwrap();
                            stream
                                .set_read_timeout(Some(Duration::from_secs(1)))
                                .unwrap();
                            let mut bytes = [0; 4096];
                            let mut length = 0;
                            while !bytes[..length].ends_with(b"\r\n\r\n") {
                                let read = stream.read(&mut bytes[length..]).unwrap();
                                assert!(read > 0 && length + read < bytes.len());
                                length += read;
                            }
                            requests.push(String::from_utf8_lossy(&bytes[..length]).to_string());
                            stream
                                .write_all(b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\n\r\n")
                                .unwrap();
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            std::thread::sleep(Duration::from_millis(5));
                        }
                        Err(error) => panic!("合成代理监听失败: {error}"),
                    }
                }
                requests
            });
            (address, worker)
        };
        let (first, first_worker) = proxy();
        let (second, second_worker) = proxy();
        network::install(&NetworkPreferences {
            proxy_mode: ProxyMode::Custom,
            proxy_server: first,
        });
        let clients = [
            BackendAccountClient::new().unwrap(),
            BackendAccountClient::with_user_agent("msime-linux/0.0.0").unwrap(),
        ];
        for address in [None, Some(second)] {
            if let Some(address) = address {
                network::install(&NetworkPreferences {
                    proxy_mode: ProxyMode::Custom,
                    proxy_server: address,
                });
            }
            for client in &clients {
                // CONNECT 在 TLS 与认证之前被合成代理拒绝；不联系真实服务。
                assert!(client
                    .request_with_limit_timeout(
                        Method::GET,
                        "/v1/users/me",
                        None,
                        None,
                        1024,
                        Duration::from_secs(1)
                    )
                    .is_err());
            }
        }
        for worker in [first_worker, second_worker] {
            let requests = worker.join().unwrap();
            assert_eq!(
                requests.len(),
                2,
                "每个长期客户端的下一次请求都应经过新代理"
            );
            assert!(requests
                .iter()
                .all(|request| request.starts_with("CONNECT ")
                    && !request.to_ascii_lowercase().contains("authorization:")));
        }
    }
}
