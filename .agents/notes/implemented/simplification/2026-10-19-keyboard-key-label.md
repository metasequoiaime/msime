# Agent Note: 统一屏幕键盘字母判断与按键标签

Status: implemented

## Problem

`KeyboardPanel` 绘制键面、`useKeyboardPanel` 同步 Shift 键面与生成发送提示时，重复计算单字符 ASCII 字母和 Shift 标签。三处表达式相同，但调用方决定是否使用 Shift 的规则不同：键面只跟随 Shift，发送字母使用 Caps Lock 与 Shift 的异或。复制表达式容易让标签逐渐偏离。

## Decision

`keyboard-input.ts` 提供公共 `isKeyboardLetter` 与 `keyboardKeyLabel`，供三处调用。标签方法只根据调用方传入的布尔值选择原标签、显式 Shift 标签或 ASCII 大写回退，不读取修饰键状态，不重新解释 Caps Lock。[公共按键按钮](2026-10-19-keyboard-key-button.md)保留单字符键面限制，状态 hook 保留发送大小写判断、虚拟键和同步 DOM 更新时机。

历史检索：[屏幕键盘状态](../../implemented/simplification/2026-10-19-keyboard-panel-state.md) 管理面板状态边界，与本片部分重叠，保留并互链；实体键 Caps Lock 切换、Engine 大写拼音和原生平台键面笔记属于其他输入路径，不修改。没有已有笔记承载共享面板标签计算。

## Alternatives considered

- **标签方法直接接收整个修饰键集合**：调用更短，但会把键面与发送提示的 Caps Lock 差异藏进方法，或让调用方伪造修饰键集合；保留显式布尔参数。
- **直接统一为 Unicode 字母判断**：可以支持更多字母，但会改变现有非 ASCII 标签的大写回退与 Shift 发送语义；本片只抽取既有规则。

## Verification

公共入口的行为测试覆盖 ASCII、非 ASCII、标点、空 Shift 标签与具名键；平台键盘、连发、布局、状态 hook 和设置集成测试分别验证显示标签、虚拟键和 Shift/Caps 发送组合。

## Consequences

三处标签计算和字母判断只保留一份实现。调用方显式决定 Shift 状态，继续保留键面的单字符限制，发送字母的 Caps Lock 与 Shift 异或，以及同步 DOM 更新时机。公共方法接收现有键类型并返回标签，不改变布局、请求队列与宿主注入。本片只验证共享前端，不声称完成原生设备验收。
