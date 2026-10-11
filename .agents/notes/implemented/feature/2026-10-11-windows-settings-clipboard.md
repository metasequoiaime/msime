# Agent Note: Windows 原生设置窗口自己画「剪贴板」页

Status: implemented

## Problem

原生设置窗口的「剪贴板」是 Shell 页，只有一行「打开」，用 `settings:tools` 拉起共享 Tauri 应用。整项工作的目标是任何设置都在原生窗口里改。共享页（`tools-page.tsx` 和 `clipboard-history-section.tsx`）在 Windows 上有三组：「剪贴板」（`clipboard_history` 开关、从系统剪贴板同步、点两次才清空的「清空历史」）、开着历史时的「历史记录」（每条重新复制、发到云剪贴板、固定或取消固定、删除），以及打开云剪贴板面板的「云剪贴板」。C ABI 原来只有只给纯文本的 `msime_client_load_clipboard_history`、要求开关开着的删除和采集，没有清空、固定、同步，也没有设置窗口能用的云剪贴板请求。

## Decision

- 「剪贴板」页改成 Native，照共享页在 Windows 上的样子画三组，文案、禁用条件、两步清空、按开关作废进行中的读写、每次进入页面重读历史和重查云剪贴板，都照共享组件。失败时与共享页的 `onError` 一样在页面顶部说明；读取失败和「重新复制」失败与共享页一样不提示。云剪贴板仍是面板，`shell_links` 加 `cloud_clipboard`，导航测试改为核对它。
- host-api 新增 `clipboard_history` 模块和 `msime_client_desktop_clipboard_history`（`list`、`sync`、`clear`、`set_pinned`、`remove`，都返回操作后的结构化历史，开关关着时为空）。读写的是 Tauri 外壳用的同一个 `<偏好目录>/clipboard_history.json`，不是移动端的 `MSIME/clipboard_history.json`。「从系统剪贴板同步」的规则（关着时不碰剪贴板、`None` 不算错误、在偏好锁下采集）写成 `clipboard_history::capture_clipboard`，Windows 的读取写成 `clipboard_history::windows_clipboard_text`（`msime_host_windows::read_clipboard_text` 加去掉末尾换行），Tauri 的 `sync_clipboard_history_blocking` 改为调用这两个函数。为此 host-api 在 Windows 上依赖 `msime-host-windows`。
- 「重新复制」用 WinRT 剪贴板写入，再像 Tauri 的 `copy_text` 一样经现有的 `msime_client_capture_clipboard_history` 记进历史。
- 「发到云剪贴板」经新的 `msime_client_account_cloud_clipboard`：用共享应用登录时写在 `%LOCALAPPDATA%\<本版本的用户目录>\account` 的 `account-session.json`（与 Tauri 的 `windows_account.rs` 同一个目录、同一种 Native 布局），不退回匿名账号，与 Tauri 一样。`list`、`add`、`delete`、`set_enabled` 的分派从 Tauri 的 `platform/cloud_clipboard.rs` 移到 host-api 的 `cloud_clipboard::account_request`，Tauri 只在 `spawn_blocking` 里调用它，原来的测试照旧经它跑。
- 所有读写和网络请求都经 `run_host` 在工作线程上做，完成后回 UI 线程排队重画。

## Alternatives considered

- **在 C++ 里自己读剪贴板，按 `ClipboardPrivacyPolicy.h` 判隐私标记** — 不给 host-api 加依赖，Server 的采集就是这么做的；但 Tauri 的同步走 Rust 的读取，两边各写一份打开剪贴板、读 `CF_UNICODETEXT` 和去末尾换行，以后改一边容易漏另一边。选了让原生窗口和 Tauri 走同一个函数。
- **「发到云剪贴板」先只打开云剪贴板面板** — 不用新增账号相关的导出；但面板拿不到要发的那一条，按钮名不副实。客户端核心的会话本来就支持多进程共用 `account-refresh.lock` 刷新（`msime_client_account_access_token` 已这样用），所以直接照 Tauri 的请求做。
- **沿用 `msime_client_load_clipboard_history` 再补几个单独的导出** — 改动小；但它只给纯文本，没有时间和固定状态，删除还要求开关开着，与 Tauri 的命令不一致。新导出一次返回结构化历史，旧导出留给已在用它的宿主。

## Consequences

- 原生窗口不再为「剪贴板」页拉起共享应用；只剩云剪贴板面板由 `MSIME.exe` 打开。
- `msime_host_api.dll` 在 Windows 上多链接 `msime-host-windows`（含 Windows Ink 的 WinRT 投影），体积和编译时间略增。
- 原生窗口另起一个账号会话对象读写同一个会话文件。刷新令牌时两边靠 `account-refresh.lock` 和刷新前重读文件避免用掉对方的刷新令牌，这条路径与输入法进程取令牌相同。
- 关掉历史时偏好保存已清空历史文件，原生窗口仍照共享页再调一次清空；两次清空无害。
- 时间按用户区域设置的短日期加时间显示，对应 `toLocaleString()`，格式可能与 WebView2 略有出入。
- 相关：Windows 剪贴板隐私标记的名单见 [剪贴板隐私与面板统计](2026-10-10-windows-clipboard-privacy-panel-statistics-host-diagnostics.md)。

## Verification

- `cargo test --locked -p msime-host-api --lib clipboard` 通过：新导出的列出、固定、删除、清空、关闭后为空和同步被拒；`capture_clipboard` 关着时不调用读取函数；`account_request` 用合成后端核对四种操作、未登录和非法请求不到后端；云剪贴板导出在没有会话文件时不联网就返回 `account_unauthorized`。`cargo build --release --locked -p msime-host-api` 和 `cargo check --locked -p msime-desktop` 通过。
- Tauri 的 `platform/cloud_clipboard.rs` 测试没有跑：`msime-desktop` 的 lib 测试在 Windows 上编译不过（`macos_input_source.rs` 等测试代码用了 `std::os::unix`，与这次改动无关），所以分派逻辑的测试另在 host-api 写了一份。
- 设置工程按 `/WX` 编译通过（`settings-test-build.ps1` 输出 `SETTINGS_BUILD_OK`），`tests/ui/settings_navigation.cpp` 用 `cl /std:c++20 /W4 /WX` 单独编译并运行通过。
- 没有启动窗口，也没有读写真实的系统剪贴板或联网：页面的实际交互、同步读到的内容、重新复制和发到云剪贴板都未在运行中的窗口里检查。
