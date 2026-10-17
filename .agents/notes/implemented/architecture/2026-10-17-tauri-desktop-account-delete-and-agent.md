# Agent Note: Tauri 桌面账号自报 User-Agent、注销改用 delete_account_detailed、快照文件工具改用 host-api

Status: implemented

## Problem

Tauri 桌面（macOS、Windows、Linux 的设置应用）的账号会话用 `BackendAccountClient::new()`，登录和刷新都以 `MSIME/Android` 发出，服务端「我的设备」把桌面显示成 Android。注销用 `delete_account`：会话不是 10 分钟内登录时服务端回 403 `recent_login_required`，页面只拿到 `account_unauthorized`，说「登录已失效」。原生 C 接口的 `account_delete` 注销以后还会删本机匿名账号名下的语音样本（#8201 的决定 12 后续 2），Tauri 不删。

`account_helpers.rs` 另有一套快照临时文件工具（建目录、读、写、删、清理预览），与 host-api 的 `cloud_dictionary::snapshot_files` 同名同用途，只给 Tauri iOS 和 Android 用（#8201 的决定 3 要求只留一份）。

## Decision

- `desktop_account::manage` 用 `BackendAccountClient::with_user_agent`，字符串由 client-services 的 `client_agent` 拼出：平台（macos、windows、linux）、`package_info().version`、设备型号与系统版本取自「关于」页系统信息用的那几项（macOS 的 `SystemVersion.plist` 写成 `macOS <版本>`，Linux 的 os-release 和 DMI；Windows 这里不读，写成 unknown）、`package_edition().id`。拼不出时退回 `new()`。社区服务共用同一个客户端。
- `account_delete` 改调 `delete_account_detailed`（失败码见 `AccountFailure::code`，需要最近登录时是 `account_recent_login_required`，本机登录留着），返回 `{voice_samples}`，形状与 C 接口相同。Windows、Linux 上注销成功以后用 `LockedAnonymousSessionStorage::new(同一目录)` 加 `AnonymousIdentitySource::Directory` 调 `delete_anonymous_voice_samples`，失败不让注销失败，结果放进 `voice_samples`。必须用 Locked 版本：Server 和 Linux provider 拿同一枚匿名刷新令牌，不在 `account-refresh.lock` 里刷新会让服务端吊销整个匿名会话。macOS 上 `voice_samples` 是 null：Tauri 不持有 App Group 里的匿名会话，macOS 也从不做语音贡献。
- 共享页：`AccountClient.deleteAccount` 可以返回 `AccountDeletion`，`voice_samples.ok === false` 时注销提示换成「账号已注销，但这台设备匿名账号名下的语音样本这次没能删除。」；`accountMessage` 补上 `account_recent_login_required`，与 Windows 原生设置窗口的文案相同。
- 快照文件工具：`account_helpers.rs` 的五个函数删掉，Tauri iOS、Android 改用 host-api 的同名函数（经 `account_helpers` 再导出），只剩 `snapshot_text_within_limit`。随之没有调用方的 `atomic_file::directory_matches` 删掉，`remove_private` 只剩 Linux 的 provider 凭据在用。
- 举报（`community_report`）仍只认登录账号，不退回匿名账号（头文件与决定 13）。

## Alternatives considered

- **Tauri 桌面也经 host-api 的 `msime_client_account` 目录模式** — 注销和语音样本的删除与原生宿主完全同一段代码；但 host-api 的目录模式写死 Native 布局，macOS 的 Tauri 会话是 Swift 的 Apple 布局，经它会把会话写成另一种格式，所以直接调 client-services 里已经公开的那几个函数。
- **注销后不删语音样本** — 少一次可能的匿名重新登录；但原生宿主都删，Tauri 不删的话同一台 Windows 机器从两个设置界面注销结果不同。

## Consequences

- **收益**：新的桌面登录在「我的设备」里显示真实平台和版本；登录超过 10 分钟后注销有明确提示；Windows、Linux 上注销以后匿名账号名下的语音样本也删掉，与原生宿主一致；快照文件工具只剩 host-api 一份。
- **代价与已知上限**：删语音样本可能多一次匿名重新登录，计入服务端按出口地址每天 5 次的额度。Windows 上 Tauri 拼出的设备型号是 unknown，与原生设置窗口（读注册表）不同。Tauri 移动端的快照文件写入改用 host-api 的 `write_snapshot_file`，多了一道「上级目录不是符号链接」的检查。

## Verification

`cargo test --locked -p msime-desktop`（`desktop_account` 的两条：没有匿名身份时不联网直接成功、失败时保留码和 `retry_after_seconds`；`account_helpers` 的符号链接用例改测 host-api 的函数），`cargo clippy --locked -p msime-desktop --all-targets -- -D warnings`；`apps/desktop` 的 `vitest run tests/account/use-account-session-actions.test.tsx tests/account/account-errors.test.ts`。
