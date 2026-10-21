//! 用户内容请求的网络代理策略：`system` 跟随系统代理、`none` 直连、`custom` 走用户填的 HTTP 代理。
//!
//! 判据只有一条，见 [`should_use_proxy`]：只有 `https://` 公网目标才可能走代理；`http://` 目标（只可能是本机或局域网）
//! 和本机、局域网主机一律直连。代理只能看到目标地址（CONNECT）和 TLS 密文，看不到 Token 与输入内容，这是最小暴露面。
//!
//! 这里只放判据和偏好，不建 HTTP 客户端：`client-core` 的编译图里没有 reqwest。`msime-client-providers` 和
//! `msime-client-services` 各有一份很薄的 `proxy` 模块，把 [`current`] 里的设置套到自己的 `ClientBuilder` 上。

use serde::{Deserialize, Serialize};
use std::sync::{OnceLock, RwLock};

/// 代理地址（`proxy_server`）的最大字节数，与设置页一致。
pub const MAX_PROXY_SERVER_BYTES: usize = 512;

/// `network.proxy_mode` 的三个取值，与 Windows 正式版 `installer/default_config/config.default.toml` 相同。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProxyMode {
    /// 跟随系统或环境变量里的代理，也是出厂值。
    #[default]
    System,
    /// 一律直连，不使用任何代理。
    None,
    /// 使用 [`NetworkPreferences::proxy_server`]；它不可用时按 `system` 处理。
    Custom,
}

/// 偏好文档里的 `network` 一节。默认值与 Windows 正式版一致：跟随系统代理，不填服务器。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct NetworkPreferences {
    pub proxy_mode: ProxyMode,
    /// `custom` 模式的 HTTP 代理地址，形如 `127.0.0.1:7890`，也可以带 `http://` 或 `https://` 前缀。留空时按 `system` 处理。
    pub proxy_server: String,
}

impl NetworkPreferences {
    pub fn is_default(&self) -> bool {
        self.proxy_mode.is_default() && self.proxy_server.is_empty()
    }

    /// 实际生效的模式：`custom` 但代理地址为空（或不合规）时回退 `system`，与 Windows 正式版的「留空按 system 处理」一致。
    pub fn effective_mode(&self) -> ProxyMode {
        match self.proxy_mode {
            ProxyMode::Custom if normalized_proxy_url(&self.proxy_server).is_none() => {
                ProxyMode::System
            }
            mode => mode,
        }
    }
}

impl ProxyMode {
    fn is_default(&self) -> bool {
        matches!(self, Self::System)
    }
}

/// 一个请求该不该走代理。`is_local_host` 由调用方按目标主机的 IP 段或名字算出来（[`is_local_host`]：回环、RFC 1918、链路本地、CGNAT、IPv6 唯一本地地址、`localhost` 与 `.local`）。
///
/// - 非 `https` 目标永远不走代理：明文 `http` 只允许发往本机或局域网，交给代理就等于把明文交出去。
/// - 本机、局域网目标永远直连，哪怕是 `https`。
/// - 其余 `https` 目标在 `system` 和 `custom` 下走代理，`none` 下直连。
///
/// `system` 只是「允许用系统代理」：客户端在 `system` 下不显式设置代理，由 reqwest 按系统或环境变量决定，
/// 系统自己的排除列表照旧生效。
pub fn should_use_proxy(url: &url::Url, mode: ProxyMode, is_local_host: bool) -> bool {
    if url.scheme() != "https" || is_local_host {
        return false;
    }
    !matches!(mode, ProxyMode::None)
}

/// 代理地址是否可用：可选的 `http://` / `https://` 前缀，主机非空，端口必须写出来，不能带用户名、密码、路径、查询或片段。
/// 与 Windows 正式版一样接受 `127.0.0.1:7890` 这种不带前缀的写法。
pub fn valid_proxy_server(value: &str) -> bool {
    normalized_proxy_url(value).is_some()
}

