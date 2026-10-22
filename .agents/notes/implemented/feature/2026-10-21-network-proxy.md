# Agent Note: 网络代理设置（system / none / custom）

Status: implemented

## Problem

Windows 正式版的 `installer/default_config/config.default.toml:162-167` 有 `[network] proxy_mode` 和 `proxy_server` 两个键：`system` 跟随系统代理、`none` 直连、`custom` 用用户填的 HTTP 代理，`proxy_server` 留空按 `system` 处理。新架构完全没有这两个字段：`crates/client-core/src/preferences.rs` 里没有代理偏好，`crates/client-providers` 和 `crates/client-services` 的 28 处 `Client::builder` / `Client::new` 只用 reqwest 默认的「系统或环境变量代理」。

后果在公司网、校园网和网络受限地区是致命的：云候选、候选翻译、语音识别、AI 联想、账号、更新检查和词库与模型下载全部连不上，设置页也没有地方填代理。旧版的三种模式全部失效。

## Decision

偏好文档里新增 `network` 一节，出厂值与 Windows 正式版逐字一致：

| 键 | 出厂 | 语义 |
| --- | --- | --- |
| `network.proxy_mode` | `"system"` | `system` 跟随系统或环境变量代理 / `none` 直连 / `custom` 用下面的 HTTP 代理 |
| `network.proxy_server` | `""` | `custom` 模式的 HTTP 代理地址，形如 `127.0.0.1:7890`，可带 `http://` 或 `https://` 前缀；留空按 `system` 处理 |

### 判据只有一条，写成纯函数

`crates/client-core/src/network.rs` 的 `should_use_proxy(url, mode, is_local_host) -> bool` 是权威实现：

- 非 `https` 目标 **never** 走代理。明文 `http` 只允许发往本机或局域网，交给代理就是把 Token 和输入内容明文交出去。
- 本机、局域网目标 **never** 走代理，哪怕是 https。`localhost`、回环、RFC 1918、链路本地、CGNAT（含 Tailscale 的 `100.64.0.0/10`）、IPv6 唯一本地地址和 `.local`，实现在 `network::is_local_host`，`ai::endpoint::is_local_network_host` 转调这一份，两个判断不再各写一套。
- 其余 https 目标在 `system` 和 `custom` 下走代理，`none` 下直连。

代理端口必须显式填写；URL 库会折叠 `http:80` 和 `https:443`，因此从原始 authority 判断端口是否存在，再用规范化端口生成地址。标准端口和前导零合法，缺端口与端口 0 非法；Rust 与设置页保持一致。

`custom` 但 `proxy_server` 为空或不合规时，`NetworkPreferences::effective_mode` 返回 `System`，就是「留空按 system 处理」。

三种模式都只对用户内容请求生效；`system` 下不显式挂代理，由 reqwest 按系统或环境变量决定，系统自己的排除列表照旧生效。为了让 `system` 真能读到 macOS 系统设置和 Windows 注册表，`msime-client-providers` 与 `msime-client-services` 的 reqwest 打开了 `system-proxy` 特性（`Cargo.lock` 因此新增 `system-configuration` 和 `windows-registry`，两者都是 reqwest 官方特性自带的可选依赖）。

### 哪些请求走代理、哪些永远不走

走代理（全部是 https 公网目标）：

| 出口 | 位置 |
| --- | --- |
| AI 接口、模型列表、润色测试 | `client-providers/src/ai/endpoint.rs` 的 `blocking_client_builder`；`ai/settings.rs`、`credential/probe.rs`、`apps/desktop/src-tauri/src/shared/mobile_ai.rs` 都经它 |
| 翻译配置检测（腾讯、小牛、自定义） | `client-providers/src/credential/translation.rs` |
| 语音识别配置检测（HTTP 批量） | `client-providers/src/credential/asr.rs` |
| 词库更新（GitHub API 与附件） | `client-providers/src/dictionary_updates.rs` |
| 本地语音模型与资源包下载（GitHub release 与镜像） | `client-providers/src/voice/local_models.rs` |
| 账号、社区、遥测、公告、云词库、AI 皮肤、诊断上传 | `client-services/src/account/client.rs` 的共用客户端 |
| 头像下载 | `client-services/src/account/avatar.rs` |
| 自建云剪贴板中继 | `client-services/src/clipboard_relay.rs` |
| 应用更新检查 | `client-services/src/update_check.rs` |

永远不走代理：

