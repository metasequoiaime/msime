# Agent Note: Linux 桌面接通粤拼简体输出

Status: implemented

## Problem

[#10435](https://github.com/metasequoiaime/msime/issues/10435) 已经在 Engine 和移动端接入 `cantonese_simplified_output`，但 Linux 的 IBus 和 Fcitx5 仍把字符集按钮限制在全拼、双拼和五笔。粤拼用户即使打开了这个偏好，桌面宿主也没有入口切换，工具栏和状态菜单的勾选状态也读错了。

原决定和转换边界见[粤拼和粤语双拼输出简体](../feature/2026-11-02-cantonese-simplified-output.md)。本篇只补 Linux 宿主，macOS 和 Windows 仍由各自的后续 PR 处理。

## Decision

Linux 的字符集按钮按当前生效方案选择偏好：粤拼使用 `cantonese_simplified_output`，其他可做简繁转换的方案继续使用 `traditional_chinese_output`。粤拼下按钮显示「简体输出」，勾选表示 Engine 会把粤拼候选和上屏文本转换为简体；普通话方案仍显示「繁体输出」。IBus 菜单、IBus 工具栏、Fcitx5 状态动作、Fcitx5 工具栏和 Ctrl+Shift+F 共用同一套方案判定。

判定和偏好键集中在 `CharacterSetTogglePolicy.h`，避免两个 Linux 宿主各自复制粤拼分支。保存仍沿用现有 IBus 菜单偏好保存和 Fcitx5 顶层布尔偏好保存路径，Engine 继续负责候选转换和去重。

## Alternatives considered

- **让粤拼继续复用 `traditional_chinese_output`**：接线最少，但会把全局简繁偏好和粤拼专用转换混在一起，无法保持既有独立偏好和默认行为。
- **在 Linux 宿主内另写繁转简**：可以绕开宿主偏好更新，但会让候选显示、上屏和 Engine 的去重再次分叉，也会重复已有转换逻辑。
- **只改状态栏动作，不改工具栏和 IBus 属性**：改动较小，但同一按钮在不同入口会读写不同的偏好，用户仍无法从常用菜单确认当前状态。

## Consequences

- Linux 的 IBus 和 Fcitx5 粤拼用户可以直接切换简体输出，状态菜单、工具栏和快捷键保持一致；默认仍是关闭，现有用户的粤拼输出不变。
- 普通话方案的繁体输出路径和 `script_conversion_applies` 规则不变；粤拼不会误用全局繁体偏好。
- 本次没有改变 macOS、Windows 或移动端行为，也没有改变任何已有设置的出厂值。
- 通过宿主单测验证了方案到偏好键、标签和勾选状态的映射；完整 Linux 宿主构建和真实 IBus/Fcitx5 桌面验收仍需在有依赖和设备环境的机器上补跑。
