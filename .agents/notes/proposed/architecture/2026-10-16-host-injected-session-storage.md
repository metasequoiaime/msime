# Agent Note: 账号会话由宿主注入存储，账号与云端逻辑只在 Rust 实现一份

Status: proposed

## Problem

账号、匿名账号、云词库、云剪贴板、社区、AI 皮肤与对话、账号释义、设置云同步这几块，除了 Rust（`crates/client-services`、`crates/client-providers`）以外，还在 Apple 共享的 Swift 包 `shared/backend`、Android 的 `account/` 与 `community/`、HarmonyOS 的 ArkTS 账号桥和 Linux 的 Python online provider 里各写了一遍（#8201）。同一个服务端接口有三四份客户端，重试、错误码和校验上限各不相同；macOS 上 Swift 和 Rust 读写同一个会话文件，任何一边改了格式或锁，另一边都不会知道。

把这些逻辑收进 Rust 的前提是 Rust 能拿到会话，而会话放在哪里由平台决定：iOS 在钥匙串和 App Group，Android 在 AndroidKeyStore 加密的 `SharedPreferences`，macOS 的匿名会话在 App Group 里的 `BackendLocalStore`。host-api 现有的账号入口（`msime_client_account`、`msime_client_community`、`msime_client_account_cloud_clipboard`、`msime_client_account_access_token`）只认 `directory`，即 `FileAccountSessionStorage` 的明文 JSON 文件，平台没法把自己的存储交给它。

刷新令牌每次刷新都轮换，服务端对 30 秒内重复出示的旧令牌回 409 `refresh_superseded`，超过 30 秒就吊销整个会话（msime-cloud `internal/account/store.go` 的 `refreshSupersededGrace`）。所以凡是读写同一份会话的进程，必须共用同一把锁、每次重读存储；新旧实现共存期间只要有一方不守这条，用户就会被登出。macOS 现在就有这个缺陷：`BackendAccountWindow` 的默认参数给同一个 `anonymous-session.json` 另建了一个 `BackendAccountSession`，而 macOS 上带 `storage` 的初始化用的是进程内什么也不做的 `BackendProcessRefreshLock`，两个 actor 可能先后用同一枚刷新令牌去刷新。

## Proposal

### 已定的决策（仓库负责人）

- **会话存储由宿主注入。** 宿主经 load、save、lock 一组回调把存储交给 Rust，钥匙串和 AndroidKeyStore 的读写原语留在平台；现有的文件存储作为默认实现保留，Windows 和 Linux 不变。为此不引入任何钥匙串或 KeyStore 的第三方 crate。
- **行为冲突以 Rust 为准。** 平台现有行为与 Rust 不一致时以 Rust 为准，每一处差异记进 #8201 第一阶段 PR 正文的差异清单。
- **两阶段。** 第一阶段只改 Rust（host-api 补齐接口），不动平台调用方，例外是 macOS 的两处 Swift 修复；第二阶段各平台改调 Rust，合成一个 PR。

### C 接口

`msime_client_session_store_register(const msime_client_session_store *)` 收下一个版本 1 的结构体：`version`、`layout`（0 是 Rust 的 `SavedAccountSession`，1 是 Swift 的 `BackendSavedSession`，读取时两种都接受）、`flags`（`MSIME_CLIENT_SESSION_STORE_SHARED`）、可选的 `client_agent`、`context`，以及 `load`、`save`、`remove`、`lock`、`unlock`、`release` 六个回调。存储里有四项：登录会话 `ACCOUNT`、云剪贴板密钥串 `CLIPBOARD_KEYSTORE`、匿名会话 `ANONYMOUS`、匿名身份 `ANONYMOUS_IDENTITY`；`lock`/`unlock` 带 `item`，只会是 `ACCOUNT` 或 `ANONYMOUS`，宿主按 item 选锁文件。注册成功返回 `{session_store: id}`，四个账号入口的请求用 `session_store` 代替 `directory`，两者恰好给一个。`msime_client_session_store_unregister(id)` 注销。全部契约写在 `crates/host-api/include/msime_client.h` 的注释里，Rust 侧是 `msime_client_services::account::host_storage` 和 `crates/host-api/src/ffi/session_store.rs`。

### 锁与线程的不变量

