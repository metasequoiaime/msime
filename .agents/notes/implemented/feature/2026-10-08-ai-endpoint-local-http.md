# Agent Note: AI 辅助接口允许本机和局域网的 http 地址

Status: implemented

## Problem

#5825：AI 辅助的自定义接口地址只收 `https://`。LM Studio、Ollama 这类本地模型服务默认只开 http（LM Studio 是 `http://localhost:1234/v1`），用户在手机上连局域网里那台电脑时地址是 `http://192.168.x.x:1234/v1`。填这样的地址后设置页不让填 API Token、不能获取模型列表，键盘也不会发 AI 候选请求，本地模型完全用不上。

各平台原本的规则也不一致：client-core 生成 AI 候选请求时已经放行 `localhost`/`127.0.0.1`/`[::1]` 的 http（借用了翻译的 `is_secure_endpoint`），设置页、Android、iOS、Windows 的传输层却都只认 https，于是连本机的 http 也走不通。

## Decision

- 规则：`https://` 不限主机；`http://` 只能指向本机或局域网，即回环（`localhost`、`127.0.0.0/8`、`::1`）、RFC 1918 私有 IPv4（`10/8`、`172.16/12`、`192.168/16`）、`100.64.0.0/10`（运营商级 NAT 与 Tailscale）、链路本地（`169.254/16`、`fe80::/10`）、IPv6 唯一本地地址 `fc00::/7`，以及 `.local` 主机名（标签非空、不带结尾点）。公网主机的 http 地址一律拒绝，并在界面上说明原因：http 只能用于本机或局域网地址，公网服务请用 https，以免 API Token 明文经过互联网。
- 检查时不做 DNS 解析：除 `localhost` 和 `.local` 以外的域名都按公网处理，`http://lmstudio:1234`、`http://nas.home.arpa` 这类局域网域名也会被拒绝，要写成 IP 或 `.local`。
- 用户名、密码、`#` 片段（包括空片段）、控制字符、缺主机、`scheme:///path`、超出 65535 的端口和 http(s) 以外的协议仍然无效，与原来一致。
- 保存 Token 的来源键是 `scheme://host:port`：主机小写，端口总是写出来（https 默认 443，http 默认 80），IPv6 带方括号。原有 https 地址的来源键不变，已存的 Token 不受影响。
- 权威实现是 `crates/client-core/src/ai/endpoint.rs`（`validate`、`credential_origin`、`is_local_network_host`）。`chat_completion_http_request`、Tauri 侧的桌面模型列表与测试（`apps/desktop/src-tauri/src/ai.rs`）、iOS 的移动端请求（`shared/mobile_ai.rs`，只在 https 地址上要求 `https_only`）、iOS 键盘偏好的 Token 来源键（`lib.rs` 的 `ios_keyboard_ai_preferences`）和 Linux 的 `ai-provider.json` 写入都调用它。
- 设置页（`packages/ui/src/settings/ai-endpoint-policy.ts`）、Android、iOS、macOS、Windows、Linux provider 脚本和 HarmonyOS 键盘要在本地判断（填 Token 前、发请求前），各有一份同规则的实现。它们都逐条跑 `shared/contracts/ai-endpoint/cases.json`；改规则时先改 Rust 和这份用例。
- 所有发往 AI 接口的请求仍然不跟随重定向，http 的局域网请求不会被转到别的主机；http 请求一律直连、不经过系统或环境变量里的代理（client-core 的 `blocking_client_builder`、Windows 的 `CURLOPT_NOPROXY`、Linux provider 的 `request_opener`、Android 的 `Proxy.NO_PROXY`、iOS 与 macOS 会话的空 `connectionProxyDictionary`），否则代理会收到明文 Token，而代理本身可能在公网上。https 请求照旧走代理。

### 各宿主

