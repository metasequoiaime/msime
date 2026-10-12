# Agent Note: 云剪贴板可以改走自建中继，静态令牌一个令牌一个空间

Status: implemented

## Problem

#6787：用户希望跨设备复制粘贴不依赖水杉官方服务，改走自己部署的中继。[端到端加密](2026-10-11-cloud-clipboard-e2e.md) 之后服务端已经读不到内容，但地址写死在各平台客户端里（Rust 的 `ACCOUNT_ORIGIN`、Android `CloudApi`、HarmonyOS、Swift），鉴权只有水杉账号的 Bearer 令牌，没有任何不经过 `api.msime.app` 的路径（Linux 用户自己运行的 socket provider 除外，而它要自己实现整套协议）。维护者在 issue 里决定先做第一刀：Rust 客户端、参考中继和桌面端的设置，移动端和鸿蒙放到后续。

## Decision

协议差异和部署写在 [`docs/cloud-clipboard-relay.md`](../../../../docs/cloud-clipboard-relay.md)，接口本身仍以 [`docs/cloud-clipboard-e2e.md`](../../../../docs/cloud-clipboard-e2e.md) 为准。这里记为什么这样定。

- **同一组接口，换基地址和凭据**：中继实现端到端加密协议里客户端用到的 12 个接口，挂在用户填的基地址下（可以带路径前缀）。加密、设备批准、核对码全部沿用，中继和官方服务一样只见到密文。issue 下最初设想的「同步口令」已在端到端加密那篇被设备批准取代，中继不再引入口令。
- **鉴权是静态令牌，一个令牌一个空间**：管理员用 `gen-token` 生成令牌，客户端带 `Authorization: Bearer <令牌>`。中继按 SHA-256(令牌) 区分空间，数据库只存摘要；令牌错误一律 401，客户端报 `relay_unauthorized`，不会被当成水杉账号未登录（共享页原本把任何 `_unauthorized` 都归为未登录，现在先认出这一个）。没有登录、刷新和账号体系，几个人共用一台中继就各发一个令牌。
- **中继与账号二选一，配置了中继就只走中继**：偏好 `cloud_clipboard_relay {url, token}` 两项要么都空、要么都合规（`client-core` 的 `valid_relay_url` / `valid_relay_token`，共享页 `cloud-clipboard-relay.ts` 照同一规则先校验）。中继失败不退回账号，否则用户以为在用自己的中继，内容却去了官方服务。这条目前只在 Tauri 的 `cloud_clipboard_request` 里成立：macOS 和 Windows 的原生入口（Windows 设置窗口的「发到云剪贴板」、macOS 表情面板里剪贴板历史的「发到云剪贴板」和原生账号窗口里的云剪贴板、未登录时菜单退回的原生窗口）还没有查 `msime_client_cloud_clipboard_source`，仍按账号走。正因为两条路会并存，设置页只在 Linux 上显示中继字段，协议文档也写明了这一点。Tauri 桌面端的选路顺序是：用户显式配置的 provider socket（`cloud_clipboard_provider_socket` 或环境变量），然后中继，然后 macOS 输入法的原生会话、自动发现的 socket，最后是账号。显式的 socket 在前，因为那是用户为这台机器专门指定的服务；中继在原生会话之前，因为原生会话只代表账号。
- **只允许 https，http 只放行回环**：地址规则与自定义翻译接口相同（`localhost`、`127.0.0.1`、`[::1]` 可以用 http），回环 http 只为本机测试和参考中继的端到端测试。客户端的 rustls 用内置的 webpki-roots，不读系统信任库，所以中继必须用公网可信的证书；局域网部署要走 DNS-01 签证书。
- **令牌放在共享偏好里，和语音、翻译服务的凭据同等对待**：进 `Preferences::credential_slots`（恢复默认时保留，诊断快照里脱敏），地址进 `DIAGNOSTIC_ENDPOINTS`（只留协议、主机和路径），整组进 `settings_document::LOCAL_SECTIONS`（不随设置文件导出），不进 `account/settings_sync` 的白名单映射，MCP 的偏好也是白名单、不暴露它。没配置时这个键不写进文档，旧版本仍能读。
- **Rust 自己联网，客户端在 `client-core/src/clipboard_relay.rs`**：六个平台都已经经 C ABI 让 Rust 发请求（先例是 `msime_client_telemetry_flush`），中继这条路的全部逻辑因此只写一份：`RelayClipboard` 用 `clipboard_e2e::run` 跑列表、上传和设备管理，开关、删除、清空、置顶和保留期直接请求。它不放在 `cloud` 下，因为 `cloud` 下的模块约定不开连接；也不复用 `BackendAccountClient`：那个客户端用 `origin.join(path)` 拼地址会丢掉路径前缀，还有五处直接 join，user agent 写死成 Android，而账号、词库、头像等请求全经过它。中继客户端自己建 reqwest 客户端：不跟随重定向、30 秒超时、只接受驱动生成的那几种路径字符，拼完再核对协议、主机、端口和路径前缀。
- **置顶、保留期、清空只在中继的动作集里**：`host-api` 的 `validate_relay_request` 在共用的九个动作外多接受 `pin`、`set_retention`、`clear`。Tauri 桌面命令先核对中继动作格式，再按选路结果限制这三个动作：只有选中中继时才放行；若选中 socket、原生会话或账号，返回 `invalid`，不会先把请求发给它们。原生的 Android、iOS、macOS 界面已经有这三个功能，以后改走中继的 C ABI 时不能退化；账号路径的 `validate_request` 和 `account_request` 不变。C ABI 是 `msime_client_relay_cloud_clipboard`（跑一个动作）和不联网的 `msime_client_cloud_clipboard_source`（告诉宿主走哪条路）。
- **密钥串单独存一份，绑定中继和空间**：`cloud-clipboard-relay-keystore` 放在宿主给的目录（桌面端是账号会话的目录），内容是 `{relay, space, keystore}`，经 `FileAccountSessionStorage` 的私有文件读写（0600、原子替换、拒绝符号链接和别人可读的文件）。地址或令牌换了就当作没有，驱动生成新的设备身份；退出水杉账号不删除它。整个驱动操作持有 `cloud-clipboard-relay.lock`（经 `client-core::file_lock`），同一台设备上几个进程第一次使用时不会各自登记一台。
- **参考中继在本仓库，以文档为准**：`crates/clipboard-relay`（`msime-clipboard-relay`）的接口逻辑在不碰网络的 `RelayStore::handle` 里，SQLite 一个文件，每个请求一个事务；axum 0.8（维护者同意引入）只做 HTTP/1 外壳，请求体上限 64 KiB，中继只说 HTTP、放在 TLS 反向代理后面。它加进 workspace members，不进 default-members。官方服务 msime-cloud 是另一份独立实现，两边各自用测试锁住同一份文档。新空间默认开启、条目的 `device` 为空，这两处是中继和官方服务有意不同的地方，写在协议差异里。