- 锁顺序固定为：进程内按（store，槽）的 `Mutex` → 宿主 `lock(item)` → `BackendAccountSession` 的会话锁 → `load`/`save`/`remove`。宿主的 `load`/`save`/`remove` never 等刷新锁，否则成环。
- 同一线程在持有某个 store 的任何一把刷新锁时再对同一个 store 加锁，Rust 直接返回 `account_unavailable`（线程局部的持锁登记），不去等一把自己拿着的锁。登记必须先于拿进程内 `Mutex` 检查，否则同一线程第二次拿同一把 `Mutex` 会把自己锁死。
- 宿主 `lock` must 用 flock(2)，每次加锁新开一个 fd；fcntl、lockf 按进程计锁，Java 的 `FileChannel.lock` 底层是 fcntl，与 flock 互不可见。本进程里的并发由 Rust 的进程内 `Mutex` 排斥，所以宿主只需负责跨进程。
- 每次加锁成功后，Rust 在同一线程上用同一个 item 和令牌调用 `unlock` 恰好一次，出错或 panic 时由 Drop guard 保证。
- 存储回调可能在任意线程上被调用，包括 Rust 自己起的线程；JNI 宿主每次回调先 `GetEnv`，必要时 `AttachCurrentThread` 并在返回前 `DetachCurrentThread`。
- `release` 只在注册成功后、最后一个引用释放时调用恰好一次；校验失败或 HTTP 客户端建不出来时 Rust 不接管 `context`。注册表每个 store 一个条目，注册时一次建好登录会话、匿名会话和社区服务（共用一个 HTTP 客户端），请求路径只查不插，所以 `unregister` 之后不会有调用把会话重新插回去。
- 目录模式按账号目录缓存的会话、社区服务和 AI 皮肤服务，以及其他 `BackendAccountClient::new()`（正式服务端、默认 User-Agent）的调用方，共用进程里同一个 reqwest 阻塞客户端：每个阻塞客户端各带一个后台线程和事件循环（macOS 上三个文件描述符），缓存又从不释放，各建一份时目录一多就占满默认 256 的打开文件上限。客户端里只有连接池、不跟随重定向和 30 秒超时，没有 cookie 存储和默认请求头，令牌每个请求单独带，所以共用不会串号；带 `client_agent` 的注入存储和测试连本机回环的客户端各建各的；目录模式自报的 `client_agent`（决定 1）每个 User-Agent 在进程里共用一个客户端。

### 各平台怎么接

| 平台 | 注册方 | `ACCOUNT` 的锁 | `ANONYMOUS` 的锁 | 布局 | flags |
|---|---|---|---|---|---|
| iOS | App 和键盘扩展各自注册 | App Group 下 `backend-account-refresh.lock` | 同一个文件 | 1 | SHARED |
| macOS | 输入法进程和账号窗口所在进程 | Application Support 下 `account-refresh.lock`，与 Swift 的 `BackendDesktopSessionFile.refreshLock` 和 Tauri 设置应用相同 | App Group 下 `backend-account-refresh.lock` | 1 | SHARED |
| Android | 只有主进程；`:ime` 继续经 `AccountSessionProvider` 从主进程取令牌 | 可为 NULL | 可为 NULL | 0 | 0 |
| HarmonyOS | 不注入，迁移时整端（应用和键盘扩展）切到目录模式 | — | — | — | — |

共存规则：凡是不经锁回调就能刷新同一份会话的尚未迁移代码（例如 Android `BackendAccount` 的 `FutureTask` 单飞），必须和 Rust 在同一次迁移里一起切换。

### macOS 的两处 Swift 修复

- `BackendAnonymousAccount.session` 显式用 `BackendFileRefreshLock.appGroup`，`BackendAccountWindow` 的默认参数改用这个共享对象：每个进程一个匿名会话对象，刷新跨进程互斥、每次重读存储，与 iOS 相同。锁和数据都在 App Group 容器里；不用 `BackendDesktopSessionFile.refreshLock`，那把锁在 Application Support，属于登录会话。
- `BackendAccountClient.User` 加上可选的 `email` 和 `avatar_url`，Swift 改写会话时不再丢掉 Rust 写下的这两个字段。

### 后续阶段（同属 #8201 第一阶段）

