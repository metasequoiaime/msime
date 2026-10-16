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
- 目录模式按账号目录缓存的会话、社区服务和 AI 皮肤服务，以及其他 `BackendAccountClient::new()`（正式服务端、默认 User-Agent）的调用方，共用进程里同一个 reqwest 阻塞客户端：每个阻塞客户端各带一个后台线程和事件循环（macOS 上三个文件描述符），缓存又从不释放，各建一份时目录一多就占满默认 256 的打开文件上限。客户端里只有连接池、不跟随重定向和 30 秒超时，没有 cookie 存储和默认请求头，令牌每个请求单独带，所以共用不会串号；带 `client_agent` 的注入存储和测试连本机回环的客户端各建各的。

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

云词库分发器与 Windows 完整备份（`msime_client_cloud_dictionary`，完整备份依赖 engine，所以放在 host-api 而不是 issue 写的 client-services）、iOS 设置映射、`msime_client_account` 的新操作（translate、phrases、sessions、user data、feedback、诊断上传、chat）、社区键盘皮肤、AI 皮肤生成与任务轮询、多分类同步编排。它们都经同一套 `session_store` 字段选会话。

### 新接口的错误形状与校验

- 新增的账号操作（`chat`、`translate`、`phrases_*`、`sessions_list`、`session_revoke`、`data_*`、`feedback`、`diagnostics_*`）返回 client-services 的 `AccountFailure`：在 `AccountError` 之外带着 HTTP 状态码、服务端错误码和 `Retry-After` 的整数秒。`AccountError` 一个变体都不加，已有操作的错误码不变：它在 Tauri 的社区命令等处被穷举匹配，把 403 `recent_login_required` 从 `Forbidden` 里分出来会改掉现有 `delete` 返回的 `account_unauthorized`。新码 `account_recent_login_required`、`account_too_large`、`account_not_found`、`account_identity_changed` 和 C 响应里的 `retry_after_seconds` 只出现在新操作上。
- 请求按 msime-cloud `origin/main` 的规则在本地完整校验，并对序列化后的请求体做整包预检：走 64 KiB `decode` 的接口超限时服务端回的是 400 `invalid_json`，分不出是哪条规则，所以 `translate`、`chat`、`msime_client_account_gloss` 在本地先报 `account_too_large`。诊断上传对服务端的每条拒绝规则做截断或替换而不只是检查，因为服务端只要有一处不合规就拒绝整份。
- 令牌来源：只认登录账号的、登录优先退回匿名账号的、绑定到上传时 `user_id` 的三类，后者对不上时报错，never 退回另一个账号（否则匿名上传后再登录，删除会删到登录账号那份、匿名那份连同 MCP 令牌还活 7 天）。
- 服务端响应的结构体不拒绝未知字段。AI 皮肤的 `RawSkinJob` 曾带 `deny_unknown_fields`，msime-cloud 在 2026-10-01 给 202 响应加了 `expires_at` 以后每次创建插画任务都解析失败；回放当前形状的本地测试抓不到服务端以后再加的字段。
- AI 皮肤生成的进度回调在调用线程上依次调用：出图线程只经通道报告完成。与头文件里其他进度回调的约定一致，JNI 和 NAPI 胶水可以直接用调用线程的 env；出图线程遇到 401 时仍可能在 Rust 的线程上调用存储回调，按上面的「任意线程」处理。

## Related notes

- [原生设置窗口账号页用到的 C 接口](../../implemented/architecture/2026-10-11-windows-settings-account-host-api.md)：部分重叠。那篇定下了 `msime_client_account` 每个 `directory` 一个会话对象、Google 登录与取消用同一对象；这里沿用它，并给同一个导出加上 `session_store`，目录模式的行为不变。
- [鸿蒙设置应用与键盘扩展经共享沙箱镜像交换数据](../../implemented/architecture/2026-10-09-harmony-ime-shared-sandbox.md)：部分重叠。鸿蒙的会话在 ArkTS 里、两个进程沙箱分开，是它不走注入、改为整端切到目录模式的背景之一。
- [iOS 原生应用的自动云同步](../../implemented/feature/2026-10-12-ios-automatic-cloud-sync.md)：与本篇无关；同步编排收进 Rust 的决定见下一条。
- [多分类自动云同步的编排收进 Rust](2026-10-16-cloud-sync-orchestration-in-rust.md)：部分重叠。同步的 C 入口沿用这里的会话来源和 `session_store` 字段。
- [旧账号会话迁移不覆盖新登录](../../implemented/bug-fix/2026-10-13-account-session-migration-no-clobber.md)：无关；匿名身份的「不存在才写」沿用的是同一种 no-clobber 思路，但那篇只管目录间的会话迁移。

