# Agent Note: macOS 原生账号页与云端窗口改调 Rust，「本机账号」卡片留在 Swift

Status: implemented

## Problem

macOS 输入法的原生账号页（`platforms/macos/src/backend/account/BackendAccountWindow.swift`）和它打开的云剪贴板、云词库、整份快照、设置同步窗口，以及给 Tauri 面板做 IPC 的两个 provider、候选释义，都用 `shared/backend` 的 Swift 实现：`BackendAccountSession` 自己读写 `account-session.json`、刷新令牌，`BackendAccountClient` 自己发 `/v1/auth`、云剪贴板、云词库和设置请求。#8201 要求账号与云端逻辑只有 Rust 一份（`crates/host-api/include/msime_client.h`）。第 4a 步已经把 Rust 函数表交给后端 dylib、实现了注入的会话存储（[Apple 宿主经一张函数表调用 Rust](2026-10-18-apple-rust-client-table.md)），这一篇记录 macOS 的页面怎样切过去、哪些故意没切。

macOS 与鸿蒙不同：Swift 的 `BackendAccountSession`、Tauri 设置应用的 `FileAccountSessionStorage` 和 Rust 的注入存储用的是同一组文件和同一把 `flock`（登录会话是 `account-refresh.lock`，匿名会话是 App Group 的 `backend-account-refresh.lock`），每次都在锁里重读，所以页面可以逐个切换，不会出现两个刷新方各拿同一枚刷新令牌刷新的情况。

## Decision

- 注册：后端 dylib 在 `MSIMEInstallRustClient` 收下函数表后立刻注册本进程的会话存储（`BackendRustAccount.registerShared`，布局 1、`SHARED`、`BackendRustStoreLayout.standard`）。输入法进程和独立设置窗口进程都经这里，各注册一次；Objective-C++ 不注册。client_agent 由 Rust 的 `msime_client_client_agent` 拼：平台 `macos`、输入法 bundle 的版本、`hw.model`、「macOS」加系统版本号、Info.plist 的 `MSIMEEdition`（没有时 full）；拼不出时整次注册失败，不带它注册会让登录以默认的 MSIME/Android 发出。
- `shared/backend/account/BackendRustAccount.swift` 是 Swift 侧唯一的入口：请求都带 `session_store`、不带目录；`status` 读不了存储时是 `account_storage`，不当作没有登录；`BackendRustClient.Failure` 的中文提示与共享账号页、Windows 原生设置同一套措辞。
- 账号页（4b）：登录账号的状态、登录方式、验证码登录、改名、退出、注销（`account_delete`，需要最近登录时提示先退出再重新登录、本机登录留着）都经 Rust。读不了登录状态时整页报原因（「无法安全读取登录状态」），不退回本机账号，与原来 Swift 读 `account-session.json` 失败时整页报错相同，只是措辞说清了原因。输入源第一次激活时的匿名账号注册改调 `ensure_anonymous`，已登录或读不了登录状态时不调。
- 原生的云剪贴板、云词库、整份快照窗口（4c）都经 Rust，只按打开窗口时的 user.id 绑定（C 上拿不到会话代次，同一个账号的令牌轮换和重新登录不再作废窗口，迁移差异 M29）：
  - 云剪贴板：`msime_client_account_cloud_clipboard` 的 `list`、`add`、`delete`、`clear`、`set_enabled`、`devices` 和设备管理，每次都带这台 Mac 的电脑名称作设备名（P6）；密钥串仍是 `account-session.json` 旁边的 `cloud-clipboard-keystore`，升级后不用重新批准。
  - 云词库：`msime_client_cloud_dictionary` 的 `list`、`add`、`update`、`delete`、`import`、`export_file`。导出由 Rust 边下载边写进中转目录、校验完换进用户选的位置。中转目录 `snapshot_directory` 在 `$TMPDIR` 下按进程号分开（`MacCloudTransferDirectory`）：Rust 在本进程第一次用到它时清掉上次留下的中转文件，输入法进程和独立设置窗口进程同时运行时不能共用。
  - 整份快照：备份、恢复按用户选的路径交给 `snapshot_export_file`、`snapshot_restore_preview_file`、`snapshot_restore_file`；「应用到本机」带本机的 HostOptions（`MSIMEClientSession.activeHostOptions`）调 `snapshot_preview`、`snapshot_enqueue`，排进 Rust 在状态目录下的快照队列，Rust 写词库维护租约、等输入会话放开词库后在本进程里激活。Rust 只写租约不发通知，而没有获得焦点的输入框时 `InputController.mm` 每秒检查租约的计时器不跑，所以请求进行期间宿主每 250 毫秒在本进程里发一次 `MSIMEDictionaryMaintenanceWillBeginNotification`。没能马上应用时请求留在队列里，窗口开着时每两秒问一次 `snapshot_status`，关掉窗口也不丢。