云词库分发器与 Windows 完整备份（`msime_client_cloud_dictionary`，完整备份依赖 engine，所以放在 host-api 而不是 issue 写的 client-services）、iOS 设置映射、`msime_client_account` 的新操作（translate、phrases、sessions、user data、feedback、诊断上传、chat）和流式对话 `msime_client_account_chat_stream`、社区键盘皮肤、AI 皮肤生成与任务轮询、多分类同步编排。它们都经同一套 `session_store` 字段选会话。

### 当前落地切片（Linux）

Linux 的在线 provider 已把匿名账号注册和账号释义整条切到 `host-api`。Python 进程经 `ctypes` 调用 `msime_client_ensure_anonymous_account_with_agent` 和 `msime_client_account` 的 `translate`，因此身份文件、刷新锁、登录账号优先、令牌轮换、`/v1/translate` 请求和逐项规范化只由 Rust 实现，provider 只在 `account_unauthorized` 时补注册一次匿名账号（见 [Linux provider 的「水杉账号」释义改调 msime_client_account 的 translate](../../implemented/architecture/2026-10-18-linux-account-translation-through-rust.md)）。provider 不再实现或调用 `/v1/auth/challenges`、`/v1/auth/login`、`/v1/auth/refresh`，也不再直接读写匿名账号文件。库路径先取安装时 CMake 写进 `msime_voice_paths.py` 的 `HOST_LIBRARY`，再取 `MSIME_HOST_LIBRARY`，最后按 `<prefix>/lib/<client-directory>/libmsime_host_api*.so` 和源码树查找，便于各 Linux edition 和 rpm、Gentoo 的 `lib64` 共用同一套脚本。

provider 的 HTTP worker 只剩候选、AI 和第三方翻译路径；它们没有账号会话语义，不随账号迁移。测试用假的 Rust bridge 检查 provider 只走 Rust 账号契约，Rust 本身的账号和 `translate` 测试继续覆盖实际校验与存储规则。

### 新接口的错误形状与校验

