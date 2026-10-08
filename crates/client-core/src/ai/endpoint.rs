//! AI 辅助接口地址的传输策略：什么样的地址可以收到用户的 API Token 和输入内容。
//!
//! 规则只有一条：`https://` 地址不限主机；`http://` 只能指向本机或局域网，也就是回环地址（`localhost`、`127.0.0.0/8`、`::1`）、RFC 1918 私有 IPv4（`10/8`、`172.16/12`、`192.168/16`）、CGNAT 与 Tailscale 用的 `100.64.0.0/10`、链路本地（`169.254/16`、`fe80::/10`）、IPv6 唯一本地地址 `fc00::/7`，以及 mDNS 的 `.local` 主机名。公网主机必须走 https，API Token 不能明文经过互联网。
//!
//! 检查时不做 DNS 解析：除 `localhost` 和 `.local` 以外的域名一律按公网处理，哪怕它在用户的网络里解析到私有地址。解析结果随网络环境变化，按解析结果放行就等于让一个公网域名在某些网络下收到明文 Token。
//!
//! 这里是这条规则的权威实现。各平台宿主在自己的语言里另有一份同样的判断（设置页要在本地决定能不能填 Token，发请求前还要再拦一次），它们都要跑 `shared/contracts/ai-endpoint/cases.json` 里的同一组用例；改规则时先改这里和那份用例，再改各宿主。

use reqwest::Url;
use std::net::{Ipv4Addr, Ipv6Addr};

/// 接口地址能接受的最大字节数，与各宿主和设置页一致。
pub const MAX_ENDPOINT_BYTES: usize = 2048;

/// 地址被拒绝的原因。设置页按它给出不同的说明。
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum AiEndpointError {
    /// 不是带主机的完整 `http(s)://` 地址，或者带了用户名、密码、`#` 片段、控制字符、反斜杠，或者 `http://` 的主机不是规范写法。
    Invalid,
    /// `http://` 地址指向的不是本机或局域网主机。
    CleartextPublicHost,
}

/// 检查 AI 接口地址，通过时返回解析后的地址。调用方负责去掉首尾空白。
pub fn validate(endpoint: &str) -> Result<Url, AiEndpointError> {
    // `\` 在 WHATWG 解析里等同 `/`，curl 等宿主却把它当成主机或用户名的一部分：`http://127.0.0.1\@example.com` 在这里是回环地址，curl 会连到 example.com。同一个地址各处读出不同的主机，一律不收。
    if endpoint.is_empty()
        || !crate::text::is_bounded_text(endpoint, MAX_ENDPOINT_BYTES)
        || endpoint.contains('\\')
    {
        return Err(AiEndpointError::Invalid);
    }
    let url = Url::parse(endpoint).map_err(|_| AiEndpointError::Invalid)?;
    // `https:///v1` 会被解析成主机 `v1`；要求 `://` 后面紧跟主机，免得路径被当成主机。
    let explicit_authority = endpoint
        .split_once("://")
        .is_some_and(|(_, rest)| rest.as_bytes().first().is_some_and(|byte| *byte != b'/'));
    if !matches!(url.scheme(), "https" | "http")
        || !explicit_authority
        || url.host_str().is_none_or(str::is_empty)
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err(AiEndpointError::Invalid);
    }
    if url.scheme() == "http" {
        // 明文地址的主机必须按解析后的样子书写（大小写除外）。`10.1`、`0x7f000001`、`127.0.0.1.`、百分号编码这类写法会被 WHATWG 规范化，手写解析的宿主却不认，结果设置页存下了 Token，宿主那边按公网拒绝，Linux 的 provider 读到这样一条配置还会让所有 AI 配置都失效。
        let host = url.host_str().unwrap_or_default();
        if !written_host(endpoint).is_some_and(|written| written.eq_ignore_ascii_case(host)) {
            return Err(AiEndpointError::Invalid);
        }
        if !is_local_network_host(host) {
            return Err(AiEndpointError::CleartextPublicHost);
        }
    }
    Ok(url)
}

/// `://` 之后、端口之前原样书写的主机；IPv6 带方括号。首尾空格与 `Url::parse` 一样先去掉。
fn written_host(endpoint: &str) -> Option<&str> {
    let (_, rest) = endpoint.trim_matches(' ').split_once("://")?;
    let authority = rest.split(['/', '?']).next()?;
    if authority.starts_with('[') {
        authority.find(']').map(|end| &authority[..=end])
    } else {
        authority.split(':').next()
    }
}

/// 地址能否接收 Token 和输入内容。
pub fn is_allowed(endpoint: &str) -> bool {
    validate(endpoint).is_ok()
}