- 释义、给 Tauri 面板的 IPC 和设置同步（4e）：
  - 候选释义经 `msime_client_account` 的 `translate`（`source_lang` ZH），登录账号优先、没有登录时用本机匿名账号，都由 Rust 选；两个都没有时（`account_unauthorized`）补注册一次匿名账号再问，一个进程 600 秒内只补一次（迁移差异 M37 的做法）。回答按问之前登录的账号打标记，问完再读一次登录状态，换了号就丢掉。
  - 输入法在主线程上同步读的 `MSIMEBackendAccountTranslationMarker` 和 `MSIMEBackendAccountSignedIn` 不再读 `account-session.json`，改读 `BackendAccountState` 的缓存：注册会话存储时、输入源第一次激活时、每一页释义发出前后各用 Rust 的 `status` 刷新。标记只按 user.id 算（拿不到会话代次）。
  - 云剪贴板 IPC 只放行面板会发的九个动作（Rust 还认的 `pin`、`set_retention`、`clear` 在 macOS 的共享面板上没有入口，不放行），原样交给 `msime_client_account_cloud_clipboard`，错误码原样交给面板，参数不合法是 `invalid`。云词库 IPC 放行原来那十三个动作；`export` 改成 `export_file`，由 Rust 写进只有本人能读写的 `msime-export-<UUID>` 目录，Tauri 一侧照旧按 `read_export` 核对后读出；冲突是 `conflict`，其余是 `unavailable`，与原来相同。面板的整份快照与账号窗口同一条 Rust 路径，面板每两秒问状态时也叫醒本进程的输入会话。
  - 桌面设置同步：字段表（`preferences_schema`，camelCase）和云端文档（`preferences_load`）从 Rust 来，合并和 `platform.macos.*` 的映射留在 Swift（`BackendAccountClient.mergedPreferences`、`MacSettingsAccess`），上传用 `preferences_put` 整份替换、带读文档时的修订号和打开窗口时的 user_id（P4）。
- 「本机账号」卡片留在 Swift（负责人决定）：匿名账号的改名、退出、注销，以及验证码登录成功后丢掉匿名会话和身份，仍走 `BackendAnonymousAccount.session` 和 `BackendAccountClient`。Rust 没有对匿名会话的这几个操作；匿名会话与 Rust 共用 App Group 的锁，可以共存。

## Alternatives considered

- **账号页改调 Rust 的同时把「本机账号」卡片改成只显示。** 最强的理由是 macOS 上就只剩 Rust 一个账号实现，`BackendAnonymousAccount` 可以跟着删。没有采用：这是功能变化（用户再也不能给本机账号改名、注销它），负责人决定保留现状；共用的锁保证两边轮流刷新匿名会话不会互相吊销。
- **client_agent 拼不出时照样注册，不带它。** 理由是注册失败整个账号页就不能用。没有采用：不带 client_agent 的注入存储以 MSIME/Android 发出所有登录请求，服务端「我的设备」会把 Mac 显示成 Android；拼 client_agent 不联网、不读文件，只有请求本身不合法时才失败，那是代码错误，应该在测试里暴露。

- **「应用到本机」照旧在 Swift 里经 `MSIMEClientSession` 的 `prepareSnapshot:`、`applySnapshot:` 当场激活，只把下载换成 Rust。** 最强的理由是这条路已经验证过：直接销毁、重建本进程的输入会话，不依赖租约和通知。没有采用：它是 Rust 快照队列之外的第二份激活逻辑（版本核对、云端改动核对、暂存清理），窗口一关请求就没了；排进 Rust 的队列以后与 Windows、鸿蒙、Android 是同一套规则，代价是要叫醒本进程的会话，见上。
- **中转目录放在状态目录下的固定位置。** 理由是与 Windows 设置应用的 `dictionary-snapshots` 一样好找。没有采用：输入法进程和独立设置窗口进程可能同时运行，Rust 在每个进程第一次用到这个目录时都会清掉里面的中转文件，共用一个会删掉另一个进程正在写的下载。

- **释义标记照旧在主线程上读会话文件。** 理由是它总是最新的，设置应用里换号以后下一个按键就换缓存范围。没有采用：那是 Swift 对 `account-session.json` 格式的第二份理解，Rust 也不给会话代次；主线程上又不能等 Rust（读存储时可能要等另一个进程的刷新锁）。代价是设置应用里换号以后，到下一页释义发出之前，缓存范围还是原来的账号。
- **IPC 把面板的动作全部转给 Rust，不再放行名单。** 理由是 Rust 自己校验，宿主少一份名单。没有采用：Rust 的云剪贴板还认置顶、保留期和清空，`msime_client_cloud_dictionary` 还认带路径的 `export_file`、`snapshot_*_file`；面板是另一个进程，路径和这几个动作只该由宿主自己发。

## Consequences