- 新增的账号操作（`chat`、`chat_models`、`account_delete`、`translate`、`phrases_*`、`sessions_list`、`session_revoke`、`data_*`、`feedback`、`diagnostics_*`）和 AI 皮肤的两个 C 入口（`msime_client_ai_skin_generate`、`msime_client_ai_skin_job`）返回 client-services 的 `AccountFailure`：在 `AccountError` 之外带着 HTTP 状态码、服务端错误码、`Retry-After` 的整数秒和请求有没有拿到响应（`network_failure`）。`AccountError` 一个变体都不加，已有操作的错误码不变：它在 Tauri 的社区命令等处被穷举匹配，把 403 `recent_login_required` 从 `Forbidden` 里分出来会改掉现有 `delete` 返回的 `account_unauthorized`。所以注销账号另开一个 `account_delete`，已有的 `delete` 不动；`chat`、`chat_models` 这两个新操作和 AI 皮肤改走带细节的路径（AccountApi 的 `*_detailed`、`AiSkinApi` 直接返回 `AccountFailure`、`BackendAiSkinService::generate_detailed` 给出 `AiSkinFailure`），Tauri 用的 `chat`、`chat_models` 和 `generate` 把同一次失败收窄回 `AccountError`，码与以前完全相同。新码 `account_recent_login_required`、`account_too_large`、`account_not_found`、`account_identity_changed`、`account_network` 和 C 响应里的 `retry_after_seconds` 只出现在新操作上。
- 断网与服务不可用分开（`account_network`，鸿蒙账号桥原来的码）：请求发出去以后没拿到响应（连不上、DNS 或 TLS 失败、超时、连接在响应头之前断开）时 `network_failure` 为真，`error` 仍是 `Unavailable`，所以经 `error` 取码的已有接口和云同步（按 `status` 为空判断「网络不可用」、`code` 只取服务端的码）都不变；服务端回了 5xx 时照旧是 `account_unavailable`。令牌要先刷新、而刷新请求本身没拿到响应时仍是 `account_unavailable`：刷新走的是只有 `AccountError` 的旧路径，要分开就得给 `AccountError` 加变体。
- 请求按 msime-cloud `origin/main` 的规则在本地完整校验，并对序列化后的请求体做整包预检：走 64 KiB `decode` 的接口超限时服务端回的是 400 `invalid_json`，分不出是哪条规则，所以 `translate`、`chat`、`msime_client_account_gloss` 在本地先报 `account_too_large`。诊断上传对服务端的每条拒绝规则做截断或替换而不只是检查，因为服务端只要有一处不合规就拒绝整份。
- 令牌来源分四类：只认登录账号的（云端数据汇总、删除和注销账号另外附带匿名账号的语音样本，见下）；登录优先退回匿名账号的（释义、反馈、诊断上传，以及按 #8201 的决定 13、16 加进来的对话、模型目录、社区键盘皮肤的下载和安装、举报、AI 皮肤生成与任务，都经 `with_shared_access_token` 或同样的选法，两个会话都没有时错误码与以前相同）；只认匿名账号的（语音贡献和删除语音样本的 `voice_delete`，决定 12：样本不挂在实名账号下）；绑定到上传时 `user_id` 的，对不上时报错，never 退回另一个账号（否则匿名上传后再登录，删除会删到登录账号那份、匿名那份连同 MCP 令牌还活 7 天）。AI 皮肤的一次生成从头到尾留在开始时选中的账号上，因为插画任务归创建它的账号。
- 删除云端数据的 `voice` 分区同时删匿名账号名下的语音样本，在登录账号的分区之后（仓库负责人在 PR #8350 上定的顺序，2026-10-17）：服务端对 `DELETE /v1/users/me/data` 一律要求 10 分钟内建立的会话（`principal(w, r, true)`，匿名账号也一样），而匿名会话通常早就建立了，所以 Rust 先用现有会话试，403 `recent_login_required` 或令牌失效时用保存的匿名身份重新登录同一个账号再删。重新登录计入服务端按地址每天的匿名登录限额（默认 5 次），所以登录账号那一步先做：它要求最近登录（用户最常见的失败）时整个操作在这里结束，不耗匿名额度。匿名这一步失败时登录账号的分区已经删了，操作照样成功，失败放在响应的 `voice_samples` 里，宿主用新的 `voice_delete` 只重做这一步，不会把词库的 reset 再写一遍。
- 注销账号（已有的 `delete` 和 `account_delete`）成功以后同样尽力删掉匿名账号的语音样本，结果同样在 `voice_samples` 里，失败不撤销、也不让注销失败：注销是不可逆的，已经做成的事不能因为附带的一步报成失败，否则宿主会让用户再注销一次、而账号已经没了。`voice_delete` 不要求登录，从没登录过的用户也能删掉自己的样本。云端数据汇总在登录账号的之后再用匿名账号取一次汇总，把 `voice` 分区合进去；这一次失败时汇总照样返回，`anonymous_voice` 标成 `unknown`，因为汇总只是展示，页面上一个分区数不准好过整页打不开。汇总不为此重新登录匿名账号（`GET` 不要求最近登录），也就不耗额度；所以本机还留着匿名身份、会话却已经过期清掉（服务端的会话 30 天后一律过期）时同样是 `unknown`，名下的样本还在，不能报成没有匿名账号的 `none`。
- 服务端响应的结构体不拒绝未知字段。AI 皮肤的 `RawSkinJob` 曾带 `deny_unknown_fields`，msime-cloud 在 2026-10-01 给 202 响应加了 `expires_at` 以后每次创建插画任务都解析失败；回放当前形状的本地测试抓不到服务端以后再加的字段。
- AI 皮肤生成的进度回调在调用线程上依次调用：出图线程只经通道报告完成。与头文件里其他进度回调的约定一致，JNI 和 NAPI 胶水可以直接用调用线程的 env；出图线程遇到 401 时仍可能在 Rust 的线程上调用存储回调，按上面的「任意线程」处理。
- 对话的流式与取消（决定 15）是单独的 `msime_client_account_chat_stream`，因为 `msime_client_account` 没有回调参数；请求、预检和令牌来源与 `chat` 相同，宿主给 `request_id`，`msime_client_account_chat_cancel` 按 id 从任何线程或回调里取消，与 AI 皮肤的生成和取消是同一种形状。先于登记到达的取消（宿主已经调用、Rust 还没登记 id）两张登记表都记下最近 64 个，同一个 id 随后登记时按已取消开始，所以取消过的 id 不能再用。增量回调与进度回调一样在调用线程上依次调用。请求和读取在 Rust 自己的一个后台线程上做，读到的字节经有界通道交给调用线程解析：reqwest 的阻塞客户端不能从别的线程打断一次读取，所以取消让调用线程立刻不再等，连接由后台线程在服务端下一次发数据时（最晚到 125 秒的总时限）断开。开始输出以后的失败按服务端契约分两种：上游失败事件是 `account_unavailable`，没等到 `[DONE]` 的断开是 `account_network`，已经交出的增量不收回。换号核对只在回答完整以后做一次，与 `chat` 相同。
- 给已登录账号添加登录方式（决定 9）是已有登录操作上的 `purpose:"link"`，不是另开一组操作：邮箱、Google ID token 和 Apple 网页登录的申请和完成两步都带它，Rust 不记哪个 challenge 是绑定，与 Apple 网页登录的 verifier 一样由宿主保存。两步都只用登录会话并带着它的令牌，never 用匿名账号；完成时服务端为同一个账号签发一个新会话，Rust 在刷新锁里核对会话代次和账号都没变以后用它替换本地会话（新的登录 ID，推进代次，与 Android 的 `keepSession` 相同），宿主紧接着做要求最近登录的操作时用的就是它。完成一步遇到 401 不刷新重试：验证码不对和令牌失效同是 401 `invalid_credentials`，重试会多耗一次 challenge 尝试。两步的失败都走 `AccountFailure`，否则分不出 `account_recent_login_required`；不带 `purpose` 的登录仍是原来的 `AccountError` 码。