## Alternatives considered

- **各宿主分别把 origin 换成可配置** — 改动最直接，六套 HTTP 客户端各加一个地址设置即可，不用新增 C ABI。不用：Rust、Swift、两套 Java 和 ArkTS 五份实现都要处理路径前缀、鉴权换成静态令牌、401 不再刷新这些差异，账号路径的每个调用点都要分辨现在是账号还是中继；而云剪贴板的流程已经由 Rust 驱动统一，联网也收进 Rust 只多一层。
- **扩展 `BackendAccountClient` 支持路径前缀和第二种凭据** — 复用现成的超时、响应上限和错误映射。不用：`prepare` 之外还有五处直接 `origin.join`，改一处会让账号路径出现两种拼接方式；它服务于账号、词库、头像、社区等全部请求，为中继改它的回归面远大于中继本身。
- **msime-cloud 出一个单机模式代替本仓库的参考中继** — 只有一份服务端实现，协议不会在两份实现之间漂移，Postgres 上的行为已经测过。不用：用户要部署的是一个小程序，不想带 PostgreSQL 和账号体系；本仓库的客户端也需要一个能在测试里随手起来的真实服务端做端到端测试。代价是协议有两份服务端实现，靠文档加各自的测试保持一致。
- **允许局域网 http（像 AI 接口那样放行私有地址段）** — 家里的 NAS 上部署最省事，不用证书。不用：中继令牌和密文会明文经过局域网，局域网也不总是可信（公共 Wi-Fi、访客网络）；内容虽然加密，令牌泄露就能删光别人的剪贴板。回环 http 只留给本机测试。
- **自签证书的指纹固定** — 局域网部署不用申请证书。这次不做：要在设置里多一个指纹字段、在 rustls 上接自定义校验，各平台原生宿主接入时还要各做一遍；DNS-01 能给局域网主机名签公网证书，先用它。

## Consequences

