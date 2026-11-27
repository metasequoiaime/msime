# Agent Note: 更新检查压缩响应、分开连接与读取超时、桌面端信任系统证书库

Status: implemented

## Problem

#11927：Windows 11 上设置窗口「关于 → 版本更新」的「检查更新」永远显示「检查失败，请稍后重试」，换任何更新通道都一样，失败都在 0.15–0.4 秒内出现。报告者自己做了实验，钉出两条互不相关的原因：

1. **只信任 Mozilla 根。** 本机 hosts 把 `api.github.com` 指到 `127.0.0.1:443` 的本地代理（SteamCommunity_302），它出示的是装进 Windows 证书库的自签 CA 签发的证书。`crates/client-services` 的 `reqwest` 用的是 `rustls-tls`（= `rustls-tls-webpki-roots`），只读 webpki 里的 Mozilla 根，不读系统库，所以 TLS 握手在 0.1 秒级就失败。同一组对照实验：Mozilla 根 + 经 hosts = `CERT_TRUST_IS_PARTIAL_CHAIN`；Mozilla 根 + 直连真实 IP = 200；Windows 证书库 + 经 hosts = 200。
2. **2.8 MB 未压缩 + 10 秒总超时。** `RELEASES_URL` 是 `releases?per_page=100`，`client-services` 的 `reqwest` 没开 `gzip`，请求不带 `Accept-Encoding`，GitHub 回未压缩 JSON（2 924 896 B）。当地实测下载要 47–63 秒，而客户端只有一个 `TIMEOUT = 10s` 的总超时，慢网上必然 `Unavailable`。带 `Accept-Encoding: gzip` 时同一 URL 只有 463 KB、4.2 秒。

#11850（Linux 云候选不返回）那种「看着像功能坏了、其实被超时吞掉」是同一类形状：单个总超时 + 一次要传几 MB。

## Decision

三处改动，都只影响网络层，不改任何界面：

1. `crates/client-services/Cargo.toml`：给 `reqwest` 开 `gzip` 特性，请求自动带 `Accept-Encoding: gzip`，响应体在读的时候自动解压。发布列表从 2.8 MB 降到约 460 KB，`MAX_RESPONSE_BYTES` 的上界按解压后的字节数生效，压缩炸弹不会比未压缩响应多占内存。
2. 超时拆成两个：`CONNECT_TIMEOUT = 10s`（DNS + TCP + TLS）与 `REQUEST_TIMEOUT = 30s`（含读完响应体）。原来的单值 10 秒把「握手慢」和「读不完」混在一起，慢网上一律变成「功能不可用」。
3. 桌面端（`cfg(any(target_os = "macos", target_os = "windows"))`）另加 `rustls-tls-native-roots`：根证书变成「Mozilla 根 ∪ 系统证书库」。reqwest 0.12 在两个根特性都开时同时加载两份来源（`src/async_impl/client.rs` 的 `tls_built_in_certs_webpki` / `tls_built_in_certs_native`），所以系统库读不到时也不会失去 Mozilla 根；只有「读到的证书全部解析失败」时 build 才报错。Android、iOS、HarmonyOS 的目标表里没有这一项，仍只认 Mozilla 根。

锁文件里新增的依赖（维护者已确认）：`rustls-native-certs` 0.8.5、`openssl-probe` 0.2.2、`schannel` 0.1.30、`security-framework` 3.7.0、`security-framework-sys` 2.17.0；`gzip` 用到的压缩 crate 已经在锁里，没有新增。锁一共只多了 5 个包。

## Alternatives considered

- **只把 `per_page` 降到 30，不加任何依赖。** 最强理由是不动 `reqwest` 的特性、零新增 crate。但列表按「所有平台的发布」倒序排列，30 条只覆盖各平台最近五六个版本；某个平台一段时间没发版，它的最新发布就会掉出第一页，更新检查会静默地「没有更新」。而且传输量只砍掉约三分之二，慢网仍然悬在超时边缘。
- **全平台都信任系统证书库。** Windows/macOS 之外没有「系统里另装 CA」的等价做法，而 `rustls-native-certs` 在移动端多半读到空集合，等于多编一份用不上的依赖；只在已确认的平台开启，范围最小。
- **加一个「自定义 CA / 信任系统证书」设置项。** 更灵活，但要把新设置从共享 Rust 一路做到六个平台的界面，而且绝大多数用户不需要；先用系统库这个平台已有的信任配置，等真有需要再谈设置项。
- **把 10 秒总超时直接改大（不动压缩）。** 2.8 MB 在报告者的链路上要 47–63 秒，改到 60 秒才够，等于把一个正常机器上 1 秒内完成的检查拖成最坏一分钟；压缩是更根本的做法。
- **顺手把 `UpdateCheckError` 拆成「不可达 / 证书不受信任 / 超时」并在界面上分开提示。** 报告者的第 4 条诉求，但它要跨 `host-api` 的 FFI 与六个平台的界面，属于另一件事，不并进这一条。

## Consequences

- 桌面端现在信任用户/企业装进系统证书库的 CA。这是浏览器同款行为，但意味着本机装了恶意 CA 的机器可以被中间人改掉更新列表；更新包本身的签名与 SHA-256 校验路径不变。
- 慢网下更新检查从「几乎必然失败」变成「几秒到十几秒内成功」；最坏情况（GitHub 卡住）从 10 秒变成 30 秒，后台定期检查每天一次，这个量级不影响使用。
- 移动端与 ohos 的行为、信任范围完全不变。
- 报告者的第 4 条（区分失败原因、给「打开发布页」兜底）和第 5 条（用户文档）仍未做，见 PR 正文。

## Testing

- `cargo test -p msime-client-services --lib update_check`：新增两条，`a_gzip_encoded_release_list_is_asked_for_and_decompressed`（本地起一个只回 gzip 的 TCP 服务，断言请求带 `Accept-Encoding: gzip`、响应被解压成原文）与 `the_update_client_builds_with_the_system_root_store`（在 CI 的 macOS/Windows 上顺带验证系统证书库加载）。
- 反向验证：临时去掉 `gzip` 特性重跑那条测试，它以「读到的是 gzip 原始字节」失败（`[31, 139, 8, ...]` 对不上 JSON），说明测试确实在测这件事，不是恒真。
- 本机没有 Windows 宿主：Windows 侧的编译与运行由 PR 上的 `Windows native settings`、`Engine C ABI (Windows)` 负责；报告者那条「hosts + 自签 CA」的路径要他自己在装了这个版本的机器上复验。