## Related notes

- [原生设置窗口账号页用到的 C 接口](../../implemented/architecture/2026-10-11-windows-settings-account-host-api.md)：部分重叠。那篇定下了 `msime_client_account` 每个 `directory` 一个会话对象、Google 登录与取消用同一对象；这里沿用它，并给同一个导出加上 `session_store`，目录模式的行为不变；目录模式带 `client_agent`（决定 1）时按目录和 `client_agent` 各一个对象，不带时与那篇相同。
- [鸿蒙设置应用与键盘扩展经共享沙箱镜像交换数据](../../implemented/architecture/2026-10-09-harmony-ime-shared-sandbox.md)：部分重叠。鸿蒙的会话在 ArkTS 里、两个进程沙箱分开，是它不走注入、改为整端切到目录模式的背景之一。
- [iOS 原生应用的自动云同步](../../implemented/feature/2026-10-12-ios-automatic-cloud-sync.md)：与本篇无关；同步编排收进 Rust 的决定见下一条。
- [多分类自动云同步的编排收进 Rust](2026-10-16-cloud-sync-orchestration-in-rust.md)：部分重叠。同步的 C 入口沿用这里的会话来源和 `session_store` 字段。
- [Android 的会话存储经 JNI 注入，回调胶水在桌面 JVM 上验证](../../implemented/architecture/2026-10-18-android-session-store-jni.md)：部分重叠。Android 实现这里的 JNI 回调契约（缓存 `JavaVM`、attach 与 detach、清异常），下面「风险」里只能在 Android 迁移时验证的那一条改由 `check-host.sh` 在桌面 JVM 上验证。
- [Apple 宿主经一张函数表调用 Rust，会话存储由 Swift 注入](../../implemented/architecture/2026-10-18-apple-rust-client-table.md)：部分重叠。macOS 与 iOS 实现这里的回调契约（钥匙串和 App Group 的位置、锁文件、失败不当作不存在），以及 Swift 怎样在不链接 Rust 的 dylib 和 `swift test` 里拿到这些 C 入口。
- [旧账号会话迁移不覆盖新登录](../../implemented/bug-fix/2026-10-13-account-session-migration-no-clobber.md)：无关；匿名身份的「不存在才写」沿用的是同一种 no-clobber 思路，但那篇只管目录间的会话迁移。

## Alternatives considered