/// 把用户填的地址规范成 reqwest 认的 `scheme://host:port`；不可用时是 `None`（空字符串也是 `None`）。
pub fn normalized_proxy_url(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty()
        || value.len() > MAX_PROXY_SERVER_BYTES
        || !crate::text::is_bounded_text(value, MAX_PROXY_SERVER_BYTES)
        || value.contains('\\')
        || value.contains('#')
    {
        return None;
    }
    let candidate = if value.contains("://") {
        value.to_owned()
    } else {
        format!("http://{value}")
    };
    // URL 库会省略显式的标准端口，是否写端口必须从原始 authority 判断。
    let authority = candidate
        .split_once("://")?
        .1
        .split(['/', '?', '#'])
        .next()?;
    let port = authority.rsplit_once(':')?.1;
    if port.is_empty() || !port.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let url = url::Url::parse(&candidate).ok()?;
    if !matches!(url.scheme(), "http" | "https")
        || !crate::endpoint::valid_endpoint_shape(&candidate, &url, url.scheme())
        || url.port_or_known_default().is_none_or(|port| port == 0)
        || !matches!(url.path(), "" | "/")
        || url.query().is_some()
        || url.cannot_be_a_base()
    {
        return None;
    }
    let host = url.host_str()?;
    Some(format!(
        "{}://{host}:{}",
        url.scheme(),
        url.port_or_known_default()?
    ))
}

/// 主机是否属于本机或局域网：回环、RFC 1918 私有地址、链路本地、CGNAT（含 Tailscale）和 IPv6 唯一本地地址，
/// 域名只认 `localhost` 和 `.local`。`host` 是 `Url::host_str` 的结果（IPv6 带方括号）。
///
/// 这是 `http://` 接口地址规则和代理判据共用的那一份，AI 接口的 `is_local_network_host` 也转到这里，
/// 两个判断不能各写一套：一个地址被判成公网却仍算本机，代理就会收到明文的 Token。
pub fn is_local_host(host: &str) -> bool {
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

fn is_local_ipv4(address: std::net::Ipv4Addr) -> bool {
    let [first, second, ..] = address.octets();
    address.is_loopback()
        || address.is_private()
        || address.is_link_local()
        // 100.64.0.0/10：运营商级 NAT 与 Tailscale 的地址段，`Ipv4Addr::is_shared` 还没有稳定。
        || (first == 100 && second & 0b1100_0000 == 0b0100_0000)
}

fn is_local_ipv6(address: std::net::Ipv6Addr) -> bool {
    address.is_loopback() || address.is_unicast_link_local() || address.is_unique_local()
}

static CURRENT: OnceLock<RwLock<NetworkPreferences>> = OnceLock::new();

fn current_slot() -> &'static RwLock<NetworkPreferences> {
    CURRENT.get_or_init(|| RwLock::new(NetworkPreferences::default()))
}

/// 本进程当前生效的代理设置。宿主每次读到偏好文档时经 [`install`] 写进来，默认是 `system`。
pub fn current() -> NetworkPreferences {
    current_slot()
        .read()
        .map(|current| current.clone())
        .unwrap_or_default()
}