## Alternatives considered

- **给 `AccountError` 加 `RecentLoginRequired`、`TooLarge` 等变体** — 最强的理由：只有一个错误类型，新旧接口共用一套映射，宿主不用分辨两种形状。否掉：现有调用方对它穷举匹配，新变体要么改掉已有操作返回的码（`delete` 遇到需要最近登录时从 `account_unauthorized` 变成别的），要么在每个匹配点各写一份兼容分支；`Retry-After` 的秒数也放不进一个无数据的枚举。
- **新接口的请求不在本地完整校验，交给服务端拒绝** — 最强的理由：规则只在服务端写一份，服务端放宽时客户端不用跟着改。否掉：64 KiB 整包超限时服务端只回 `invalid_json`，诊断上传任何一处不合规都整份被拒，用户看到的只是「不可用」；本地截断或预检能给出可操作的码。服务端放宽的那天，本地规则跟着改是一次小改动。
- **AI 皮肤的进度回调留在出图线程上** — 最强的理由：不用多一条通道，Tauri 现在就是这样 emit 事件的。否掉：C 宿主的进度回调一律在调用线程上，JNI 胶水要缓存 `JavaVM` 并 attach/detach，NAPI 没法从别的线程直接调 JS；三个线程并发回调还要宿主自己串行化。

- **Rust 自己读写钥匙串和 KeyStore** — 最强的理由：宿主一行回调都不用写，没有跨线程回调和 JNI attach 的麻烦，存储逻辑也只有一份。否掉：要引入 `security-framework`、`keyring` 一类第三方 crate，仓库负责人已定不加；钥匙串的 access group、App Group 的 entitlement 和 AndroidKeyStore 的密钥别名都是各宿主自己的配置，Android 上还得从 Rust 反向调 Java，复杂度只是换了个地方。
- **所有平台都改用目录模式（明文 JSON 文件）** — 最强的理由：`FileAccountSessionStorage` 已经存在、已有测试和跨进程锁，不需要任何新 ABI。否掉：iOS 和 Android 的令牌会从钥匙串、KeyStore 加密存储落到明文文件，平台现有的安全保证变弱。鸿蒙例外：它的会话本来就在 ArkTS 管的文件里，C 回调在 Rust 线程上触发、没法直接调 ArkTS，改用 threadsafe function 阻塞等结果又会在调用方是 JS 线程上的同步 napi 函数时死锁，所以鸿蒙走目录模式。
- **sans-IO：Rust 只给出「读这一项」「写这一项」的指令，由宿主执行** — 最强的理由：没有任意线程上的回调，JNI 和 NAPI 胶水最简单，`msime_client_account_gloss` 这类按键路径的接口就是这么做的。否掉：刷新要在一把锁里完成「重读、发请求、再读一次核对、保存」，sans-IO 会把这段时序和锁顺序推回每个平台，正是要消除的那几份重复实现。
- **一个进程级的全局默认存储（init 一次，之后所有请求都用它）** — 最强的理由：请求形状不变，平台迁移时少改字段。否掉：全局默认会悄悄改变现有带 `directory` 的调用；同一个进程（例如测试或 Windows 的设置程序）也可能同时需要两种存储。
- **锁回调不带 item，每个 store 一把锁** — 最强的理由：接口更小，宿主只管一个锁文件。否掉：macOS 上登录会话和匿名会话的锁本来就在两个容器里（Application Support 的 `account-refresh.lock` 与 App Group 的 `backend-account-refresh.lock`），Tauri 设置应用和 Swift 都已在用；只有一把锁就得让其中一方改锁文件，共存期间两边对不上。
- **只靠宿主的锁排斥本进程里的并发** — 最强的理由：Rust 里少一层 `Mutex`。否掉：Java 的 `FileChannel.lock` 同进程重复加锁会抛 `OverlappingFileLockException`，fcntl 同进程第二个 fd 直接成功；Rust 自己再套一把进程内锁，宿主就只需要保证跨进程。
- **平台行为以各平台现状为准** — 最强的理由：迁移不会改变任何用户可见行为。否掉：仓库负责人已定以 Rust 为准，否则 Rust 实现得为每个平台各留一套分支，等于把重复实现搬进 Rust。