- **给 `AccountError` 加 `RecentLoginRequired`、`TooLarge` 等变体** — 最强的理由：只有一个错误类型，新旧接口共用一套映射，宿主不用分辨两种形状。否掉：现有调用方对它穷举匹配，新变体要么改掉已有操作返回的码（`delete` 遇到需要最近登录时从 `account_unauthorized` 变成别的），要么在每个匹配点各写一份兼容分支；`Retry-After` 的秒数也放不进一个无数据的枚举。
- **断网用 `AccountError` 的新变体 `Network`、`code()` 仍报 `account_unavailable`** — 最强的理由：刷新令牌这一步也能分出断网，新旧接口共用一份判断。否掉：Tauri 的 `mobile_community.rs` 等处对 `AccountError` 穷举匹配，要跟着改；更麻烦的是到处有 `== AccountError::Unavailable` 的比较（AI 皮肤、同步策略、测试），加了变体以后断网悄悄不再等于它，行为在不报错的地方变掉。
- **断网用 `server_code` 里的本地码表示（像 `request_too_large`）** — 最强的理由：不用给 `AccountFailure` 加字段。否掉：云同步把 `server_code` 原样写进失败记录的 `code` 交给宿主，断网会从 `null` 变成一个看起来像服务端给的码。
- **新接口的请求不在本地完整校验，交给服务端拒绝** — 最强的理由：规则只在服务端写一份，服务端放宽时客户端不用跟着改。否掉：64 KiB 整包超限时服务端只回 `invalid_json`，诊断上传任何一处不合规都整份被拒，用户看到的只是「不可用」；本地截断或预检能给出可操作的码。服务端放宽的那天，本地规则跟着改是一次小改动。
- **流式对话用 reqwest 的异步客户端和一个 tokio 运行时，取消时直接丢掉请求的 future** — 最强的理由：取消时连接立刻断开，服务端马上停下上游、这次请求不计量，与 Android `ChatCall.cancel()` 的 `disconnect()` 相同。否掉：client-services 要直接依赖 tokio、另建一个异步客户端（连接池不与阻塞客户端共用，TLS 配置再建一份），每次对话再建一个运行时、多占描述符；增量回调还不能在运行时里调用，否则宿主在回调里调到任何用阻塞客户端的 `msime_client_*` 都会因为运行时里不能再起运行时而 panic。后台线程的代价是取消以后最多 125 秒的一个线程和一条连接，服务端在连接断开之前多生成一段。
- **流式对话逐段核对给出令牌的账号（Android 每段增量前都做）** — 最强的理由：用户中途退出登录时，已经换了账号的页面上不会再冒出旧账号的回答。否掉：每段增量都要读一次会话存储，注入模式下就是一次宿主回调；页面退出登录时自己会取消对话，回答完整时的核对仍会报 `account_cancelled`。
- **AI 皮肤的进度回调留在出图线程上** — 最强的理由：不用多一条通道，Tauri 现在就是这样 emit 事件的。否掉：C 宿主的进度回调一律在调用线程上，JNI 胶水要缓存 `JavaVM` 并 attach/detach，NAPI 没法从别的线程直接调 JS；三个线程并发回调还要宿主自己串行化。

