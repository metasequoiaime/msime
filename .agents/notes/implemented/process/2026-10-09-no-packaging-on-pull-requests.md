# Agent Note: PR 上只编译和测试，打包挪到合进 develop 之后

Status: implemented

## Problem

功能分支的 PR 上在打包。2026-10-09 的 #6395 是一个 iOS 皮肤功能，因为动到了 `crates/client-core`、`crates/host-api` 和 `packages/ui`，`ci-platforms.yml` 判定所有原生平台都要跑，其中：

- `android-device` 在 API 28 和 35 两台模拟器上各打一次完整 APK（bootstrap vcpkg、`build-apk.sh`），再装包跑冒烟，是整个 PR 里最重的部分；API 35 那台失败，让一个 iOS 功能的 PR 变红。
- Web engine 作业把 `target/web-engine/dist/` 上传成 `web-engine-<sha>`，没有任何流程下载它。
- `ci-macos-package.yml` 挂在 `pull_request` 上，PR 一碰到打包输入就在 Depot macOS runner 上打一次未签名的 universal 包（`timeout-minutes: 120`）。

这三项都不在 develop 和 main 的必需检查里（必需的是 `macOS 15 arm64`、`macOS 15 x86_64`、`iOS Simulator`、两项 `quality`、`Android host`），跑了也不挡合并，只花 runner 时间和让 PR 变红。维护者的要求是：功能分支只编译和测试，不打包。

## Decision

- PR 上保留各平台的编译、lint 和单元测试（`Android host`、`Linux native host`、`Linux settings app`、`HarmonyOS host`、`Windows GNU cross build`、`Windows release script tests`、`Web engine (wasm)` 的构建和测试步骤），范围仍由 `Decide platform scope` 按改动路径决定。
- `ci-platforms.yml` 的 `android-device` 加上 `github.event_name != 'pull_request'`：只在推到 develop、main（以及手动触发、merge_group）时跑。`release-android.yml` 的 `device` 门禁不变，发版前用同一个提交再跑一次。
- `ci-platforms.yml` 的 web engine 产物上传加同样的条件，PR 上只验证能构建。
- `ci-macos-package.yml` 的触发从 `pull_request` 改为 `push` 到 develop，加上原有的手动触发和每周定时。判定步骤改用 `github.event.before..github.sha` 两个点的 diff，只有这次推送碰到打包输入时才打包。去掉 `concurrency`：前一次推送碰了打包输入、紧接着的推送没碰时，取消前一次就等于那次改动从没打过包；没碰打包输入的运行十几秒就结束。
- `scripts/test-macos-package-resources.py` 这类静态打包契约检查不受影响，`scripts/verify-local.sh` 照常每次都跑。

本篇取代 [Android 模拟器上的 PR 门禁和发版门禁](../testing/2026-10-08-android-device-gates.md) 里「在 PR 上调用 `android-device.yml`」这一条，那篇其余决定（模拟器脚本、发版门禁、跳过名单规矩）仍然成立。

## Alternatives considered

- **维持现状，设备门禁留在 PR 上**：那篇笔记的理由是 0.3.0 的两个回归都是发版当天合入的，在 PR 上失败时责任和上下文都还在。没有采用，因为这个理由的代价被低估了：它让每一个碰到共享 crate 的 PR（包括 iOS、macOS 功能）都背上两台模拟器各几十分钟的打包和间歇失败，而合进 develop 后的 push 仍然在发版前很久就报出来，出问题的提交范围也只是一次推送。
- **把路径判定拆细，让共享 crate 的改动少点亮几个平台**：能减少一部分编译，但不解决「PR 上在打包」本身；碰到 `client-core` 的 PR 仍然会打 APK。拆细判定是另一件事，不在这次改动里。
- **完全不在 PR 上构建，只在 develop 上构建**：省得最多。没有采用，因为 `Android host`、macOS、iOS 是必需检查，编译失败必须在合并前拦住；没有 merge queue 时，编译错误一旦进 develop 就挡住所有后续 PR。
- **macOS 打包改为只定时运行，不跟 develop 推送**：更省，但打包输入的改动要等最多一周才知道有没有打坏。跟推送且按路径判定，绝大多数推送十几秒结束，只有真碰到打包输入的推送才打包。

## Consequences

- **收益**：功能分支的 PR 不再打 APK、不起模拟器、不打 macOS 包、不上传 web 产物；模拟器的间歇失败不再让无关 PR 变红。
- **代价**：Android 设备上才出现的运行时问题（权限、SELinux、旧系统缺陷、首次启动崩溃）和打包脚本问题，要等合进 develop 后才报出，修的人得回头定位是哪次推送带进来的。动了文件读写、权限或安卓资源的改动，合并前仍要按 `AGENTS.md` 在本机模拟器上跑过。
- **仍然没有覆盖**：develop 上 `android-device` 或 macOS 打包失败时没有自动回滚或通知，靠看 develop 的 Actions 页面。

## Verification

- `actionlint` 和仓库的 workflow 校验通过；`scripts/test-macos-package-resources.py` 通过（它核对 `ci-macos-package.yml` 的判定仍覆盖每个打包输入）。
- PR 本身的 CI 上，`android-device` 显示 skipped，web engine 的上传步骤被跳过，`macOS package` 不再出现在 PR 的检查里。
