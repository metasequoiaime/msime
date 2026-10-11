# Agent Note: 原生设置窗口账号页用到的 C 接口

Status: implemented

## Problem

原生设置窗口接下来要把「账号与同步」做成原生页，不再打开共享应用。共享账号页（`packages/ui/src/account/account-page.tsx`）在 Windows 上经 Tauri 的十四个 `account_*` 命令（`desktop_account.rs`）工作：状态、登录方式、发验证码、验证码登录、Google 浏览器登录与取消、资料、改昵称、取头像、选头像上传、移除头像、退出（含所有设备）、注销、清掉失效会话；登录后在 Windows 上还有「设置同步」卡片，经 `windows_settings_sync.rs` 的四个 `account_preferences_*` 命令读字段表、下载云端设置、上传本机设置、应用云端设置。窗口只能经 `msime_client.h` 的 C 接口调 Rust，而 host-api 里只有给输入法取令牌的 `msime_client_account_access_token`、云剪贴板和给鸿蒙自己读写回环 socket 的 `msime_client_google_loopback`，没有这些操作。

会话文件必须和共享应用、输入法是同一份：`windows_account.rs` 把它放在 `%LOCALAPPDATA%\<本版本的用户目录>\account\account-session.json`，启动时把旧版本放在 `%LOCALAPPDATA%\<tauri identifier>`（`app.msime.windows`）里的会话搬过来。页面读到的脱敏形状（`StatusResponse`、`ProvidersResponse`、`ChallengeResponse`、`ProfileResponse`，包括头像地址过滤和 `sms` 归为 phone）和搬旧会话的逻辑都只写在 Tauri 里。

## Decision

- 脱敏响应搬进 client-core 的新模块 `account::page`，测试一起搬过去；Tauri 的 `shared/account_dto.rs` 改为转出这几个类型，只留 iOS、Android 才用的聊天和偏好结构。
- 搬旧会话搬进 client-core 的 `account::adopt_legacy_account_session`，建目录用 `FileAccountSessionStorage` 自己的检查；`windows_account.rs` 改调它，三个测试一起搬过去。
- 新导出 `msime_client_account`：请求 `{directory, legacy_directory?, action:{operation, ...}}`，十四个操作与 Tauri 命令一一对应（名字去掉 `account_` 前缀，`choose_avatar` 变成 `upload_avatar {path}`，文件对话框由宿主自己弹），返回与 Tauri 相同的 JSON，错误串是 `AccountError::code()`。
- 每个 `directory` 在进程里只建一个 `BackendAccountSession`（静态表），这样另一个工作线程上的 `google_cancel` 能结束进行中的 `google_login`；会话存储是跨进程共享的，每次访问都重读文件，刷新时共用 `account-refresh.lock`，所以缓存会话对象不会读到旧状态。第一次建对象时先按 `legacy_directory` 搬旧会话，与共享应用启动时做的一样。
- 设置同步是同一个导出的四个操作 `preferences_schema`、`preferences_load`、`preferences_upload {preferences_directory}`、`preferences_apply {preferences_directory,user_id,preferences}`。上传和应用的逻辑（按版本过滤、只留字段表的键、合并、按会话代次上传；按下载时的账号和代次应用）从 Tauri 的命令里搬进 client-core 的 `settings_sync::upload_desktop_settings` 和 `prepare_desktop_settings_apply`，Tauri 改调它们；字段表的响应形状 `PreferenceSchemaResponse` 也搬进 `account::page`。偏好目录由宿主传，版本取这个目录记录的版本（`PreferencesStore::edition`）；应用后按读到时的修订号写回，关掉剪贴板历史时与 `msime_client_save_preferences` 一样清掉历史，返回保存后的快照。写回失败的码与 Tauri 相同（`conflict`、`storage`）。
- Google 登录整个在 Rust 里：用 `sign_in_google_with_browser`（与 Tauri 同一个实现：回环监听、校验授权链接、等回跳、提交授权码），浏览器经 `msime_host_windows::open_url`（Tauri 在 Windows 上用的也是它）打开。非 Windows 平台这一项返回 `account_unavailable`。

## Alternatives considered

- **每次调用新建会话对象**（云剪贴板导出的做法）— 更简单；但 Google 登录要等几分钟，取消只能作用在同一个会话对象上，新建的对象取消不了它。
- **让原生窗口用 `msime_client_google_loopback` 自己监听回环端口** — 那是给自己有 HTTPS 栈的鸿蒙用的，原生窗口还得自己申请 challenge、提交授权码、写会话文件，令牌就要经过 C++；在 Rust 里跑完整流程，令牌不出 Rust。
- **传回调函数让宿主打开浏览器** — 不需要：`open_url` 已经是 Windows 上打开链接的共享实现，C 接口保持纯 JSON。
- **在 host-api 里另写一份脱敏和搬旧会话** — 改动小，但两边会分叉；做法与之前把 `settings_dictionary`、AI 设置收进共享 crate 一致。

## Consequences

- 原生账号页可以完成共享页在 Windows 上能做的全部账号与设置同步操作，用的是共享应用和输入法读的同一份会话，登录、退出在三者之间立即生效。
- 原生窗口要传 `%LOCALAPPDATA%\MSIME_EDITION_TAURI_IDENTIFIER` 作 `legacy_directory`，只用过原生窗口的用户也能接上旧会话。
- 以后改脱敏规则或搬旧会话只改 client-core 一处。

## Verification

- `cargo test --locked -p msime-client-core --lib account` 通过（111 项，含搬过去的 `account::page` 与 `file_storage` 用例，以及用假账号服务跑的设置同步上传和应用）。
- `cargo test --locked -p msime-host-api --lib ffi::account` 通过（5 项，不联网）：设置同步的参数和账号检查在联网前完成、读共享会话文件且响应里没有令牌、`forget` 删掉文件、先搬旧目录里的会话、不合法请求在联网前被拒绝、未登录时需要令牌的操作返回 `account_unauthorized`、`google_cancel` 无进行中的登录时无害。
- `cargo build --release --locked -p msime-host-api`、`cargo check --locked -p msime-desktop` 通过，警告都是改动前就有的；`cargo check --tests -p msime-desktop` 在 Windows 上因 `account_helpers.rs`、`macos_input_source.rs` 里的 unix 专用测试代码编不过，改动前就是这样。
- 设置工程按 `/WX` 编译通过（`settings-test-build.ps1`），没有启动窗口。没有对真实账号服务发过请求，Google 登录也没有真正跑过浏览器：联网路径都是 Tauri 已在用的同一实现。
