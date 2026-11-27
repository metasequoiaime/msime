# Agent Note: #11863 macOS 五笔自造词位置入口迁移边界

Status: proposed

## Problem

#11863 要求在 macOS 上为五笔自造词位置增加设置入口。共享 Rust 偏好 `input.wubi_phrase_placement`、默认值和引擎行为已经存在，但当前 macOS 设置界面仍由共享 React 页承载，`platforms/macos/src/settings/` 的 AppKit 窗口只作为设置应用无法启动时的回退。

此前 PR #11902 把入口加进 AppKit 回退窗口，违反了 `docs/ARCHITECTURE.md` 中“macOS 的 AppKit 设置窗口冻结为回退”的边界，已关闭，不能作为本 issue 的合并实现。

## Proposal

把 macOS 入口随 macOS 原生设置迁移实现。迁移完成前不在冻结的 AppKit 回退窗口或 `packages/ui` 增加新设置控件。共享 Rust 继续持有字段、默认值、可见条件和持久化规则，原生 macOS 设置只适配同一 Host API。

Windows、Android、iOS 的原生入口可以分别按 #11863 的平台范围推进。macOS 入口实现 PR 应在原生设置宿主可用后单独使用 `Fixes #11863`，并验证五笔方案、学习开关、位置切换和新造词入库行为。

## Alternatives considered

- 重新提交 #11902 的 AppKit 回退实现：会把新功能继续放入已冻结的回退窗口，违反现有架构决定。
- 在共享 React 页临时增加 macOS 控件：会扩大 `packages/ui` 的设置职责，与迁移期间只修缺陷的约定冲突。
- 先修改默认值或缺字段行为：会改变已有用户的输入手感，且不是本 issue 授权范围。

## Acceptance criteria

- macOS 原生设置迁移完成并提供对应入口。
- 入口只在五笔方案且学习开启时显示，读写共享字段并保持默认值不变。
- 新造词位置在 macOS 真实输入法和共享 Rust 回归测试中按用户选择生效。
- 实现 PR 使用 `Fixes #11863`，并包含 macOS 原生构建及真实编辑器验收。

## Risks

在冻结回退窗口中增加控件会使两套 macOS 设置行为分叉，迁移后还要维护已弃用入口。临时放入 React 页则会延长共享 WebView 设置的生命周期，偏离原生迁移计划。
