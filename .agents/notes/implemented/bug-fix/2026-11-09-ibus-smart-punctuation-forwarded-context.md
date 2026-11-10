# Agent Note: IBus 无 surrounding text 时保留智能标点前文

Status: implemented

## Problem

Issue #11166 报告 Ubuntu 24.04、Wayland、IBus 1.5.29 下，终端和 Firefox 的中文模式智能标点不按“数字后直出”工作。IBus 输入上下文不一定提供 surrounding text；当前宿主在没有 surrounding text 时只记输入法自己上屏的文字，而把转发给应用的 ASCII 数字和字母当成无关按键清掉。下一次输入逗号、句号或冒号时，宿主无法知道前一个字符是数字或字母，行为与设置不一致。

## Decision

在 IBus `process_key` 返回未处理、且按键是无修饰的 ASCII 字母或数字时，把这个刚转发给应用的字符记为智能标点影子前文。其它未处理按键继续清除影子；焦点、隐私、受限状态也不保存。已有 surrounding text 时仍以编辑器提供的文本为准，这个影子只作为没有 surrounding text 的回退。

测试在 IBus 冒烟中关闭 surrounding text，转发合成数字 `7`，确认后续句号输出 ASCII，再输入逗号确认标点边界会清除数字上下文并输出中文逗号。

## Alternatives considered

- **继续要求 IBus 客户端提供 surrounding text**：不采用。终端等客户端的能力由应用和输入法模块决定，输入法不能为缺少该接口的客户端补上文本查询；这会让已存在的设置在这些客户端永远失效。
- **把所有转发按键都记录进影子**：不采用。快捷键、方向键、删除和标点会让影子脱离文档，随后可能错误地把中文标点改成 ASCII；只记录无修饰 ASCII 字母和数字，并在其它按键上清除。
- **直接把每个标点都交给 Engine**：不采用。这样会丢掉“数字/字母后直出”的宿主上下文策略，并与 Fcitx5、Windows 的行为契约不一致。

## Consequences

- IBus 无 surrounding text 的终端和其它客户端现在能在数字或字母后按配置输出 ASCII `,`、`.`、`:`。
- 标点、修饰组合、光标编辑等其它转发按键会清掉影子，不会把过期的数字或字母带到后续输入。
- IBus 仍优先使用 surrounding text；本改动不改变能提供有效 surrounding text 的客户端路径。

## Testing

- 新增 IBus runtime smoke：无 surrounding text 时转发 ASCII 数字后，句号为 ASCII；随后逗号为中文，证明边界清除。
- 待在 Ubuntu 24.04 Wayland 的 Alacritty 和 Firefox 上用合成输入复验；不记录真实输入、账号或私人资料。
