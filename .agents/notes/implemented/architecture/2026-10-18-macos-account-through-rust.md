# Agent Note: macOS 原生账号页与云端窗口改调 Rust，「本机账号」卡片留在 Swift

Status: implemented

## Problem

macOS 输入法的原生账号页（`platforms/macos/src/backend/account/BackendAccountWindow.swift`）和它打开的云剪贴板、云词库、整份快照、设置同步窗口，以及给 Tauri 面板做 IPC 的两个 provider、候选释义，都用 `shared/backend` 的 Swift 实现：`BackendAccountSession` 自己读写 `account-session.json`、刷新令牌，`BackendAccountClient` 自己发 `/v1/auth`、云剪贴板、云词库和设置请求。#8201 要求账号与云端逻辑只有 Rust 一份（`crates/host-api/include/msime_client.h`）。第 4a 步已经把 Rust 函数表交给后端 dylib、实现了注入的会话存储（[Apple 宿主经一张函数表调用 Rust](2026-10-18-apple-rust-client-table.md)），这一篇记录 macOS 的页面怎样切过去、哪些故意没切。

macOS 与鸿蒙不同：Swift 的 `BackendAccountSession`、Tauri 设置应用的 `FileAccountSessionStorage` 和 Rust 的注入存储用的是同一组文件和同一把 `flock`（登录会话是 `account-refresh.lock`，匿名会话是 App Group 的 `backend-account-refresh.lock`），每次都在锁里重读，所以页面可以逐个切换，不会出现两个刷新方各拿同一枚刷新令牌刷新的情况。

## Decision

- 注册：后端 dylib 在 `MSIMEInstallRustClient` 收下函数表后立刻注册本进程的会话存储（`BackendRustAccount.registerShared`，布局 1、`SHARED`、`BackendRustStoreLayout.standard`）。输入法进程和独立设置窗口进程都经这里，各注册一次；Objective-C++ 不注册。client_agent 由 Rust 的 `msime_client_client_agent` 拼：平台 `macos`、输入法 bundle 的版本、`hw.model`、「macOS」加系统版本号、Info.plist 的 `MSIMEEdition`（没有时 full）；拼不出时整次注册失败，不带它注册会让登录以默认的 MSIME/Android 发出。
- `shared/backend/account/BackendRustAccount.swift` 是 Swift 侧唯一的入口：请求都带 `session_store`、不带目录；`status` 读不了存储时是 `account_storage`，不当作没有登录；`BackendRustClient.Failure` 的中文提示与共享账号页、Windows 原生设置同一套措辞。
- 账号页（4b）：登录账号的状态、登录方式、验证码登录、改名、退出、注销（`account_delete`，需要最近登录时提示先退出再重新登录、本机登录留着）都经 Rust。读不了登录状态时整页报原因，不退回本机账号：那样会把已登录的用户显示成匿名账号。输入源第一次激活时的匿名账号注册改调 `ensure_anonymous`，已登录或读不了登录状态时不调。
- 「本机账号」卡片留在 Swift（负责人决定）：匿名账号的改名、退出、注销，以及验证码登录成功后丢掉匿名会话和身份，仍走 `BackendAnonymousAccount.session` 和 `BackendAccountClient`。Rust 没有对匿名会话的这几个操作；匿名会话与 Rust 共用 App Group 的锁，可以共存。

## Alternatives considered

- **账号页改调 Rust 的同时把「本机账号」卡片改成只显示。** 最强的理由是 macOS 上就只剩 Rust 一个账号实现，`BackendAnonymousAccount` 可以跟着删。没有采用：这是功能变化（用户再也不能给本机账号改名、注销它），负责人决定保留现状；共用的锁保证两边轮流刷新匿名会话不会互相吊销。
- **读不了登录状态时照旧退回匿名账号。** 理由是页面总有东西可显示。没有采用：钥匙串锁着、状态文件权限不对时，用户其实是登录着的，显示成本机账号会让他以为被退出了，还可能在这时用验证码再登一次；Rust 的 `status` 把这种情况报成 `account_storage`，页面照实说（迁移差异 M41）。
- **client_agent 拼不出时照样注册，不带它。** 理由是注册失败整个账号页就不能用。没有采用：不带 client_agent 的注入存储以 MSIME/Android 发出所有登录请求，服务端「我的设备」会把 Mac 显示成 Android；拼 client_agent 不联网、不读文件，只有请求本身不合法时才失败，那是代码错误，应该在测试里暴露。

## Consequences

- **收益**：macOS 的登录会话只由 Rust 刷新和改写（共享界面的三页见下），「我的设备」显示 macOS 和机型，注销需要最近登录时有明确提示，锁屏或权限问题读不了会话时不再被当作已退出。
- **代价**：账号页的测试改用替身函数表（`platforms/macos/tests/core/FakeRustTable.swift`）断言发出的 action，Rust 自己的行为由 host-api 和 client-services 的测试覆盖；匿名账号仍有一份 Swift 实现。
- **还没切的**：`shared/backend-ui` 里 macOS 与 iOS 共用的三页（社区词包与回复模板、完整词库目录、云端候选与排序）仍用 Swift 的 `BackendAccountSession.shared`，与 Rust 共用 `account-refresh.lock`，随 iOS 那一组一起换成 Rust 的 `community` 和 `cloud_dictionary`。

## Verification

- macOS ctest `backend-account-swift`：替身 Rust 下账号页的状态、登录方式（没开放的不发请求）、验证码登录（带 `challenge_id` 和 `code`）、改名、需要最近登录的注销、退出所有设备、断网时退出、读不了登录状态时不退回本机账号、宿主没注册存储时全部失败；「本机账号」卡片的改名和注销不经 Rust，验证码登录成功后丢掉匿名会话。把读状态改回 `try?`、去掉登录后丢掉匿名会话时都失败。
- `swift test --package-path shared/backend`：`BackendRustAccountTests`（client_agent 由 Rust 拼、拼不出时不注册、一个进程只注册一次、请求带 `session_store` 不带目录、读不了存储是失败、中文提示）。
