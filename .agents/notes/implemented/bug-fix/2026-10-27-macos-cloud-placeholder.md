# Agent Note: macOS 候选窗隐藏云占位行

Status: implemented

## Problem

macOS 候选窗把 Engine 的云候选异步占位行（`source` 15）按普通候选渲染，导致它占用序号、翻页和展开网格位置。AI 旧占位行（`source` 12）也属于同一类不可上屏状态行。

## Decision

在 macOS 候选呈现层过滤 `source` 12 和 15。过滤后的列表统一用于普通候选窗、展开网格和装饰候选；候选自身的 `id` 保持不变，因此可见候选仍通过 Engine 的 generation/index 身份选择。新增独立的 `show_candidate_status` 开关，控制候选窗首行状态文字与枝叶装饰，不再依赖预编辑显示开关。

## Alternatives considered

无。继续绘制占位行会让用户看到不可选择的候选并改变序号映射。

## Consequences

云候选或 AI 请求等待期间，候选窗只显示可上屏候选；如果当前页只有占位行，候选窗会隐藏。状态文字和枝叶装饰由共享 React 候选窗口设置页的独立开关控制，保留拼音预编辑时也能隐藏它们。开关存入 `client-core::Preferences::show_candidate_status`，缺字段默认开启，macOS 原生宿主从共享偏好读取；冻结的 AppKit 回退页不增加控件。Option/Ctrl 数字键释义和候选菜单的删除快捷键同样按过滤后的可见顺序映射。固定数量页发现占位后，从当前代次的完整快照中过滤并按用户页容量重新分页；显示页座位复用自动适应已有的上报协议，翻页、数字键与鼠标都保留原始 Engine 身份。占位被真实结果替换后清掉显示页并恢复普通固定分页；整页只剩占位时同样清理显示页。隐藏行仍占引擎索引，因此移动高亮的步数上限按原始身份距离计算。

## Verification

新增共享设置页测试先确认缺少开关导致失败，再验证 macOS 开关独立保存、不改变拼音预编辑，Linux 不出现此入口。更新原生候选布局回归：AI 占位不参与读屏，云占位后的候选编号连续并保留原始身份，仅含占位的候选窗隐藏。共享偏好另覆盖缺字段默认值和关闭后的序列化。本机复用主仓既有 `target/` 运行 Rust 偏好回归与 clippy；复用 `target/macos-isolated` 的编译参数和未改动对象，只重编候选布局测试及偏好实现后链接临时候选测试，`candidate-row-layout` 通过。UI 11 项测试、类型检查和构建通过。本机系统盘剩余 34 GiB，实际调用 `scripts/verify-local.sh --quick`，通过本机 Windows 原生策略 173 项、Harmony 设置打包、工作流与笔记检查、Rust workspace、Android 目标、wasm 检查和 Android 宿主检查。仓库契约中的 `windows package managers` 复现公共 workflow runner 配置失败；在 Linux 阶段创建新构建树前以退出码 75 主动停止，因此完整 quick 没有完成。macOS 原生改动由上述普通及 `--adaptive` 候选测试验证，其余平台继续由 PR CI 覆盖；不能把本次门禁记为全绿。固定数量分页回归覆盖横排与竖排：7/13 个原始候选中隐藏一个后分别显示一页/两页，页容量、页码、前后翻页、物理数字键、鼠标点选保持一致；同时覆盖占位消失恢复普通分页、大量隐藏行之间移动以及全部隐藏后清理显示页。
