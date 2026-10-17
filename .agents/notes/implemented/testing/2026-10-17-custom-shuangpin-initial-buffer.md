# Agent Note: 复用自定义双拼校验首键文本缓冲

Status: implemented

## Problem

自定义双拼表校验要扫描 26 个可能的首键。每个首键先由 `initial_for_key` 构造一个新的 `String`，再交给两键编码检查；普通单字母首键也会走堆分配，虽然编码拼接本身已经有复用的 `buffer`。

## Decision

将首键写入 `decode_pair` 已有的复用字符串：`append_initial` 对多字母声母写入静态单位，对普通首键直接追加 ASCII 字符；随后在同一缓冲中追加韵母并执行既有合法性和冲突判断。校验错误字段、零声母优先级和自定义表语义保持不变。

## Historical audit

[自定义双拼方案](../../implemented/feature/2026-10-10-custom-shuangpin-profile.md)定义了编码校验、错误字段和 `check_custom_profile` 不留存的边界；本片只优化校验内部的短文本所有权，不改变公开接口或校验规则。旧实现已在本仓库提交历史中确认，当前 `develop` 未包含本片。

## Alternatives considered

- **继续为每个首键创建 `String`**：实现最简单，但重复分配与随后复用的编码缓冲不一致。
- **为首键引入新的栈数组类型**：可以避免分配，但会增加首键表示和生命周期分支；直接写入已有 `String` 不改变调用契约。
- **改变 `decode_pair` 返回借用文本**：需要重做冲突错误的拥有型字段，扩大修改面；当前错误路径仍可按需克隆最终冲突文本。

## Verification

TDD 回归固定小鹤表：旧实现分配 449 个缓冲，首键缓冲复用后为 411 个；进一步用可复制的首键/韵母描述替代已发现读法后降为 6 个（剩余为校验集合本身），阈值为 10；先以旧代码验证红灯，再以新代码验证绿灯。随后运行 `shuangpin::custom::tests`（14 项）和 `cargo clippy -p msime-engine --all-targets -- -D warnings`。

## Consequences

自定义双拼表校验减少首键构造和已发现读法的堆分配；错误信息和所有合法表判定继续由原有路径产生。该优化只覆盖校验热路径，已生成并缓存的 `ShuangpinProfile`、双拼解码和宿主接口均不变。
