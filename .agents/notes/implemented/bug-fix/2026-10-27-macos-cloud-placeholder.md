# Agent Note: macOS 候选窗隐藏云占位行

Status: implemented

## Problem

macOS 候选窗把 Engine 的云候选异步占位行（`source` 15）按普通候选渲染，导致它占用序号、翻页和展开网格位置。AI 旧占位行（`source` 12）也属于同一类不可上屏状态行。

## Decision

在 macOS 候选呈现层过滤 `source` 12 和 15。过滤后的列表统一用于普通候选窗、展开网格和装饰候选；候选自身的 `id` 保持不变，因此可见候选仍通过 Engine 的 generation/index 身份选择。新增独立的 `show_candidate_status` 开关，控制候选窗首行状态文字与枝叶装饰，不再依赖预编辑显示开关。

## Alternatives considered

无。继续绘制占位行会让用户看到不可选择的候选并改变序号映射。

## Consequences

云候选或 AI 请求等待期间，候选窗只显示可上屏候选；如果当前页只有占位行，候选窗会隐藏。状态文字和枝叶装饰由共享 React 候选窗口设置页的独立开关控制，保留拼音预编辑时也能隐藏它们。开关存入 `client-core::Preferences::show_candidate_status`，缺字段默认开启，macOS 原生宿主从共享偏好读取；冻结的 AppKit 回退页不增加控件。Option/Ctrl 数字键释义和候选菜单的删除快捷键同样按过滤后的可见顺序映射。

## Verification

新增共享设置页测试先确认缺少开关导致失败，再验证 macOS 开关独立保存、不改变拼音预编辑，Linux 不出现此入口。更新原生候选布局回归：AI 占位不参与读屏，云占位后的候选编号连续并保留原始身份，仅含占位的候选窗隐藏。共享偏好另覆盖缺字段默认值和关闭后的序列化。本机复用主仓既有 `target/` 运行 Rust 偏好回归与 clippy；复用 `target/macos-isolated` 的编译参数和未改动对象，只重编候选布局测试及偏好实现后链接临时候选测试，`candidate-row-layout` 通过。UI 11 项测试、类型检查和构建通过。本机系统盘空间低于 50 GiB，不启动新的完整构建树；完整 quick 门禁尚未运行。固定数量页目前仍沿用 Engine 的 `page/page_count`，跨页压紧和页数排除占位仍需后续实现。
