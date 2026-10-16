# Agent Note: 算式有限结果在显示舍入时保持数值

Status: implemented

## Problem

`V` 模式的 `evaluate` 已拒绝无穷值和 NaN，但 `format_number` 为压到 15 位有效数字，先格式化科学计数文本，再解析回 `f64`。最大有限值被格式化为 `1.79769313486232e308` 后，重新解析超过 `f64::MAX`，于是有限算式 `(2^1023)*1.9999999999999998` 最终给出 `inf` 候选，负值同样变成 `-inf`。

## Decision

保留现有 15 位有效数字、普通数值显示区间和负零归一化规则。舍入后的科学计数文本解析为非有限值时直接返回该文本；它仍是有限数值的十进制近似，不能再通过浮点表示来决定是否显示。真实求值溢出继续无结果。

## Alternatives considered

- 统一改用最短往返格式：所有有限 `f64` 都能显示且不需要这次解析，但改变现有消除 `0.1+0.2` 二进制尾数的产品规则。
- 在舍入后再拒绝非有限值：能消除 `inf` 候选，但会把本来有效的算式变成原文回退；保留已有科学计数文本即可显示正确近似值。

## Verification

新增格式化/算式查询及真实 Session 按键、选择上屏回归，在旧实现分别以 `Some("inf")` 与 `inf` 候选失败，修复后通过。正负最大有限值保留 15 位科学计数文本，真实求值溢出仍无数值候选。完整引擎 2173 项通过、33 项忽略，golden 31 项通过；引擎 Clippy（`-D warnings`）、`cargo fmt --all --check`、quick、笔记门禁及 `git diff --check` 通过。quick 覆盖 Rust 工作区、Android/WASM 编译、Android 宿主 Java 测试、macOS 原生宿主与共享 Apple 桥接层构建；Linux、Windows 交叉编译与 Harmony 真编译因本机工具或依赖不齐跳过。独立审查未发现阻断，并另行验证 11 项算式查询及 6 项 Session 测试。

## Consequences

合法的大数算式不再误报为无穷值，正常大小数值的显示规则继续由原路径处理。显示结果仍是现有 `f64` 求值的近似结果，文本舍入后不保证能再由 `f64` 表示；本次不更换数值求值库或承诺任意精度。此改动只涉及纯引擎计算与显示，不改变文件读写、目录、权限或 Android 资源。

## 历史笔记检索

检索活跃笔记的 `format_number`、`SIGNIFICANT_DIGITS`、`exmex`、算式和有效数字。现有[带符号单位换算](../../implemented/bug-fix/2026-10-16-signed-unit-conversion.md)复用此显示函数但拥有单位语法；[Android 数字键盘算式](../../implemented/feature/2026-10-08-android-nine-key-number-pad.md)使用独立的 `BigDecimal` 策略；两者均不持有本次显示舍入决定。

## 参考

使用 Rust 标准库的 [`f64::is_finite`](https://doc.rust-lang.org/std/primitive.f64.html#method.is_finite) 区分数值范围，科学计数文本由既有的 [`LowerExp`](https://doc.rust-lang.org/std/fmt/trait.LowerExp.html) 格式化路径产生；没有新增数值库。
