# Agent Note: 九键左列选项直接追加到读音列表

Status: implemented

## Problem

刷新九键读音时，`key_choices` 先创建一个临时 `Vec<String>`，再把其中唯一消费的选项追加到 `self.spellings`。字母字符串本身仍然需要拥有，但中间向量的申请和释放落在每次刷新热路径上。

## Decision

`NineKeySession` 在追加左列选项前按键面字母数预留 `self.spellings`，逐项生成并追加大写字母和可原样上屏的数字。计算规则、选项顺序、锁定时是否显示数字和九键双拼的键面保持不变。旧的 `key_choices` 仅在测试配置下保留，供冻结刷新对照，不进入产品构建。

## Alternatives considered

- 保留临时 `Vec`：实现最简单，但每次刷新都会多申请一个只消费一次的容器。
- 让 `key_choices` 返回迭代器：可避免中间向量，但需要把键面、表和锁定状态的借用生命周期穿过追加调用；直接在会话上追加更清晰，也能显式预留目标容量。

## Consequences

读音字符串的申请仍然存在，因为每个选项需要独立拥有的 `String`；只删除临时选项向量。目标 `spellings` 可能按键面上限多预留少量容量，后续刷新继续复用它。候选与读音快照不改变。

## Verification

固定合成词库、数字 `64` 的刷新在旧实现上为 111 次分配，新实现通过不超过 110 次的预算；新旧读音列表逐项相等，覆盖 `ni`、`mi`、零声母项和原样数字项。九键相关测试、Engine/runtime 测试、Clippy、fmt、笔记校验和 quick 门禁作为本切片最终门禁。