- 明文 `http` 目标，包括 AI 的本机 Ollama（`http://127.0.0.1:11434`）、局域网 LLM、自定义翻译只允许的 https 之外的回环地址。
- 本机与局域网 https 目标。
- 语音模型的回环测试服务（`local_models.rs` 的 `https_only == false`，仍然 `no_proxy()`）。
- 云候选与候选翻译在**输入法进程里**直接发出的那部分：macOS 走 `NSURLSession`（`platforms/macos/src/cloud/CloudCandidateRequest.mm` 已对 http 目标设空 `connectionProxyDictionary`），Windows 走 libcurl（对 `http://` 目标设 `CURLOPT_PROXY` 为空）。这两处是平台原生网络栈，不读这份偏好，继续只跟随系统代理。

### 安全边界怎么保住

`ai/endpoint.rs` 的原有约束原样保留，并向前延伸一步：`blocking_client_builder` 对 `http` 地址仍然直接 `no_proxy()`，注释不变；对 `https` 地址改为经 `proxy::configure_blocking_for(builder, url)`，它内部再跑一次 `should_use_proxy(url, current().effective_mode(), is_local_host(url))`，本机和局域网目标得到的是 `no_proxy()`。所以代理只会看到目标主机名（CONNECT）和 TLS 密文，看不到 API Token、提示词或输入内容；而且用户填的 HTTP 代理只对 https 目标生效，代理本身即使在公网上也拿不到明文。

固定源站的账号与中继客户端使用 `proxy::configure_blocking_for`，三种模式都先排除明文与本地目标。`proxy::configure_blocking` 只用于公网 HTTPS 客户端：`custom` 挂 `reqwest::Proxy::custom` 逐目标判断，`system` 保留 reqwest 系统代理；不能把它用于可能访问本机的源站。这两个 `proxy.rs`（providers 和 services 各一份）内容相同：两个 crate 同级、互不引用，而 `client-core` 的编译图里有意不放 reqwest。判据只写在 `client-core` 一份。

### 设置如何到达出口

`crates/client-core/src/network.rs` 有一份进程内的 `RwLock<NetworkPreferences>`。`PreferencesStore::load`、`read_locked` 的「没有文件」分支和 `save_locked` 都调用 `network::install`，所以任何读到或保存了偏好的进程（设置窗口、输入宿主、MCP）接着发出的请求都用最新的代理。默认是 `system`，没有偏好文件时也不会突然直连。

账号默认连接池在进程内共用，宿主自报 User-Agent 的对象各自缓存连接池。每条请求路径（包括 multipart、快照、流式下载和头像）在发出前检查 `network::current`，设置变了才重建；长期缓存的账号会话同样生效。构造对象时重建不够，因为 FFI 还缓存着旧对象。建立连接池和记录代理版本使用同一份设置快照，已开始的请求可继续使用旧池。

设置页在「AI 辅助」页新增「网络」一组（macOS、Windows、Linux 桌面；iOS、Android、鸿蒙的请求走平台自己的网络栈，不显示）。`proxy_server` 的本地检查与 Rust 的 `valid_proxy_server` 同口径；不合规时 `Preferences::validate` 返回 `InvalidNetwork`，整份保存失败，而不是存下一个用不了的地址。

`network` 进 `settings_document::LOCAL_SECTIONS`：代理地址常写内网主机和端口，只对这台机器的网络成立，导出设置文件时不写、导入时保留本机的值。恢复默认设置时也保留代理（与它保留服务凭据的理由相同：代理没了，所有出口一起断）。

## Alternatives considered

**在 `client-core` 里直接建 reqwest 客户端。** 判据和使用只在一处，providers 和 services 都不用各自维护一份 `proxy.rs`。但 `client-core` 的编译图里有意没有 reqwest、tokio 这些网络栈（`input-runtime` 和 `backend-engine` 都依赖它，ARCHITECTURE.md 明确写了这两个的编译图里没有 reqwest），为这个功能把它们拖进来，代价远大于两份二十行的转发。

**只对 `custom` 挂代理，`system` 完全不动。** 改动更小，也不用打开 `system-proxy` 特性。但 reqwest 关掉默认特性后 `system` 其实读不到 macOS 系统设置和 Windows 注册表，只有环境变量；而「跟随系统代理」在桌面上的含义就是系统设置里那个代理，环境变量是开发机才常见的东西。所以打开 `system-proxy`。

**把 `should_use_proxy` 的 `is_local_host` 参数去掉，函数内部自己判断。** 调用点更少一行。但目标主机的本机/局域网判断要和 `http://` 接口地址规则共用同一份实现（否则一个地址可能被判成公网却仍算本机，代理就会收到明文 Token），而那份实现要处理「不做 DNS 解析」的规则；把判断留在参数里，纯函数才能用假的本机标志覆盖全部组合而不碰网络。

