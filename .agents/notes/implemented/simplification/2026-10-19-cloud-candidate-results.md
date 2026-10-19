# Agent Note: 云端候选结果与固定位置视图

Status: implemented

## Problem

`CloudCandidatesPanel` 已将状态放入公共 hook，但候选列表、手机提示和固定位置记录仍内联在面板中。复用结果呈现时容易遗漏快捷短语只读、Unicode 单字删除限制和固定记录的静态结构。

## Decision

提供受控 `CloudCandidateResults` 与 `CloudFixedPositions` 及 props 类型。候选视图接收已确认的查询文本、版本、词库类型、候选、忙碌状态和选定位置，复用 `CloudDictionaryItem`，通过回调提交原对象；固定位置视图保留静态主内容。查询可见性由父面板控制，候选与手机提示以 Fragment 返回，不增加 DOM 包装。请求、确认和刷新留在 hook。

## Existing notes audit

按 `CloudDictionaryItem`、云端候选面板、固定位置视图检索活跃笔记。

- [候选状态](../../implemented/simplification/2026-10-19-cloud-candidates-panel-state.md)部分重叠，继续持有异步与确认约束，视图归本篇。
- [macOS 账号桥接](../../implemented/architecture/2026-10-18-macos-account-through-rust.md)仅涉及宿主协议，无关本次呈现抽取。
- 其他命中为引擎排序和平台候选窗口，无关；没有完全吸收或过时提案。

## Alternatives considered

- 把候选和固定位置组合成一个组件可减少 props 接线，但两份结果独立到达，固定记录没有交互主按钮，分别导出更符合现有边界。
- 直接复用 `CloudDictionaryItem` 渲染固定位置可减少 JSX，但会把静态内容变成按钮，因此保留原结构。

## Verification

新增组件回归先因组件缺失出现 8 项失败；抽取后覆盖结果顺序、空态、快捷短语只读、忙碌禁用、Unicode 单字与英文例外、原对象动作和选定位置。真实面板固定按钮回归在抽取前后通过，保留规范编码、确认与按原查询刷新；现有确认生命周期、hook 和宿主消费者共 9 文件、72 项通过。桌面构建、HarmonyOS 类型检查、lint、格式与笔记检查通过；独立审查另跑 2 文件、21 项及桌面类型检查通过。构建 CSS 的 SHA256 为 `d5f8b485896f48db99d4883b4cc93221613f3ed3aba7f1a6d589a3d0d9b3377d`，与抽取前一致。

## Consequences

两个受控组件可独立复用，代价是父面板显式连接动作与数据。组件不管理查询生命周期；父级继续在已确认查询后显示候选。固定位置保持独立显示，不能等待它到达才显示候选；组件测试与构建不代表真实账号或原生设备验收。