- **绑定成功后保留原来的本地会话，丢掉服务端签发的新会话** — 最强的理由：本地会话不换，进行中的操作和别的进程都不受影响，代次也不用推进。否掉：新会话是刚建立的，宿主紧接着注销账号或删除云端数据时就不用再登录一次；丢掉的那个会在服务端留 30 天，「我的设备」里多出一条。Android 也换成新会话。
- **绑定另开一组操作（`link_request_code`、`link_login` 等）** — 最强的理由：新操作天然只有 `AccountFailure` 一种错误形状，不用在同一个操作上按 `purpose` 的取值分出两种码。否掉：请求和响应与登录完全相同，只差一个用途，与服务端和 Android 的接口形状一致；宿主的登录面板只多传一个字段，不用为绑定再写一遍分发。
- **Rust 自己读写钥匙串和 KeyStore** — 最强的理由：宿主一行回调都不用写，没有跨线程回调和 JNI attach 的麻烦，存储逻辑也只有一份。否掉：要引入 `security-framework`、`keyring` 一类第三方 crate，仓库负责人已定不加；钥匙串的 access group、App Group 的 entitlement 和 AndroidKeyStore 的密钥别名都是各宿主自己的配置，Android 上还得从 Rust 反向调 Java，复杂度只是换了个地方。
- **所有平台都改用目录模式（明文 JSON 文件）** — 最强的理由：`FileAccountSessionStorage` 已经存在、已有测试和跨进程锁，不需要任何新 ABI。否掉：iOS 和 Android 的令牌会从钥匙串、KeyStore 加密存储落到明文文件，平台现有的安全保证变弱。鸿蒙例外：它的会话本来就在 ArkTS 管的文件里，C 回调在 Rust 线程上触发、没法直接调 ArkTS，改用 threadsafe function 阻塞等结果又会在调用方是 JS 线程上的同步 napi 函数时死锁，所以鸿蒙走目录模式。
- **sans-IO：Rust 只给出「读这一项」「写这一项」的指令，由宿主执行** — 最强的理由：没有任意线程上的回调，JNI 和 NAPI 胶水最简单，`msime_client_account_gloss` 这类按键路径的接口就是这么做的。否掉：刷新要在一把锁里完成「重读、发请求、再读一次核对、保存」，sans-IO 会把这段时序和锁顺序推回每个平台，正是要消除的那几份重复实现。
- **一个进程级的全局默认存储（init 一次，之后所有请求都用它）** — 最强的理由：请求形状不变，平台迁移时少改字段。否掉：全局默认会悄悄改变现有带 `directory` 的调用；同一个进程（例如测试或 Windows 的设置程序）也可能同时需要两种存储。
- **锁回调不带 item，每个 store 一把锁** — 最强的理由：接口更小，宿主只管一个锁文件。否掉：macOS 上登录会话和匿名会话的锁本来就在两个容器里（Application Support 的 `account-refresh.lock` 与 App Group 的 `backend-account-refresh.lock`），Tauri 设置应用和 Swift 都已在用；只有一把锁就得让其中一方改锁文件，共存期间两边对不上。
- **只靠宿主的锁排斥本进程里的并发** — 最强的理由：Rust 里少一层 `Mutex`。否掉：Java 的 `FileChannel.lock` 同进程重复加锁会抛 `OverlappingFileLockException`，fcntl 同进程第二个 fd 直接成功；Rust 自己再套一把进程内锁，宿主就只需要保证跨进程。
- **删匿名语音样本时每次都先重新登录匿名账号** — 最强的理由：少一次注定被拒的请求（匿名会话几乎总是超过 10 分钟）。否掉：重新登录计入按地址每天 5 次的匿名登录限额，用户刚因为登录账号要求最近登录而重试时，上一次换来的匿名会话还新鲜，先试一次就不用再耗一次额度。
- **先删匿名账号的语音样本、再删登录账号的分区**（第一版的做法） — 最强的理由：匿名那一步重做没有副作用，它失败时登录账号的分区还没动，整个操作重试是干净的，不会把词库的 reset 再写一遍。否掉（仓库负责人，PR #8350）：用户最常遇到的失败是登录账号要求最近登录，这时匿名那一步已经做完，多半耗掉了一次按地址每天 5 次的匿名重新登录，用户重新登录后再删又耗一次；改成登录账号在前以后，匿名那一步的失败另放在 `voice_samples` 里、用 `voice_delete` 单独重试，重试同样不碰登录账号的分区。
- **匿名那一步失败时让整个 `data_delete` 或注销失败** — 最强的理由：只有一种结果形状，宿主不用看 `voice_samples`。否掉：这时登录账号的分区已经删了、账号已经注销了，报失败会让宿主提示用户重做一件已经做成的事；注销的话账号已经不在，重做只会得到 `account_unauthorized`。
- **平台行为以各平台现状为准** — 最强的理由：迁移不会改变任何用户可见行为。否掉：仓库负责人已定以 Rust 为准，否则 Rust 实现得为每个平台各留一套分支，等于把重复实现搬进 Rust。

## Acceptance criteria