- **收益**：不登录水杉账号也能用云剪贴板，内容和元数据都只在用户自己的中继上；中继拿不到密钥，和官方服务的安全性质相同。协议、客户端和参考中继在同一个仓库里，端到端测试用真实客户端连真实中继。
- **代价**：中继的密钥串在所有平台上都是应用私有目录里的文件，弱于账号路径在 iOS 上的钥匙串和 Android 上的 AndroidKeyStore 加密。令牌明文在偏好文件里，和语音、翻译的凭据一样靠文件权限保护。协议有两份服务端实现要保持一致。新增依赖 axum 0.8（带进 axum-core、matchit、httpdate），只有参考中继用。
- **已知缺口（后续改动）**：设置页的中继字段只在 Linux 显示。Windows 的设置窗口是原生 WinUI，要在 `platforms/windows/settings/main.cpp` 画出同步方式、地址和令牌，并按 `msime_client_cloud_clipboard_source` 选用中继的 C ABI；macOS 输入法菜单的「云剪贴板…」未登录时退回原生窗口（`DesktopCloudClipboard.mm`），要在配置了中继时直接打开 Tauri 面板；Linux Fcitx5 键盘的云剪贴板栏只认 provider socket（`FcitxEngine.cpp`），没有 socket 但配置了中继时要改走中继的 C ABI。Android 原生宿主已经经 JNI 接上，设置页在「我的 → 云剪贴板」，密钥串由宿主用 AndroidKeyStore 加密、请求经只发请求的 `msime_client_relay_cloud_clipboard_exchange`，取舍见 [Android 接入自建中继](2026-10-12-android-cloud-clipboard-relay.md)。iOS（App Group 容器里的密钥串，并排除备份）和 HarmonyOS（NAPI；共享沙箱落地前键盘读不到偏好，只有设置应用能用）还没接，设置页因此不在这些宿主上显示中继；Tauri 移动端的 `cloud_clipboard_request` 在配置了中继时拒绝（`relay_unsupported`），不走账号。账号页里「云剪贴板」入口仍按是否登录显示，Linux 上配置中继后从「剪贴板」页打开面板。PRIVACY.md 的云剪贴板一节已写明中继的数据流向，各平台接入时再补对应平台的说明。

## Verification

`client-core`：`clipboard_relay_tests.rs` 覆盖地址和令牌规则、规范化与路径前缀拼接、越界路径（`..`、`//`、百分号编码、片段、`@`）被拒、令牌不进 Debug 输出、错误码映射、密钥串的 0600 权限和原子替换、换地址或换令牌后失效、别人可读的密钥串被拒、非法请求不出本机；偏好测试覆盖校验、序列化省略、凭据清单、诊断快照脱敏和恢复默认保留；设置文件测试锁住导出不写、导入不带；设置同步测试锁住 Android 与桌面导出都不含中继。`host-api`：中继动作集与账号动作集的差别、`source` 与 `configured_relay`、两个 C ABI 的非法输入和未配置。`msime-clipboard-relay`：`store` 的单元测试，以及 `tests/flow.rs` 在 127.0.0.1 上起真实中继、用两台 `RelayClipboard` 跑完整流程（首台建钥、第二台待批准、核对码一致、批准、互传、去重、搜索、置顶、保留期、删除、移除后换钥、重新批准、重置、清空、关闭开关），以及错误令牌、两个令牌的空间隔离、重启后数据还在（同一地址）、超限请求体 413、没有令牌 401。Tauri：选路顺序、专有动作只进中继、非法动作不触发后端、偏好读出中继的单元测试；macOS 上 `cargo test` 与 `clippy -D warnings` 通过。共享页：`apps/desktop/tests/settings/cloud-clipboard-relay.test.tsx`（规则与 Rust 用例一一对应、去掉配置时删键、令牌被拒不算未登录、剪贴板页填写和保存、改回账号）以及原有的设置页和云剪贴板面板测试。

没有验证的：真实 TLS 反向代理后的中继、装机后的 Linux 设置窗口和面板、Windows 上 `cfg(not(unix))` 分支的编译（编译机没有 `x86_64-pc-windows-gnu` 目标，要在装了 MinGW 的机器上跑 `cargo check -p msime-desktop --target x86_64-pc-windows-gnu --lib`）。合并前要在 Linux 上对着一台部署在 https 下的参考中继走一遍：填地址和令牌、两台设备批准、互传、移除、重置、改回账号。