/// 保存 Token 用的来源键：`scheme://host:port`，主机小写、端口总是写出来（https 默认 443，http 默认 80）。地址不合规时没有来源键，设置页也就不让填 Token。
///
/// 与设置页的 `aiCredentialOrigin` 和 Android 的 `AiPolishConfiguration.credentialOrigin` 逐字一致，否则页面存下的 Token 在这里找不到。
pub fn credential_origin(endpoint: &str) -> Option<String> {
    let url = validate(endpoint).ok()?;
    let host = url.host_str()?.to_ascii_lowercase();
    let port = url.port_or_known_default()?;
    Some(format!("{}://{host}:{port}", url.scheme()))
}

/// 发往 AI 接口的 HTTP 客户端的公共设置，`url` 必须已经通过 [`validate`]。不跟随重定向，免得 Token 被转发到别的主机；https 地址只走 https；http 地址（只会是本机或局域网）直连、不经过系统或环境变量里配置的代理，否则代理会收到明文的 Token，而代理本身可能在公网上。
pub fn blocking_client_builder(url: &Url) -> reqwest::blocking::ClientBuilder {
    let builder = reqwest::blocking::Client::builder().redirect(reqwest::redirect::Policy::none());
    if url.scheme() == "https" {
        builder.https_only(true)
    } else {
        builder.no_proxy()
    }
}

/// 主机是否属于本机或局域网，见模块说明。`host` 是 `Url::host_str` 的结果：IPv4 已规范成点分十进制，IPv6 带方括号。IP 字面量按地址段判断，域名只认 `localhost` 和 `.local`。
pub fn is_local_network_host(host: &str) -> bool {
    if let Some(address) = host
        .strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
    {
        return address.parse().is_ok_and(is_local_ipv6);
    }
    if let Ok(address) = host.parse() {
        return is_local_ipv4(address);
    }
    let domain = host.to_ascii_lowercase();
    domain == "localhost"
        || domain
            .strip_suffix(".local")
            .is_some_and(|label| !label.is_empty() && !label.ends_with('.'))
}

fn is_local_ipv4(address: Ipv4Addr) -> bool {
    let [first, second, ..] = address.octets();
    address.is_loopback()
        || address.is_private()
        || address.is_link_local()
        // 100.64.0.0/10：运营商级 NAT 与 Tailscale 的地址段，`Ipv4Addr::is_shared` 还没有稳定。
        || (first == 100 && second & 0b1100_0000 == 0b0100_0000)
}

fn is_local_ipv6(address: Ipv6Addr) -> bool {
    address.is_loopback() || address.is_unicast_link_local() || address.is_unique_local()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct Contract {
        cases: Vec<Case>,
    }

    #[derive(Deserialize)]
    struct Case {
        endpoint: String,
        result: String,
        origin: Option<String>,
    }

    /// 各平台共用的用例；这里和每个宿主都要逐条通过。
    #[test]
    fn shared_contract_cases() {
        let contract: Contract = serde_json::from_str(include_str!(
            "../../../../shared/contracts/ai-endpoint/cases.json"
        ))
        .unwrap();
        assert!(contract.cases.len() > 40);
        for case in &contract.cases {
            let actual = match validate(&case.endpoint) {
                Ok(_) => "allowed",
                Err(AiEndpointError::Invalid) => "invalid",
                Err(AiEndpointError::CleartextPublicHost) => "cleartext_public",
            };
            assert_eq!(actual, case.result, "{}", case.endpoint);
            assert_eq!(
                credential_origin(&case.endpoint),
                case.origin,
                "{}",
                case.endpoint
            );
        }
    }

    /// 明文地址的主机只认规范写法，与手写解析的宿主一致；这些写法各平台的结论不同（无效或公网），所以不放进共享用例。https 不判断主机，照常放行。
    #[test]
    fn rejects_http_hosts_not_written_canonically() {
        for endpoint in [
            "http://0x7f000001:1234/v1",
            "http://10.1:1234/v1",
            "http://0x08080808/v1",
            "http://127.0.0.1./v1",
            "http://%6cocalhost/v1",
            "http://[0:0::1]/v1",
            "http://@localhost/v1",
        ] {
            assert_eq!(
                validate(endpoint).unwrap_err(),
                AiEndpointError::Invalid,
                "{endpoint}"
            );
        }
        assert!(is_allowed("https://0x7f000001:1234/v1"));
        assert!(is_allowed(" http://192.168.1.20:1234 "));
        // 带结尾点的 `.local` 不按局域网名处理。
        assert_eq!(
            validate("http://studio.local./v1").unwrap_err(),
            AiEndpointError::CleartextPublicHost
        );
    }
}