- **收益**：macOS 的登录会话只由 Rust 刷新和改写（共享界面的三页见下），「我的设备」显示 macOS 和机型，注销需要最近登录时有明确提示，锁屏或权限问题读不了会话时不再被当作已退出。
- **代价**：账号页的测试改用替身函数表（`platforms/macos/tests/core/FakeRustTable.swift`）断言发出的 action，Rust 自己的行为由 host-api 和 client-services 的测试覆盖；匿名账号仍有一份 Swift 实现。
- **行为变化**：释义在只有匿名账号可用、而匿名会话失效时，10 分钟内只补注册一次，原来每一页都会注册；单条释义不合格时只那一条没有，不再整页作废；设置同步的冲突是「云端内容已被其他设备更新」。云剪贴板窗口在同一个账号重新登录后不再作废；导出和备份写到用户选的位置之前先整份校验，原有文件在失败时不动；「应用到本机」没能马上应用时留在队列里，不再当场失败。
- **删掉的**（4g 的 macOS 部分）：macOS 自己的端到端加密驱动接线（`MacCloudClipboardDriver.e2e`，只剩设备名）、导出和备份的 Swift 文件替换（`MacCloudFileTransfer`）、宿主自己准备和激活快照的 `MacPreparedLocalSnapshot`（`BackendLocalSnapshot.swift` 改名 `BackendLocalDictionary.swift`，只剩 HostOptions 和叫醒输入会话），以及两个 provider 的 Swift API 协议。`shared/backend` 里这些页面原来用的 Swift 客户端仍是 iOS 的，留到 iOS 那一组；`shared/apple/MSIMEClientSession` 的 `prepareSnapshot:` 等选择器 Swift 已不再调用，`snapshot-activation-test` 仍覆盖，没有删。
- **不再编进输入法**：`src/cloud/` 里没人引用的 Objective-C 云剪贴板、云词库客户端和窗口（`CloudClipboardClient`、`CloudClipboardWindowController`、`CloudDictionaryClient`、`CloudURLSession`、`CloudFileTransfer`）和 `src/core/Account*`（含对 `/v1/auth/refresh` 的 `MSIMEAuthRefresh`）已从 `MSIMEClientInputMethod` 的源文件里去掉，输入法二进制里不再有这些符号。其中 `CloudFileTransfer` 和 `src/core/Account{AuthClient,Keychain,SessionManager,WindowController}` 除了彼此和两个守护脚本里登记它们的条目（`test-macos-orphan-sources.py` 的 `RETAINED`、`test-macos-cloud-redirect-policy.py` 的文件名单）没有任何引用，已经删掉，脚本里的条目一起去掉。其余四个仍由三个测试目标（`cloud-clipboard-deletion`、`cloud-clipboard-selection`、`cloud-dictionary-client`）编译，`test-macos-cloud-query-encoding.py`、`test-macos-cloud-redirect-policy.py` 也读它们的源码，文件留着，删不删由负责人定。
- **还没切的**：`shared/backend-ui` 里 macOS 与 iOS 共用的三页（社区词包与回复模板、完整词库目录、云端候选与排序）仍用 Swift 的 `BackendAccountSession.shared`，与 Rust 共用 `account-refresh.lock`，随 iOS 那一组一起换成 Rust 的 `community` 和 `cloud_dictionary`。

## Verification

- macOS ctest `backend-account-swift`：替身 Rust 下账号页的状态、登录方式（没开放的不发请求）、验证码登录（带 `challenge_id` 和 `code`）、改名、需要最近登录的注销、退出所有设备、断网时退出、读不了登录状态时不退回本机账号、宿主没注册存储时全部失败；「本机账号」卡片的改名和注销不经 Rust，验证码登录成功后丢掉匿名会话。把读状态改回 `try?`、去掉登录后丢掉匿名会话时都失败。
- 同一个 ctest 的 4c 部分：云剪贴板窗口的每个操作（开着时才多读设备状态、批准带回设备状态时不再读、等待批准时留着草稿、`clear`、设备名、换了账号不发请求），云词库窗口的增删改、冲突、导入和 `export_file` 的路径，整份快照的预览、入列（期间发叫醒通知）、排队后靠状态轮询等到应用、备份与恢复的路径和摘要、没有输入会话时不发请求。把 `clear` 改回 `delete`、去掉设备名、去掉状态轮询、去掉叫醒通知时都失败。
- 同一个 ctest 的 4e 部分：释义的请求、标记、问完换号丢掉、补注册一次匿名账号与 600 秒的间隔、同步读的标记；设置同步的字段表、合并、带修订号和 user_id 的上传、冲突、换号不发请求。ctest `desktop-cloud-provider`、`desktop-dictionary-provider`：两个 IPC provider 的转发、名单、错误码、账号绑定（请求在路上换号时回答不交给面板）、取消、`export_file` 的目录和权限、整份快照的面板、令牌只能用一次。
- `swift test --package-path shared/backend`：`BackendRustAccountTests`（client_agent 由 Rust 拼、拼不出时不注册、一个进程只注册一次、请求带 `session_store` 不带目录、读不了存储是失败、中文提示）。