- Android：`platforms/android/java/app/msime/android/voice/AiEndpointPolicy.java`，`AiPolishConfiguration`、`AiPolishHttpTransport`、`AiPolishModelCatalog`、AI 候选的 `OnlineCandidatePolicy.validAiURL` 和设置页 `AiSettingsPage` 都经它判断。targetSdk 28 起系统默认拒绝明文，而 network-security-config 只能按域名放行、写不出地址段，所以 `res/xml/network_security_config.xml` 用 `base-config cleartextTrafficPermitted="true"` 整体放开，不改信任锚；边界在应用层，其他网络路径（云候选、账号、同步、社区、更新、语音识别、翻译）各自只接受 https。
- iOS：`platforms/ios/SharedUI/core/AIEndpointPolicy.swift`（App 与键盘扩展共用），`CustomService` 只在 AI 类型上放行局域网 http（语音仍只收 https/wss），键盘的 AI 面板、AI 候选请求（`OnlineCandidateRequest.allowsLocalHTTP`，云候选和翻译不放行）和钥匙串里 Token 的来源键都经它判断。Tauri 插件编不进 SharedUI，`crates/tauri-mobile-platform/ios/Sources/AIEndpointPolicy.swift` 是同样的一份。iOS 10 起 ATS 本来就不拦 IP 字面量、不带点的主机名和 `.local`，App 与键盘扩展的 Info.plist 仍显式写上 `NSAllowsLocalNetworking`（只覆盖本地，不放开任何公网域名），并补上 `NSLocalNetworkUsageDescription`：iOS 14 起连局域网要经过本地网络授权，回环不需要。
- macOS：`platforms/macos/src/core/AIEndpointPolicy.h`，AI 联想设置窗口和执行 AI 描述符的 `CloudCandidateRequest` 用它；翻译描述符仍只放行回环。`Info.plist.in` 同样写上 `NSAllowsLocalNetworking` 和中英文的 `NSLocalNetworkUsageDescription`（macOS 15 起有本地网络授权）。
- Windows：`platforms/windows/src/candidate/AiEndpointPolicy.h`，AI 候选请求按它决定 curl 的协议（http 或 https），http 时设 `CURLOPT_NOPROXY "*"`。测试在配置时把 `cases.json` 编进测试程序，Wine 下也能跑。
- Linux：`platforms/linux/scripts/msime-linux-online-provider` 的 `ai_endpoint_check`（`urllib.parse` 与 `ipaddress`），`load_ai_config` 用它，覆盖 AI 候选、模型列表、润色测试和凭据测试。
- HarmonyOS：键盘侧 `entry/src/main/ets/keyboard/settings/AiEndpointPolicy.ts`，模型列表、设置页的测试请求和键盘发 AI 候选前都经它判断；设置页本身是共享的 `packages/ui`。系统 http 模块在没有 `network_config.json` 时允许明文，没有新增配置。
- 手写解析的宿主（Android、iOS、macOS、Windows、HarmonyOS）只认规范写法：IPv4 必须是四段十进制、不带前导零，`10.1`、`0x7f000001`、`010.0.0.1` 这类写法不会被当成局域网地址，因为 curl 或系统解析器可能把它们读成别的地址（`010.0.0.1` 按八进制是 8.0.0.1）。
- 翻译接口（`translation::is_secure_endpoint`）不在这次范围内，仍然只放行回环的 http。

## Alternatives considered

- **放开所有 http 地址** — 最简单，任何自建服务（包括挂在公网 IP、没配证书的 one-api / 反代）都能直接填。但这时 API Token 和用户输入的上下文会明文经过互联网，任何一段链路上的人都能读到并盗用 Token；输入法发出去的是用户正在打的字，这个代价不能由一个设置项悄悄承担。真需要接公网服务的用户可以给它配上 https（反代或隧道），成本远低于泄露。
- **维持只收 https** — 安全边界最清楚，不需要在每个宿主各维护一份网段判断。但本地模型服务几乎都只开 http，用户只能自己给局域网服务签证书再让手机信任，等于这个功能对本地模型不可用；而局域网内明文的风险与用户自己的网络是同一个信任域，不值得为它挡掉整类用法。
- **按 DNS 解析结果判断域名是否在局域网** — 能让 `http://nas.home.arpa` 这类名字也可用。但解析结果随所在网络变化，同一个公网域名在某个 Wi-Fi 下解析到私有地址时就会放行，Token 下一次可能明文发往公网；检查和连接之间还会有解析结果不一致的窗口。只认字面量和 `.local` 让判断与网络环境无关。

## Consequences

- **收益**：LM Studio、Ollama 等本地或局域网的模型服务可以直接配置（填 Token、获取模型列表、测试、键盘 AI 候选）；设置页、client-core 和六个宿主的判断第一次有了同一份用例，以前「Rust 放行回环 http、宿主却拒绝」这种不一致会被测试抓到。
- **代价与已知上限**：网段判断在 Rust、TypeScript、Java、Swift、Objective-C、C++、Python 和 ArkTS 里各有一份（iOS 的 Tauri 插件还有一份 Swift 副本），只能靠共享用例保持一致；共享用例只收各平台解析器结论一致的写法，`http://10.1`、十六进制 IPv4 这类非规范写法在 WHATWG 解析的平台（Rust、设置页）会按规范化后的地址判断，在手写解析的宿主上不会被当成局域网地址（按无效或公网拒绝），拒绝的方向是安全的；同一个非规范地址可能在设置页通过、键盘却不发请求。局域网里的 http 仍然是明文，同一网络里的其他设备能看到 Token 和输入内容，这是用户选择本地服务时接受的风险。若以后要支持更多局域网命名（`.home.arpa`、`.lan`）或 DNS 解析，需要重新评估上面的第三条备选。

## Verification

逐条跑 `shared/contracts/ai-endpoint/cases.json` 的测试：`cargo test -p msime-client-core --lib ai::endpoint`；`apps/desktop` 的 `vitest run tests/settings/ai-endpoint-policy.test.ts`；Android 的 `tests/voice/AiEndpointPolicySmoke.java`（`bash platforms/android/check-host.sh`）；iOS 的 `KeyboardTests/settings/AIEndpointPolicyTests.swift`（`xcodebuild test -scheme MSIMEClientTests`）；macOS 的 ctest `ai-endpoint-policy`；Windows 的 `windows-ai-endpoint-policy`（构建时把用例编进测试程序）；Linux 的 `platforms/linux/tests/provider/ai_service_contract.py`；HarmonyOS 的 `bash platforms/harmony/tests/run.sh`。没有在真机上连过局域网里的模型服务，各平台的本地网络授权弹窗也没有验收过。
