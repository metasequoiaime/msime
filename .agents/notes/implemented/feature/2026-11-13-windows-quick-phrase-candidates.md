# Agent Note: Windows 快捷短语混入普通候选

Status: implemented

## Problem

共享引擎已经支持把编码相同的快捷短语混进普通候选，偏好也已经落在 `mixed_input.quick_phrase`。当前 Windows 会话没有向 host-api 请求 `quick_phrase_candidates`，Windows 原生设置也没有这项开关，因此 Windows 用户无法使用 #10991 要求的能力。旧决定只允许 macOS、Linux 请求该能力，见 [快捷短语混输与调频两个共享偏好](../../implemented/feature/2026-10-22-quick-phrase-preferences.md)；本 issue 的报告者后来明确要求 Windows 10/11，因此这次只 supersede 其中的 Windows 范围，HarmonyOS 另开独立 issue 分支和验收。

## Decision

Windows 的 `HostCapabilities` 现在声明 `quick_phrase_candidates`，Windows 的 Server 和 TSF 会话选项包装器都写入该能力字段；host-api 继续把它与 `mixed_input.quick_phrase` 相与，所以用户关闭偏好时不会出现快捷短语。Windows 原生设置在「输入 → 候选与联想 → 更多选项」增加「快捷短语混输」开关，读写同一个共享偏好，并在「候选混输」总开关关闭时置灰。共享偏好的默认值和缺字段规则保持不变：本次只开放能力，不翻转已有用户的默认行为。

Windows 控件对本项只查询已存对象，不回退新装默认对象：共享序列化在 `quick_phrase=true` 时省略字段，旧文档缺字段同样读作开启；新装文档则显式保存 `false`。若沿用普通控件的出厂默认回退，用户开启并重新打开设置后会被误显示为关闭。这个读取分支不改变共享层默认值或缺字段行为。真实 WinUI 验收覆盖新装关闭、开启后存储省略字段、进程重启后的开启回读，以及关闭后的页面回读。

共享能力测试覆盖 Windows、macOS、Linux 与移动端的能力矩阵；Windows 生命周期回归测试确认两个会话入口传入 `quick_phrase_candidates`，共享 host-api 已有的偏好门控测试继续覆盖开关行为。

快捷短语能力与大写锁定标点分别验证：两项能力的宿主范围已经不同，旧的合取断言改为只验证标点，快捷短语由六平台矩阵完整验证，保留所有未接入宿主的关闭断言。

## Alternatives considered

- 只在 Windows 设置页增加开关：设置值会被宿主忽略，用户会看到一个无效控件，因此不能接受。
- 让 Windows 无条件打开混输：会改变现有用户的候选顺序，也绕过已有的 `mixed_input.quick_phrase` 偏好门控；这次只开放能力，不改默认值。
- 把 Windows 与 HarmonyOS 放进同一个 PR：违反一个 issue 一个 PR，也无法在同一套设备验收条件下完成；HarmonyOS 另开分支处理。

## Consequences

Windows 用户打开「快捷短语混输」后，普通组字会按共享引擎的现有规则显示编码相同的快捷短语；关闭开关或关闭总的「候选混输」时，仍可使用 Shift+K 的快捷短语模式。没有改变 macOS、Linux 或其他平台的默认值。真实编辑器中的混输及 Shift+K 仍需在 Win10/11 复验；已有真实 WinUI 的控件、落盘与重启回读证据，不把缺少编辑器复验等同于实现未完成。


## Verification

- 整合提交 `2ec8b92e98` 的完整 quick `20261117-133227` 退出 0（49 分 04 秒），Windows 可移植 232 项、Linux 133 项、Rust/WASM、Android Java、Linux/macOS 宿主、Apple bridge 及 x86_64/i686 pipe-only 构建通过。完整 MinGW 与 Harmony HAP 缺工具链的阶段明确跳过。
- 最后只加强截图脚本和本笔记，没有改动生产源码。脚本在 Windows PowerShell 5.1 解析无误，并在真实 WinUI 基线窗口中用同一可见范围函数验证完整开关落在正文视口内；不能只用 `IsOffscreen`，它在控件被裁剪时也可能返回可见。
- 改前展开图来自 Windows 验收机既有 WinUI 缓存（干净检出 `7baffcb16e`），使用独立合成设置目录；确认「更多选项」中没有快捷短语开关，邻接开关完整可见。没有切换或修改该缓存工作树。临时交互任务结束后已清理，图片保留。最终新增开关的展开图和回读仍由原 PR 的 Windows 作业验证。
