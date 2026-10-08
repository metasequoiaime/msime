# Agent Note: 光标移动延迟构造编辑文本

Status: implemented

## Problem

`edit_at_caret` 在处理 `MoveLeft`、`MoveRight`、`MoveHome` 和 `MoveEnd` 时先完整构造 `editing_text()`，随后只使用它的长度。普通全拼光标移动因此为不需要的字符串副本付出一次分配。

## Decision

先调用无分配的 `editing_text_len()` 计算边界；只有退格和前向删除确实需要修改文本时才构造 `editing_text()`。移动命令继续走原有的重解码和候选刷新流程。

## Alternatives considered

- **继续为所有命令构造完整编辑文本**：实现简单，但四个只读移动命令会为不需要的字符串副本分配内存。
- **缓存完整编辑文本**：可以进一步复用内容，但需要在引擎刷新、方案切换和本地模式变化时维护失效，扩大会话状态边界。
- **为每个移动命令分别计算长度**：会重复方案分支逻辑；复用已有的 `editing_text_len()` 保持长度语义唯一。

## Verification

旧实现的回归测试为 78 次分配并失败；延迟构造后降为 77 次并通过。会话测试、完整 `msime-engine` 测试、Clippy 和格式检查均通过。

## Consequences

全拼光标左移的分配次数从 78 次降至 77 次，光标边界、候选重解码以及删除路径行为保持不变。
