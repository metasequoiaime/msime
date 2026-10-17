# Agent Note: 保留运行时上游模型完整性能提交链

Status: implemented

## Problem

运行时末层裁剪提交 `03b3b5db8c6758f5489edd29bf7f5c40ae3bc0b7` 已由后续上游提交 `c690cec2c5fb1a64f4ca7d0b447d57e4249fe538` 继承；后者还包含 query scratch、QKV 和 attention 工作缓冲复用。仅固定到较早提交会回退这些后续性能改动。

## Decision

四处 `chinese-ime-lm` 锁定统一恢复到 `c690cec2c5fb1a64f4ca7d0b447d57e4249fe538`。该提交是 `03b3b5db8c6758f5489edd29bf7f5c40ae3bc0b7` 的后代，保留上文末层无用输出裁剪，同时保留后续已验证的模型工作缓冲优化。相关决定见[末层前缀输出裁剪](2026-10-17-runtime-prefix-final-row.md)和[模型 query scratch](2026-10-17-runtime-query-scratch.md)。

## Alternatives considered

- **固定到 `03b3b5db`**：能直接接入末层裁剪，但会丢失之后的 query、QKV 与 attention scratch 优化。
- **跟随上游 `main`**：能自动取得最新提交，但破坏构建输入可复现性，也无法逐项验证新行为。
- **只恢复两个 `Cargo.toml`**：表面依赖正确，但主锁文件和 fuzz 锁文件仍会固定旧提交，`--locked` 构建不一致。

## Consequences

项目继续获得末层前缀裁剪和后续模型缓冲复用，四处锁定保持可复现一致；不会引入新的模型格式或 API。未来更新上游提交时必须确认它仍包含已验证的性能提交链，并同步两个锁文件。

## Verification

配置回归先在错误的 `03b3b5db` 状态下确认四处命中 `c690` 为 0，再恢复后确认四处命中。引擎、Golden、Runtime、Clippy、Rustfmt、笔记校验、diff 检查和 quick 门禁均重新运行。
