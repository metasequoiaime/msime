# Agent Note: 本地皮肤安全测试按设备可创建的夹具拆分

Status: implemented

## Problem

`develop` 的 Android API 28、35 设备单测都在 `local_files_refuse_symbolic_links_and_fifos` 的 `mkfifo` 准备步骤失败，设备日志为 `Permission denied`。失败运行上传的 API 35 `logcat` 明确记录 shell 域的 `{ create }` 被拒绝，类型为 `fifo_file`，且 `permissive=0`。现有目录清单和同步状态的 FIFO 用例已记录该限制；新用例把符号链接和 FIFO 放在一起，导致可在设备验证的符号链接防护也没有执行。

## Decision

拆成独立的符号链接和 FIFO 用例。符号链接文件与符号链接祖先目录仍在 Android、macOS、Linux 上运行。仅 FIFO 用例使用与已有用例一致的 Android `cfg_attr(ignore)`，注明 SELinux 的 `fifo_file create` 限制，在允许创建 FIFO 的 Unix 主机继续实际运行拒绝断言。生产代码与设备 runner 的筛选规则保持。

## Existing notes audit

检索 `mkfifo`、`fifo_file`、`adb shell`、`SELinux` 和本地皮肤读取：候选同步目录句柄及 `client-core` 私有目录句柄笔记持有生产读取与存储边界，不持有本次测试夹具拆分规则；本地皮肤统一选择笔记持有界面分类，与测试失败无关。未找到本次设备夹具覆盖面的已有归属。

## Alternatives considered

- **整个合并用例在 Android 上忽略**：与已有 FIFO 用例的注解相同且改动最少，但会连同设备可以覆盖的符号链接防护一起跳过，因此拆分测试。
- **改用 `libc::mkfifo` 创建夹具**：避免依赖设备的命令工具，但无法绕开 SELinux 的内核访问控制，不能修复已经确认的权限拒绝。

## Verification

原有 `develop` CI 的 API 28、35 日志均在合并用例的 `mkfifo` 断言失败。远端 macOS 的 `cargo test -p msime-client-services --lib skin::candidate_community::tests::local_files_ --locked` 4/4 通过，FIFO 和符号链接用例均实际运行；`cargo clippy -p msime-client-services --all-targets --locked -- -D warnings` 通过。

在专用 API 28、35 arm64 AVD 上分别运行 `MSIME_ANDROID_TEST_SERIAL=<对应设备> bash platforms/android/tests/device/build-core-test.sh`，四个客户端 crate 的设备单测均通过；两台设备上 `client-services` 各 1176 通过、10 忽略、4 个硬链接用例按既有 runner 规则筛除。符号链接用例均为 `ok`，新拆出的 FIFO 用例均按明确原因忽略。格式检查使用 `rustfmt --check --edition 2021 crates/client-services/src/skin/candidate_community/tests.rs`。

远端 `bash scripts/verify-local.sh --quick` 通过，包含 macOS 原生宿主与 Apple 桥接层构建、Linux 容器原生测试 92/92 通过。本机 `pnpm run verify-notes` 与 `git diff --check` 通过。

## Consequences

Android 仍无法构造真实 FIFO 夹具；不能把忽略标为设备 FIFO 防护验收。保留 Unix 主机上的真实 FIFO 拒绝断言，同时让 Android 实际执行符号链接防护检查，避免因夹具准备失败而阻断整套设备验收。