**让平台原生的云候选和候选翻译也读这份偏好。** 那才是「全部出口」都覆盖。但那两处是 macOS 的 `NSURLSession` 和 Windows 的 libcurl，要在两套原生代码里再写一遍 `should_use_proxy`，并各自维护一份 `shared/contracts` 用例；这次先覆盖 Rust 的出口，原生那两处继续只跟随系统代理，写进下面的未覆盖清单。

## Consequences

- **收益**：网络受限地区可以用一个 `127.0.0.1:7890` 让设置页、AI、翻译检测、语音检测、账号、更新检查和所有下载重新连上；三种模式与 Windows 正式版同名同义，旧配置文件里的 `[network]` 值可以直接照抄。判据是纯函数，五种模式 × 三类目标的组合都有单测，不需要真网络。
- **代价**：`Cargo.lock` 多两个可选依赖（`system-configuration`、`windows-registry`），只在 macOS 和 Windows 上编译。`network` 这一节在偏好文档里多一行；等于默认值时不写，旧版本读得了。
- **已知未覆盖**：macOS 的 `NSURLSession` 云候选、候选翻译、账号界面和 Windows 的 libcurl 云候选、候选翻译、AI 候选只跟随系统代理，不读 `network.proxy_mode`；Linux 的 `ln` 与 voice provider 是用户的 Python 服务，自己在进程里读系统代理，设置页在这一平台上仍显示「网络」组，但只对设置页自己的请求和词库/模型下载生效。要覆盖这几处，得在原生代码里再实现一遍判据并加进 `shared/contracts`。

## Verification

- 组合覆盖：三种模式 × 公网 https / 本机 http / 本机 https 每格一个具名用例（`system_proxies_public_https`、`system_direct_connects_cleartext_loopback`、`system_direct_connects_local_https`、`custom_proxies_public_https`、`custom_direct_connects_cleartext_loopback`、`custom_direct_connects_local_https`、`none_direct_connects_public_https`、`none_direct_connects_cleartext_loopback`、`none_direct_connects_local_https`），再加 `custom_without_a_server_falls_back_to_the_system_proxy`、`cleartext_targets_are_never_proxied`、`proxy_server_shapes`、`installed_preferences_are_what_the_egress_reads`、`local_hosts_are_shared_with_the_http_endpoint_rule`，共 14 个。
- 改前失败、改后通过：把 `should_use_proxy` 与 `normalized_proxy_url` 换成桩（分别返回 `false` 和 `None`）后 `cargo test -p msime-client-core network::` 是 `10 passed; 4 failed`（失败的是 `system_proxies_public_https`、`custom_proxies_public_https`、`custom_without_a_server_falls_back_to_the_system_proxy`、`proxy_server_shapes`；「不走代理」的那些格子在桩下必然通过，所以真正说明问题的是走代理和有地址校验的那四个）。恢复实现后 14 个全过。
- `cargo test -p msime-client-core -p msime-client-providers` 全绿（438 / 231 个用例）。
- `cargo fmt --all --check`、`cargo clippy -p msime-client-core -p msime-client-providers -p msime-client-services --all-targets --locked -- -D warnings` 通过。
- `cargo check -p msime-host-api -p msime-input-runtime --all-targets` 通过（跨 crate 检查）。
- `pnpm --filter @msime/desktop test` 785 个文件、4852 个用例全绿；`pnpm format:check` 通过。

审查回归：显式标准端口 `:80` / `:443`（含前导零、IPv6）的代理地址与设置页采用同一规则；未写端口、端口零、空用户凭据与空查询拒绝。账号默认和自报 User-Agent 的长期客户端实际请求先后通过两台合成 CONNECT 代理，下一次请求使用新池；本机剪贴板中继在设置系统代理环境后仍只把合成令牌发给源站。两项传输回归分别在独立测试进程运行，避免全局偏好及代理环境影响并行测试。三 crate 严格 Clippy、网络规则 14 项、设置页 4 项、类型检查、生产构建与远端 `verify-local.sh --quick` 通过；quick 包含 macOS 与共享 Apple 桥构建、Android JNI/JVM、Linux 原生 108 项 CTest 和 Linux 设置外壳检查。Windows 编译在远端缺交叉工具链而跳过，仍由 PR 的平台 CI 把关；HarmonyOS 真打包未运行。

独立进程隔离后的服务层全套回归通过：`cargo test -p msime-client-services` 为 1,276 项单测与 4 项集成测试通过、1 项忽略，随后严格 Clippy 通过。
