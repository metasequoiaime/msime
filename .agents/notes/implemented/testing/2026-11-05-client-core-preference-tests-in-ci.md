# Agent Note: msime-client-core 偏好测试进主机 CI

Status: implemented

## Problem

2026-11-05 #10809（macOS 跨输入法使用原有语音快捷键）给 `VoiceInput` 加了布尔字段 `hotkey_global`，没有登记进 `crates/client-core/src/preferences/tests.rs` 的 `NOT_CREDENTIALS`。`every_credential_like_preference_field_is_listed` 把名字里带 key 的字段都当成凭据类字段，要求它们要么在 `credential_slots` 里、要么在豁免名单里，于是这条测试在 develop 上变红。

它在 #10809 的 PR 上没有跑。`msime-client-core` 的单测整套只在 Android 设备作业里由 `build-core-test.sh` 运行；Linux 资源集成作业设了 `MSIME_SKIP_RUST_TESTS`，Windows 设置作业只挑了两条偏好测试。#10809 只改桌面和 macOS，不触发 `android-device-pr`，所以合入时全绿。合入之后，所有跑设备验收的 PR（#10941、#10949）的 API 28 和 API 35 都在这一条上失败，与它们自己的改动无关。

## Decision

- `NOT_CREDENTIALS` 补上 `voice_input.hotkey_global`，和同组的 `hotkey_ctrl_f9`、`hotkey_ralt` 等并列。
- `ci-platforms.yml` 的 `Rust engine` 作业在 `msime-input-runtime` 测试之后加一步 `cargo test --locked -p msime-client-core --lib preferences::`，触发条件与引擎测试相同（共享路径改动），复用同一份编译缓存。偏好测试只读写临时目录，不依赖设备或平台 SDK，在 Studio 上 295 项 2.4 秒跑完。

## Alternatives considered

- **只补名单**：最强的理由是改动最小，立刻解开被卡住的 PR。没有只做这一步：下一个只改桌面的 PR 新加带 key 的偏好字段，还会同样绕过检查，再把 develop 弄红。
- **主机 CI 跑整套 `msime-client-core` 测试**：覆盖更全。没有采用：整套里有依赖资源目录和文件描述符上限的用例（Studio SSH 会话里曾因 fd 上限报 unable to open database file），在 ubuntu runner 上跑需要先逐个确认，这次只放进确定无平台依赖的偏好模块。
- **让只改共享偏好的 PR 也触发 `android-device-pr`**：能整套跑到，但一次要占约 110 分钟 runner，为一条纯逻辑测试不值得。

## Consequences

- **收益**：新加偏好字段漏登凭据名单，会在引入它的 PR 上就变红，不再等到下一个跑设备验收的 PR 才暴露。
- **代价**：`Rust engine` 作业多一次 `msime-client-core` 的编译和约几秒的测试。
- **未覆盖**：偏好以外的 `msime-client-core` 模块仍只在设备作业里整套运行。
