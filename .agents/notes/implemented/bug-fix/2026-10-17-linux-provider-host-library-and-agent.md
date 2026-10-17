# Agent Note: Linux provider 按安装路径加载 host 库，匿名注册自报 User-Agent

Status: implemented

## Problem

`msime-linux-online-provider` 找 host 库时只猜 `<prefix>/lib/msime-client/` 和源码树里的 `target/debug`。rpm 和 Gentoo 把库装在 `lib64`，于是 provider 一个库也加载不到：「水杉账号」的候选释义和 `--ensure-anonymous-account` 在这些发行版上从来不可用。安装时 CMake 其实已经把库相对 `bin` 的路径写进了 `msime_voice_paths.py`（`HOST_LIBRARY`，语音 provider 用它），只是在线 provider 没有读。

匿名账号注册走旧入口 `msime_client_ensure_anonymous_account`，User-Agent 是 `MSIME/Android`，服务端「我的设备」把 Linux 机器显示成 Android。

Rust 刷新匿名令牌时在账号目录里建 `account-refresh.lock`，而首次配置判断「状态目录里只有安装包注册的匿名账号」的三处清单（`msime-linux-prepare` 的 `installer_account_state`、`msime-linux-setup` 的 `ANONYMOUS_ACCOUNT_FILES`、设置应用 `linux_setup.rs` 的同名清单）只认两份 JSON，多了锁文件就当作目录被占用。

候选翻译选了 AI 服务时，`translations` 在给 `options` 赋值之前就读它，每次都抛 `UnboundLocalError`，AI 候选翻译从没发出过请求。

## Decision

- `RustAccountApi._load` 第一个试 `msime_voice_paths.HOST_LIBRARY`（相对脚本所在的 `bin`，带本版本的库名），然后才是 `MSIME_HOST_LIBRARY` 和原来猜的几处；源码树里没有这个模块，跳过。必需符号加上 `msime_client_ensure_anonymous_account_with_agent` 和 `msime_client_client_agent`。
- `msime_voice_paths.py.in` 另写入 `VERSION`（`MSIME_LINUX_VERSION`）和 `EDITION`（`MSIME_EDITION`）。`ensure_anonymous`（`--ensure-anonymous-account` 和惰性注册都经它）改调 `_with_agent`，`client_agent` 由 `msime_client_client_agent` 拼出：`linux`、安装包版本、设备留空、系统取 os-release 的 `NAME` 加 `VERSION_ID`、版本 ID。一个进程只拼一次；拼不出时照旧注册、不带它。
- 三处清单都加上 `account-refresh.lock`。
- `translations` 在 `selected == "ai"` 那一支先取 `options`。

## Alternatives considered

- **按发行版再多猜几个目录（`lib64`、`lib/x86_64-linux-gnu`）** — 不依赖配置出来的模块；但 Gentoo、Arch 的 `msime-bin` 和以后的发行版各有各的布局，CMake 已经知道真实路径，再猜只会漏掉下一个。
- **在 provider 里自己拼 User-Agent** — 少一次 FFI；但那样又多一份 Android 已经出过三处差别的清洗规则（见迁移差异 M12），P11 正是为了只留一份。

## Consequences

- **收益**：deb、rpm、Gentoo 上账号释义和匿名注册都能用；新注册的匿名账号在「我的设备」里显示 linux 和版本号；AI 候选翻译能发出请求。
- **代价与已知上限**：已经注册过的匿名账号不重新注册，`client_agent` 只对新注册生效。账号释义改走 `msime_client_account` 的 `translate` 以后（见 [Linux provider 的「水杉账号」释义改调 msime_client_account 的 translate](../architecture/2026-10-18-linux-account-translation-through-rust.md)），释义请求和其中的令牌刷新也带同一个 `client_agent`。`linux_system` 只读 os-release，不报内核或桌面。

## Verification

`platforms/linux/tests/provider/host_library_contract.py`（安装路径排第一、注册带 client_agent 且只拼一次、拼不出时照旧注册、os-release 读法）、`platforms/linux/tests/dictionary/translation_provider_selection.py`（AI 选择走到 `ai_translation`）和 `platforms/linux/tests/core/setup_resolution.py`（有锁文件的目录仍算安装包留下的状态）。