- `cargo test -p msime-client-services` 覆盖：两种布局的读写、清除登录会话时连密钥串一起删、超长或解不开的文档是 `Storage`、置 SHARED 时在同一线程上成对加解锁（含 panic）、同一线程嵌套加锁被拒、匿名身份「已存在」时重读并采用先发布的那份、`shared_access_token_in` 带 `expected_user_id` 时不退回另一个账号、`client_agent` 的形状校验与发出的 User-Agent。
- `cargo test -p msime-host-api` 覆盖：`msime_client_session_store` 在 64 位目标上是 80 字节、各字段偏移与头文件一致；非法描述符不接管 `context`、不调用 `release`；`release` 等最后一个进行中的调用结束才调用且只调用一次；四个入口在注入模式下的 status、forget、取令牌、缓存与每次重读、补登录 ID 时持宿主锁、`directory`/`session_store` 恰好一个的校验。
- 新接口：client-services 的测试覆盖 `AccountFailure::code()` 的映射顺序与 `Retry-After`、释义的整包预检与逐项置空、常用语按服务端规则的校验、「我的设备」保留 `web` 和空串、反馈的截图尺寸只读头部、诊断上传按服务端规则重建（事件种类、崩溃时间、配置快照的层数和条目、2 MiB 先截最旧的事件），以及用 loopback 假服务器回放服务端真实的 202 `{id,state,expires_at}` 和 failed `{id,state,artwork:null,reason}`（修复前这条测试失败）；host-api 的测试用假后端覆盖三类令牌来源、401 只在同一账号内重试一次、撤销当前会话改走退出、绑定账号对不上时是 `account_identity_changed`，C 层覆盖不联网的校验与 `retry_after_seconds` 的形状。断网与 503 分开、`chat`/`chat_models`/AI 皮肤的 `retry_after_seconds`、`account_delete` 的 `account_recent_login_required`（且只在成功后清本地会话）由 client-services 的 loopback 假服务器测试（挂断连接、429、503、403）和 host-api 的假后端测试覆盖，同一组测试同时核对已有的 `chat`、`chat_models`、`delete`、`generate` 给出的码不变。流式对话：client-services 的解析器测试逐字节喂入（UTF-8 字符、CRLF 和 `data:` 前缀被拆开），loopback 假服务器分段写出 SSE，覆盖增量在调用线程上按序交出、请求带 `stream: true` 和 `Accept: text/event-stream`、流开始前的 429 和 503 `server_busy` 带 `retry_after_seconds`、上游失败事件和没等到 `[DONE]` 的断开（已交出的增量留着）、等响应头时和两段增量之间从别的线程取消立刻返回且服务端下次写数据时连接被断开、回调里取消不再交出下一段、旧后端 400 `invalid_chat_request` 退回非流式；host-api 的假后端测试覆盖令牌来源、401 重试不交出增量、回调里取消和回答期间换号是 `account_cancelled`，C 层覆盖请求校验、`account_busy` 与按 id 取消。
- 绑定登录方式：client-services 的 loopback 假服务器测试核对六步都带登录账号的令牌和 `purpose:"link"`、完成后本地会话换成服务端签发的新会话（文件存储和注入存储都在刷新锁里写入）、403 `recent_login_required`、409 `identity_already_linked`、401 不重试、令牌快过期时先刷新、申请时 401 刷新一次重试、完成请求进行中别的进程刷新照常写入而换号或重新登录时不写入；host-api 的 C 层测试核对没有登录会话时（只有匿名会话也一样）是 `account_unauthorized`，以及不合法的用途、provider 和凭据在联网之前就是 `account_invalid`。
- 带 `directory` 的现有测试全部照旧通过；`shared/backend` 的 `swift test` 和 `platforms/macos/scripts/test_backend_account_swift.sh` 通过。
- 第二阶段：iOS、macOS、Android 改调注入接口后，Swift 和 Java 里的会话刷新、匿名注册和账号接口实现删除；HarmonyOS 与 Linux 改用目录模式。

## Risks

- 宿主的回调实现不守锁顺序（例如 Android 用同一个 monitor 既实现 `lock` 又包住 `load`）会死锁；头文件写了约束，但只能在平台迁移时靠代码审查和真机验证发现。
- JNI 回调在 Rust 起的线程上触发（AI 皮肤的出图线程遇到 401 会刷新令牌），胶水忘了 attach/detach 或没清 Java 异常时进程会崩；同上，只能在 Android 迁移时验证。
- iOS 键盘扩展引入 reqwest 和 tokio 后常驻内存可能超标；要在真机上测出增量，超标时扩展里继续走 sans-IO。
- 共存期间若有一方不经锁刷新同一份会话，仍可能被吊销；刷新遇到 409 `refresh_superseded` 后重读重试的兜底暂不做，等平台迁移以后再看是否还需要。
- User-Agent 由宿主的 `client_agent` 决定：注入模式在注册时给，`msime_client_account` 和 `msime_client_account_chat_stream` 的目录模式在每次请求里给（决定 1），不给时仍发 `MSIME/Android`，所以不带它的宿主（现在的 Windows 设置窗口就不带）经目录模式登录的会话在「我的设备」里仍显示成 android，要宿主在迁移时自己加上。目录模式按（目录，`client_agent`）缓存会话对象，同一个宿主的调用带的 `client_agent` 不一致时 `google_cancel` 找不到另一个对象上的 `google_login`；同一目录的几个对象经文件和 `account-refresh.lock` 互斥，与几个进程共用一个目录相同。其余入口（社区、取令牌、云剪贴板、云词库、AI 皮肤、云同步）的目录模式不登录，仍发 `MSIME/Android`。
