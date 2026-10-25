# Agent Note: Windows 7 独立背单词面板复用设置表单

Status: implemented

## Problem

Windows 7 已有背单词设置页，但普通托盘的独立面板仍需要 WebView。单独重写复习卡片会复制共享队列和学习进度规则。

## Decision

在 `panels/win7` 提供 `vocabulary` 路由，使用已有 `VocabularyForm` 和 `VocabularyData`。独立窗口仅负责标准控件、滚动、STA 文件选择与串行工作线程，不计算学习计划，不修改默认值。全部文件读写继续由共享服务和有限读写适配完成；导入取消、删除与清空确认的行为与设置页一致。普通启动器和安装包仍等待完整产品适配。

### 既有笔记审计

- [完整 Windows 7 支持](../../implemented/feature/2026-10-23-windows7-support.md)：部分重叠，本篇落实一个面板，不替代其安装与目标系统验收。
- [原生设置基础](../../implemented/feature/2026-10-23-windows7-native-settings-foundation.md)：部分重叠，复用该篇的背单词表单及共享服务边界。
- [独立剪贴板面板](../../implemented/feature/2026-10-24-windows7-clipboard-panels.md)：部分重叠，共用面板开发宿主，剪贴板焦点与来源规则不适用于复习窗口。

## Alternatives considered

- **重写一个仅显示卡片的面板。** 窗口可以更短，但会遗漏词书管理入口，也需要持续同步设置页的作答与错误处理，因此复用完整表单。

## Verification

Windows 11 服务会话的 x64／x86 MSVC `/W4 /WX` 构建与四套面板 CTest 通过。独立面板测试使用实际 HWND 与真实隔离共享服务，验证 CSV 和 UTF-16 emoji 导入、文件选择取消、设置、隐藏与显示释义、认识／不认识后的学习日程、持久进度、删除／清空取消及成功、超限文件无部分导入、损坏进度不自动重置，以及偏好修订不变；另覆盖滚动顶部、底部和结束消息。关闭时等待已受理任务完成。

## Consequences

背单词的独立原生入口可复用完整词书管理与复习能力，代价是保留设置页的长布局，需要滚动。双架构词书与复习窗口回归已在 Win7 SP1 交互桌面通过，托盘启动器与安装包已集成；设置导入／导出的系统文件对话框另在最终安装路径打开并取消。最终 quick 见[产品构建](2026-10-24-windows7-product-build.md)。
