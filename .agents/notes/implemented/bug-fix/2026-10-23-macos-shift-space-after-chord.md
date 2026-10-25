# Agent Note: macOS Shift 组合键后的空格不再误切换中英文

Status: implemented

## Problem

#9463 报告 macOS 在按住 Shift 输入字符后继续按空格时，`Shift+空格` 快捷键会切换中英文并吞掉空格。输入控制器只根据当前事件判断快捷键，没有记录同一轮 Shift 是否已经被普通字符打断。

## Decision

由共享的 macOS 修饰键观察器记录 `Shift` 按住期间是否收到过普通字符按下。空格本身不算打断字符；如果这轮已经输入过其他字符，`Shift+空格` 交回英文或中文的正常按键路径，直到 Shift 松开后清除状态。原有单独的 `Shift` 轻点和未被打断的 `Shift+空格` 快捷键保持不变。

每个事件都先检查当前修饰键；只要已无 `Shift`，便结束上一轮取消状态，再处理普通按下或松开。不能只在 `flagsChanged` 的释放边沿清理：系统可能漏交该通知，而后续字符的 `keyDown` 或 `keyUp` 已确认释放。双侧 Shift 只松开一侧时仍保留取消状态；新一轮的空格及其重复不重新取消快捷键。

## Alternatives considered

只在输入控制器里检查前一个按键，会依赖事件顺序和按键释放，焦点切换或丢失释放事件时可能留下错误状态；将状态放在已观察完整按键流的 `MSIMEModifierTap` 中能和单按 Shift 的取消规则共用同一生命周期。

把所有 `Shift+空格` 都交给应用会消除误切换，但会破坏用户启用的快捷键。按本轮 Shift 是否被字符打断区分，保留设置行为并覆盖报告中的连续输入。

## Verification

新增 `ShortcutTest` 覆盖：Shift 先按普通字符后按空格时状态持续到 Shift 松开、空格本身不改变状态、输入控制器在英文模式下保留空格且不切换中英文。`ModifierTap.h` 的 macOS Objective-C++ 语法检查和最终集成源码的完整本地 quick 均通过；本树 `target/macos-isolated` 实际运行完整 `shortcut` CTest，88 秒通过。系统 IMK 与真实编辑器验收仍未执行。

补充漏释放通知后的普通 `keyDown` / `keyUp`、双侧 Shift、下一轮空格与重复、reset，以及单按 Shift 开启/关闭时的控制器恢复。提取新增测试函数并编译真实头文件，在原提交上以“普通事件后取消状态未清理”断言失败。既有 Ctrl+Shift+E 后再测 Shift+空格的夹具增加真实释放与新按下边沿，两组快捷键不应被写成同一轮 Shift。

## Consequences

同一轮 Shift 中先输入字符后按空格会按普通空格处理；用户松开并重新按下 Shift 后，原有 `Shift+空格` 快捷键仍可用。状态只保存在当前输入控制器，不记录输入内容或按键文本。

[可编辑焦点限制](../feature/2026-10-24-macos-editable-shift.md) 只约束可选的单按 Shift，不改变本文的快捷键或状态归属规则。
