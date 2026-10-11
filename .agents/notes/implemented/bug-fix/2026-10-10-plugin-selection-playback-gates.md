# Agent Note: 插件选择同步打开播放开关

Status: implemented

## Problem

插件详情页选择音效包、旋律包或音乐包时，`withPackSelected` 只保存包 id。设置列表会把包显示为当前选择，但播放开关可能仍关闭，或者发声方式仍指向另一类声音，导致用户选择后输入法继续静音。

## Decision

选择普通音效包时同时打开 `key_sound` 并切换到 `keys`；选择旋律包时同时打开 `key_sound` 并切换到 `melody`；选择音乐包时同时打开 `music`。各包原有音量和其他偏好继续保留。

## 开关在哪里生效

这些开关是 `preferences.plugins` 里的偏好，读它们的是 host-api 的 `SessionSound`，不是各平台宿主。宿主只决定什么时候报告：

- macOS 已接入[桌面生命周期核心](../architecture/2026-10-11-desktop-lifecycle-core.md)并打开 `FEEDBACK`：按键音来自每个物理按下的 `msime_client_key`，上屏音来自核心在派发里认出的上屏，背景音乐由 `msime_client_set_active` 的所有者仲裁。它们和以前一样受 `key_sound`、`key_sound.mode`、上屏音开关和 `music` 约束，所以「选包即打开开关」的规则不变；变的是 macOS 宿主不再自己调 `msime_client_key_sound`、`msime_client_commit_sound`、`msime_client_music_set_active`。
- 失焦上屏在 macOS 上不再出上屏音（`setFocused:NO` 的上屏，以及 IMK 的 `commitComposition:` 和关会话前经命令 `MSIME_FINISH_ON_FOCUS_LOSS` 结束的组字），安全输入（密码框）由核心按字段属性 `SECURE` 静音并停掉音乐。
- Windows、IBus 和 Fcitx5 仍由宿主直接调用上面三个函数，接入生命周期核心后照此改写。

## Alternatives considered

- 只修改设置页的“使用中”标记：这会掩盖播放开关与所选包不一致，用户选择后仍然听不到声音。
- 选择包后只打开对应开关，不切换 `key_sound.mode`：在另一种发声方式已经启用时，普通音效包或旋律包仍不会播放。
- 把开关和模式修改交给各个详情组件：相同规则会在多个入口重复，直接集中在 `withPackSelected` 可保证所有插件选择入口一致。

## Consequences

插件详情页的“设为当前”操作现在代表立即使用该包；用户仍可在“声音与效果”中单独关闭开关或切换模式。选择操作不会改变音量、上屏音、成就音效及其他插件设置。

## Verification

`pnpm --filter @msime/desktop exec vitest run tests/settings/plugins-section.test.tsx --reporter=verbose --maxWorkers=1 --no-file-parallelism`：51 passed。

`pnpm --filter @msime/desktop typecheck`、`pnpm --filter @msime/desktop build`、变更文件格式检查、`pnpm run verify-notes`、`git diff --check` 和 `bash scripts/verify-local.sh --quick` 均通过。仓库全量格式检查仍报告基线中的 `apps/desktop/src/main.tsx` 与 `packages/ui/src/theme/theme-catalog.json` 两个未涉及文件格式问题。

macOS 接入生命周期核心之后：`shortcut` 断言控制器只经 `msime_client_key`、派发回复的 `feedback` 和 `msime_client_set_active` 报告，旧的三个声音调用一次都没有；`session-smoke` 用真实 host 库确认开关全关时不起播放器，并在按键音和上屏音开着（音量 0，不出声）的会话上确认旧调用在选择加入 `FEEDBACK` 之前照常排队、之后成了空操作。选包后在真机上出声这一步没有手测。
