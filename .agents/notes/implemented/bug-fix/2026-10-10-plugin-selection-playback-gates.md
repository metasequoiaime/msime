# Agent Note: 插件选择同步打开播放开关

Status: implemented

## Problem

插件详情页选择音效包、旋律包或音乐包时，`withPackSelected` 只保存包 id。设置列表会把包显示为当前选择，但播放开关可能仍关闭，或者发声方式仍指向另一类声音，导致用户选择后输入法继续静音。

## Decision

选择普通音效包时同时打开 `key_sound` 并切换到 `keys`；选择旋律包时同时打开 `key_sound` 并切换到 `melody`；选择音乐包时同时打开 `music`。各包原有音量和其他偏好继续保留。

## Alternatives considered

- 只修改设置页的“使用中”标记：这会掩盖播放开关与所选包不一致，用户选择后仍然听不到声音。
- 选择包后只打开对应开关，不切换 `key_sound.mode`：在另一种发声方式已经启用时，普通音效包或旋律包仍不会播放。
- 把开关和模式修改交给各个详情组件：相同规则会在多个入口重复，直接集中在 `withPackSelected` 可保证所有插件选择入口一致。

## Consequences

插件详情页的“设为当前”操作现在代表立即使用该包；用户仍可在“声音与效果”中单独关闭开关或切换模式。选择操作不会改变音量、上屏音、成就音效及其他插件设置。

## Verification

`pnpm --filter @msime/desktop exec vitest run tests/settings/plugins-section.test.tsx --reporter=verbose --maxWorkers=1 --no-file-parallelism`：51 passed。

`pnpm --filter @msime/desktop typecheck`、`pnpm --filter @msime/desktop build`、变更文件格式检查、`pnpm run verify-notes`、`git diff --check` 和 `bash scripts/verify-local.sh --quick` 均通过。仓库全量格式检查仍报告基线中的 `apps/desktop/src/main.tsx` 与 `packages/ui/src/theme/theme-catalog.json` 两个未涉及文件格式问题。