## Acceptance criteria

- `cargo test -p msime-client-services` 覆盖：两种布局的读写、清除登录会话时连密钥串一起删、超长或解不开的文档是 `Storage`、置 SHARED 时在同一线程上成对加解锁（含 panic）、同一线程嵌套加锁被拒、匿名身份「已存在」时重读并采用先发布的那份、`shared_access_token_in` 带 `expected_user_id` 时不退回另一个账号、`client_agent` 的形状校验与发出的 User-Agent。
- `cargo test -p msime-host-api` 覆盖：`msime_client_session_store` 在 64 位目标上是 80 字节、各字段偏移与头文件一致；非法描述符不接管 `context`、不调用 `release`；`release` 等最后一个进行中的调用结束才调用且只调用一次；四个入口在注入模式下的 status、forget、取令牌、缓存与每次重读、补登录 ID 时持宿主锁、`directory`/`session_store` 恰好一个的校验。
- 新接口：client-services 的测试覆盖 `AccountFailure::code()` 的映射顺序与 `Retry-After`、释义的整包预检与逐项置空、常用语按服务端规则的校验、「我的设备」保留 `web` 和空串、反馈的截图尺寸只读头部、诊断上传按服务端规则重建（事件种类、崩溃时间、配置快照的层数和条目、2 MiB 先截最旧的事件），以及用 loopback 假服务器回放服务端真实的 202 `{id,state,expires_at}` 和 failed `{id,state,artwork:null,reason}`（修复前这条测试失败）；host-api 的测试用假后端覆盖三类令牌来源、401 只在同一账号内重试一次、撤销当前会话改走退出、绑定账号对不上时是 `account_identity_changed`，C 层覆盖不联网的校验与 `retry_after_seconds` 的形状。
- 带 `directory` 的现有测试全部照旧通过；`shared/backend` 的 `swift test` 和 `platforms/macos/scripts/test_backend_account_swift.sh` 通过。
- 第二阶段：iOS、macOS、Android 改调注入接口后，Swift 和 Java 里的会话刷新、匿名注册和账号接口实现删除；HarmonyOS 与 Linux 改用目录模式。

## Risks

- 宿主的回调实现不守锁顺序（例如 Android 用同一个 monitor 既实现 `lock` 又包住 `load`）会死锁；头文件写了约束，但只能在平台迁移时靠代码审查和真机验证发现。
- JNI 回调在 Rust 起的线程上触发（AI 皮肤的出图线程遇到 401 会刷新令牌），胶水忘了 attach/detach 或没清 Java 异常时进程会崩；同上，只能在 Android 迁移时验证。
- iOS 键盘扩展引入 reqwest 和 tokio 后常驻内存可能超标；要在真机上测出增量，超标时扩展里继续走 sans-IO。
- 共存期间若有一方不经锁刷新同一份会话，仍可能被吊销；刷新遇到 409 `refresh_superseded` 后重读重试的兜底暂不做，等平台迁移以后再看是否还需要。
- 注入模式的 User-Agent 由宿主的 `client_agent` 决定；目录模式的请求里没有传它的字段，仍发 `MSIME/Android`，所以经目录模式登录的 Windows、Linux 会话在「我的设备」里显示成 android。要改得给目录模式另加一个字段，这会改变现有请求的 UA，暂不做。
