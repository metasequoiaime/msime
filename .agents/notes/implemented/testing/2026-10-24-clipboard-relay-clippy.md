# Agent Note: 剪贴板参考中继进入严格 Clippy 门禁

Status: implemented

## Problem

`msime-clipboard-relay` 已进入完整本地验证的测试列表，但严格 Clippy 循环仍未枚举它。`cargo check --workspace` 和真实测试不能替代全目标 `-D warnings` 检查，现有的「第一方 crate 警告作为硬失败」约定因此漏掉参考中继。

## Decision

将 `msime-clipboard-relay` 加入现有全目标严格 Clippy 循环，复用原来的失败收集和硬失败规则。

历史检索：[词库构建器门禁](../../implemented/testing/2026-10-24-dict-builder-local-gates.md)与本项部分重叠，持有同一显式枚举机制，保留其独立组件边界；[引擎 CI](../../implemented/testing/2026-10-10-engine-tests-in-ci.md)持有 quick 与完整验证的分工；参考中继、端到端加密与客户端架构笔记持有运行时协议和依赖，与本项不重叠。其余命中为具体功能或缺陷，没有被本项吸收的提案。

## Alternatives considered

- **改成一次工作区 Clippy 调用**：新包不易再被漏掉。但桌面壳和目标专用包有各自的构建前置条件，现有分包检查有意避免把缺少桌面资源当成警告回归。本项沿用已经运行的枚举机制。
- **仅加入工作区默认成员**：裸 Cargo 命令能自动选中该包。但本地验证按包显式调用 Clippy，默认成员不会改变真正的执行列表。

## Testing

执行现有中继测试与全目标严格 Clippy，并检查脚本实际循环仅新增这一包；完整 quick 门禁按原规则运行，结果随本轮提交提供。

## Consequences

完整验证增加中继及其测试目标的检查时间；冷缓存还需编译相关依赖。quick 继续按原规则只编译，不在其中新增 Clippy 阶段。该改动不启动中继服务，也不改变中继协议、断言、时限、跳过规则或已知失败列表。
