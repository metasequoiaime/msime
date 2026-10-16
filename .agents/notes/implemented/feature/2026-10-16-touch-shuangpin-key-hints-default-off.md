# Agent Note: 触屏双拼键位提示默认关闭

Status: implemented

## Problem

触屏双拼键位提示已经有共享开关，设置页也允许用户重新打开；但 `touch_shuangpin_key_hints` 在共享偏好、Android、iOS、HarmonyOS 和共享设置页的缺省值仍为开启。#7758 要求新安装的 26 键双拼键面默认保持简洁，同时保留手动开启能力。

既有开关设计见[触屏双拼键位提示开关](2026-10-09-touch-shuangpin-key-hints.md)。本篇只记录默认值从开改关的决定，不改变该开关的存储和重绘契约。

## Decision

`touch_shuangpin_key_hints` 的缺失值和新安装默认值统一为 `false`。共享 `Preferences` 用显式的 `disabled_by_default` serde 默认函数和 `Default` 值；Android、iOS、HarmonyOS 及共享 React 设置页对缺失或类型不对的字段都按关闭处理。用户点开设置后仍写入 `true`，已有文档里明确保存的 `true` 不被迁移覆盖；桌面的 `shuangpin_keymap_hint` 是候选窗键位图，属于另一项设置，不随本次改变。

## Alternatives considered

- **只改新安装模板，旧文档缺键仍按开**：可以减少升级用户的行为变化，但缺键无法区分新安装和升级，且同一份共享文档在不同宿主会显示不同默认，不能满足用户看到的统一默认关闭。
- **删除整个提示功能**：能得到干净键面，但会破坏已经依赖提示的用户和现有设置入口；保留显式开关可以让需要提示的用户恢复原行为。

## Consequences

缺少该字段的文档在三种移动原生宿主和共享设置页都不再显示提示；明确保存的 `true` 或用户重新开启仍正常显示。旧文档不会被自动写入字段，避免无必要的配置迁移；账号同步字段和键面策略保持不变。

## Verification

更新了 client-core 偏好回归、iOS 默认值回归、HarmonyOS 账号默认映射和 Android 缺省读取。`cargo test -p msime-client-core --locked preferences` 通过（147 个偏好测试及匹配的集成测试）；HarmonyOS 键盘逻辑测试通过（4125 + 59 项断言）；共享设置 637 个测试文件、3673 项测试通过，TypeScript 编译通过；notes 和 `git diff --check` 通过。Android `check-host.sh` 因本机未设置 `ANDROID_SDK_ROOT` 跳过，iOS 原生测试留给 CI（本 worktree 没有 staged iOS 静态库）。
