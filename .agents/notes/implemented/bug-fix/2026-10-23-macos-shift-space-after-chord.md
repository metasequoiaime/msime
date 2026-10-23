# Agent Note: macOS Shift 组合键后的空格不再误切换中英文

Status: implemented

## Problem

#9463 报告 macOS 在按住 Shift 输入字符后继续按空格时，`Shift+空格` 快捷键会切换中英文并吞掉空格。输入控制器只根据当前事件判断快捷键，没有记录同一轮 Shift 是否已经被普通字符打断。

## Decision

由共享的 macOS 修饰键观察器记录 `Shift` 按住期间是否收到过普通字符按下。空格本身不算打断字符；如果这轮已经输入过其他字符，`Shift+空格` 交回英文或中文的正常按键路径，直到 Shift 松开后清除状态。原有单独的 `Shift` 轻点和未被打断的 `Shift+空格` 快捷键保持不变。

## Alternatives considered

只在输入控制器里检查前一个按键，会依赖事件顺序和按键释放，焦点切换或丢失释放事件时可能留下错误状态；将状态放在已观察完整按键流的 `MSIMEModifierTap` 中能和单按 Shift 的取消规则共用同一生命周期。

把所有 `Shift+空格` 都交给应用会消除误切换，但会破坏用户启用的快捷键。按本轮 Shift 是否被字符打断区分，保留设置行为并覆盖报告中的连续输入。

## Verification

新增 `ShortcutTest` 覆盖：Shift 先按普通字符后按空格时状态持续到 Shift 松开、空格本身不改变状态、输入控制器在英文模式下保留空格且不切换中英文。`ModifierTap.h` 已用 macOS Objective-C++ 语法检查；完整 macOS CMake 测试交给 PR CI。

## Consequences

同一轮 Shift 中先输入字符后按空格会按普通空格处理；用户松开并重新按下 Shift 后，原有 `Shift+空格` 快捷键仍可用。状态只保存在当前输入控制器，不记录输入内容或按键文本。
