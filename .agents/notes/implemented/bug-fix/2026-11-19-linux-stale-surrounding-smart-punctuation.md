# Agent Note: Linux IBus 过期 surrounding text 下保留智能标点前文

Status: implemented

## Problem

部分 IBus 客户端会发布可用但过期的 surrounding text，或者把光标位置报在文本开头。用户刚转发数字后按句号时，宿主只看这份文本，无法识别数字前文，于是输出中文句号。无 surrounding text 的终端路径已经记录并处理转发数字，但不能覆盖这种半可用的客户端状态。

## Decision

IBus 额外保留最近一次转发给应用的 ASCII 字母或数字，只到紧接的 `,`、`.` 或 `:` 为止。surrounding text 的光标前是 ASCII 字母或数字时继续以它为准。光标在开头、光标前是非 ASCII 字符或无法从文本取得有效的 ASCII 前文时，回退到这次转发的字符。光标移动、删除、其他按键、输入法自己上屏和非智能标点都会清除临时前文。

## Alternatives considered

- 继续完全信任 surrounding text：实现最简单，但会复现报告中客户端发布过期光标位置时的中文句号。
- 无论 surrounding text 内容如何都使用临时前文：可能把用户移动光标后其他位置的标点误判成数字后标点，因此只在 surrounding text 不能提供 ASCII 前文时回退。
- 把转发字符追加进长期影子：会让客户端拦截、改写或丢弃按键后留下过期状态，影响后续编辑，所以临时值只保留到下一次智能标点。

## Consequences

- Deepin、Chrome 等发布过期或无效光标位置的 IBus 客户端在数字后输入句号、逗号和冒号时可以保持 ASCII。
- 正常提供 ASCII 前文的 surrounding text 路径不变，中文前文、光标移动和其他按键仍不会误触发英文标点。
- 临时前文只存在于输入法进程内存，不写入偏好、日志或用户数据。真实报告环境仍需补做视觉和输入验收。

## Verification

- Linux runtime smoke 新增过期光标位置回归，合成转发数字后断言句号为 ASCII。
- 原有无 surrounding text、有效 surrounding text 和重复标点回归保持不变。
- `git diff --check` 通过，Linux native host 与 PR CI 负责编译和运行完整测试。

## 既有笔记审计

- [IBus 无 surrounding text 时保留智能标点前文](2026-11-09-ibus-smart-punctuation-forwarded-context.md) 部分重叠，本次保留其无 surrounding text 行为，并补充半可用 surrounding text 的回退边界。
