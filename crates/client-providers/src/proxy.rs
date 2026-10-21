//! 把共享偏好里的网络代理设置套到 reqwest 客户端上。
//!
//! 判据（哪些目标能走代理）在 `msime_client_core::network`，这里只做三件事：`none` 关掉代理，
//! `custom` 挂一个按目标逐个判断的代理，`system` 保持 reqwest 默认的系统/环境变量代理。
//!
//! 这里和 `msime-client-services` 的 `proxy` 模块内容相同：两个 crate 是同级的，互不引用，
//! 而 `client-core` 的编译图里有意不放 reqwest（输入运行时和后端引擎都用它）。判据只写一份。

use msime_client_core::network::{
    current, normalized_proxy_url, should_use_proxy, NetworkPreferences, ProxyMode,
};
use reqwest::blocking::ClientBuilder;
use reqwest::Url;

/// 按当前偏好配置一个只会请求 `https` 目标的客户端。`custom` 下每个目标仍按 [`should_use_proxy`] 再判断一次，
/// 所以本机或局域网目标、以及任何明文 `http` 目标都不会交给代理。
pub fn configure_blocking(builder: ClientBuilder) -> ClientBuilder {
    configure_blocking_with_preferences(builder, &current())
}

fn configure_blocking_with_preferences(
    builder: ClientBuilder,
    preferences: &NetworkPreferences,
) -> ClientBuilder {
    match preferences.effective_mode() {
        ProxyMode::None => builder.no_proxy(),
        ProxyMode::System => builder,
        ProxyMode::Custom => match normalized_proxy_url(&preferences.proxy_server) {
            // `effective_mode` 已经把不可用的地址归到 `system`，这里只是兜底。
            None => builder,
            Some(server) => builder.proxy(reqwest::Proxy::custom(move |url: &Url| {
                should_use_proxy(url, ProxyMode::Custom, is_local(url))
                    .then(|| Url::parse(&server).expect("代理地址已规范化"))
            })),
        },
    }
}

/// 按当前偏好配置一个只发往 `url` 的客户端。目标不该走代理时（明文 `http`、本机或局域网）显式直连。
pub fn configure_blocking_for(builder: ClientBuilder, url: &Url) -> ClientBuilder {
    configure_blocking_for_with_preferences(builder, url, &current())
}

/// 使用同一份设置快照建立连接池，避免缓存的版本与实际代理设置不一致。
pub(crate) fn configure_blocking_for_with_preferences(
    builder: ClientBuilder,
    url: &Url,
    preferences: &NetworkPreferences,
) -> ClientBuilder {
    if should_use_proxy(url, preferences.effective_mode(), is_local(url)) {
        configure_blocking_with_preferences(builder, preferences)
    } else {
        builder.no_proxy()
    }
}

fn is_local(url: &Url) -> bool {
    url.host_str()
        .is_some_and(msime_client_core::network::is_local_host)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(value: &str) -> Url {
        Url::parse(value).unwrap()
    }

    /// 构造客户端只为验证配置不报错；不会发出请求。
    #[test]
    fn every_mode_builds_a_client() {
        for preferences in [
            NetworkPreferences::default(),
            NetworkPreferences {
                proxy_mode: ProxyMode::None,
                proxy_server: String::new(),
            },
            NetworkPreferences {
                proxy_mode: ProxyMode::Custom,
                proxy_server: "127.0.0.1:7890".into(),
            },
            NetworkPreferences {
                proxy_mode: ProxyMode::Custom,
                proxy_server: String::new(),
            },
        ] {
            for target in [
                "https://api.example.test/v1",
                "http://127.0.0.1:11434/v1",
                "https://studio.local/v1",
            ] {
                assert!(
                    configure_blocking_for_with_preferences(
                        ClientBuilder::new(),
                        &url(target),
                        &preferences
                    )
                    .build()
                    .is_ok(),
                    "{preferences:?} {target}"
                );
            }
            assert!(
                configure_blocking_with_preferences(ClientBuilder::new(), &preferences)
                    .build()
                    .is_ok()
            );
        }
    }
}
