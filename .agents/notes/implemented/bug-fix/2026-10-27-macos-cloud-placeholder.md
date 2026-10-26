# Agent Note: macOS 候选窗隐藏云占位行

Status: implemented

## Problem

macOS 候选窗把 Engine 的云候选异步占位行（`source` 15）按普通候选渲染，导致它占用序号、翻页和展开网格位置。AI 旧占位行（`source` 12）也属于同一类不可上屏状态行。

## Decision

在 macOS 候选呈现层过滤 `source` 12 和 15。过滤后的列表统一用于普通候选窗、展开网格和装饰候选；候选自身的 `id` 保持不变，因此可见候选仍通过 Engine 的 generation/index 身份选择。新增独立的 `show_candidate_status` 开关，控制候选窗首行状态文字与枝叶装饰，不再依赖预编辑显示开关。

## Alternatives considered

无。继续绘制占位行会让用户看到不可选择的候选并改变序号映射。

## Consequences

云候选或 AI 请求等待期间，候选窗只显示可上屏候选；如果当前页只有占位行，候选窗会隐藏。状态文字和枝叶装饰可由候选窗口设置页的独立开关控制，隐藏预编辑时仍可单独保留。

## Verification

运行 `git diff --check`、`pnpm run verify-notes`，并进行静态检查确认普通候选窗、展开网格和装饰候选均经过过滤。未运行完整 macOS 原生构建：当前磁盘可用空间低于仓库要求的 50 GiB。