/// 记下宿主刚读到或刚保存的代理设置。值没变时不写锁，读偏好的路径因此没有额外开销。
pub fn install(preferences: &NetworkPreferences) {
    let Ok(mut current) = current_slot().write() else {
        return;
    };
    if *current != *preferences {
        *current = preferences.clone();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(value: &str) -> url::Url {
        url::Url::parse(value).unwrap()
    }

    // 三种模式 × 三类目标，每格一个用例，失败时直接看出是哪一格。
    #[test]
    fn system_proxies_public_https() {
        assert!(should_use_proxy(
            &url("https://api.example.test/v1"),
            ProxyMode::System,
            false
        ));
    }

    #[test]
    fn system_direct_connects_cleartext_loopback() {
        assert!(!should_use_proxy(
            &url("http://127.0.0.1:11434/v1"),
            ProxyMode::System,
            true
        ));
    }

    #[test]
    fn system_direct_connects_local_https() {
        assert!(!should_use_proxy(
            &url("https://127.0.0.1:8443/v1"),
            ProxyMode::System,
            true
        ));
    }

    #[test]
    fn custom_proxies_public_https() {
        assert!(should_use_proxy(
            &url("https://api.example.test/v1"),
            ProxyMode::Custom,
            false
        ));
    }

    #[test]
    fn custom_direct_connects_cleartext_loopback() {
        assert!(!should_use_proxy(
            &url("http://127.0.0.1:11434/v1"),
            ProxyMode::Custom,
            true
        ));
    }

    #[test]
    fn custom_direct_connects_local_https() {
        assert!(!should_use_proxy(
            &url("https://192.168.1.20:8443/v1"),
            ProxyMode::Custom,
            true
        ));
    }

    #[test]
    fn none_direct_connects_public_https() {
        assert!(!should_use_proxy(
            &url("https://api.example.test/v1"),
            ProxyMode::None,
            false
        ));
    }

    #[test]
    fn none_direct_connects_cleartext_loopback() {
        assert!(!should_use_proxy(
            &url("http://127.0.0.1:11434/v1"),
            ProxyMode::None,
            true
        ));
    }

    #[test]
    fn none_direct_connects_local_https() {
        assert!(!should_use_proxy(
            &url("https://studio.local/v1"),
            ProxyMode::None,
            true
        ));
    }

    /// 公网 `http` 不在接口地址规则允许的范围里，但判据本身不能把它交给代理。
    #[test]
    fn cleartext_targets_are_never_proxied() {
        assert!(!should_use_proxy(
            &url("http://api.example.test/v1"),
            ProxyMode::Custom,
            false
        ));
    }

    /// `custom` 但代理地址为空时回退 `system`。
    #[test]
    fn custom_without_a_server_falls_back_to_the_system_proxy() {
        let empty = NetworkPreferences {
            proxy_mode: ProxyMode::Custom,
            proxy_server: String::new(),
        };
        assert_eq!(empty.effective_mode(), ProxyMode::System);
        let blank = NetworkPreferences {
            proxy_mode: ProxyMode::Custom,
            proxy_server: "   ".into(),
        };
        assert_eq!(blank.effective_mode(), ProxyMode::System);
        let broken = NetworkPreferences {
            proxy_mode: ProxyMode::Custom,
            proxy_server: "not a proxy".into(),
        };
        assert_eq!(broken.effective_mode(), ProxyMode::System);
        let usable = NetworkPreferences {
            proxy_mode: ProxyMode::Custom,
            proxy_server: "127.0.0.1:7890".into(),
        };
        assert_eq!(usable.effective_mode(), ProxyMode::Custom);
        assert!(should_use_proxy(
            &url("https://api.example.test/v1"),
            usable.effective_mode(),
            false
        ));
    }

    #[test]
    fn local_hosts_are_shared_with_the_http_endpoint_rule() {
        for local in [
            "localhost",
            "127.0.0.1",
            "10.1.2.3",
            "172.16.0.9",
            "192.168.1.20",
            "100.64.0.7",
            "169.254.1.1",
            "[::1]",
            "[fe80::1]",
            "[fd00::1]",
            "studio.local",
        ] {
            assert!(is_local_host(local), "{local}");
        }
        for public in [
            "api.example.test",
            "8.8.8.8",
            "studio.local.",
            "[2001:db8::1]",
        ] {
            assert!(!is_local_host(public), "{public}");
        }
    }

    #[test]
    fn proxy_server_shapes() {
        for (value, expected) in [
            ("127.0.0.1:7890", Some("http://127.0.0.1:7890")),
            ("http://127.0.0.1:7890", Some("http://127.0.0.1:7890")),
            (
                "https://proxy.example.test:8443",
                Some("https://proxy.example.test:8443"),
            ),
            (
                "proxy.example.test:8080",
                Some("http://proxy.example.test:8080"),
            ),
            (" [::1]:7890 ", Some("http://[::1]:7890")),
            (
                "http://proxy.example.test:80",
                Some("http://proxy.example.test:80"),
            ),
            (
                "https://proxy.example.test:443",
                Some("https://proxy.example.test:443"),
            ),
            (
                "proxy.example.test:00080",
                Some("http://proxy.example.test:80"),
            ),
            ("http://[::1]:80", Some("http://[::1]:80")),
            ("http://proxy.example.test", None),
            ("https://proxy.example.test", None),
            ("http://[::1]", None),
            ("http://proxy.example.test:", None),
            ("", None),
            ("127.0.0.1", None),
            ("127.0.0.1:0", None),
            ("socks5://127.0.0.1:1080", None),
            ("http://user:pass@127.0.0.1:7890", None),
            ("http://127.0.0.1:7890/path", None),
            ("http://127.0.0.1:7890?x=1", None),
            ("127.0.0.1:7890#fragment", None),
            ("http://127.0.0.1:99999", None),
        ] {
            assert_eq!(
                normalized_proxy_url(value).as_deref(),
                expected,
                "{value:?}"
            );
            assert_eq!(valid_proxy_server(value), expected.is_some(), "{value:?}");
        }
    }

    /// `install` 之后 `current` 读到的是刚写入的值；进程内只保存一份。
    #[test]
    fn installed_preferences_are_what_the_egress_reads() {
        let previous = current();
        let installed = NetworkPreferences {
            proxy_mode: ProxyMode::Custom,
            proxy_server: "127.0.0.1:7890".into(),
        };
        install(&installed);
        assert_eq!(current(), installed);
        install(&previous);
        assert_eq!(current(), previous);
    }
}
