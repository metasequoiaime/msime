# Agent Note: 键盘与手写设置拆成顶部标签

Status: implemented

## Problem

桌面「键盘手写」把屏幕键盘与手写内容连续排在同一页。两者是不同的输入方式，用户要求通过两个顶部标签分别进入。

## Decision

共享「键盘手写」页复用 `useSettingsTabs`，提供「键盘」「手写」两个顶部标签，默认显示键盘。键盘预览、尺寸、按键反馈与键盘外观入口属于键盘标签；`HandwritingSettingsContent` 属于手写标签。

标签仅在 `handwritingOnScreenKeyboardPage` 成立的宿主启用，沿用已有页面投影。侧栏仍只有「键盘与手写」入口，旧 `handwriting` 路由仍进入合并页；手机与不提供手写的宿主保留现有独立页面。标签选择不写入偏好，两个面板保持挂载以保留状态，未选面板通过 `hidden` 隐藏。

## Historical audit

导航标题按[功能语义命名](../simplification/2026-10-19-settings-navigation-semantic-titles.md)调整为「键盘与手写」，标签与路由仍遵循本篇。

- [精简悬浮工具栏、屏幕键盘和手写输入三页的设置](../simplification/2026-10-16-toolbar-keyboard-handwriting-settings-prune.md)：部分重叠。本决定替代其中把手写内容紧接键盘内容排列的选择，保留合并侧栏入口、能力过滤和旧路由别名的理由。
- [桌面按键速查改为顶部标签](2026-10-19-shortcuts-reference-top-tabs.md)：使用同一个标签组件，属于独立页面的决定，保留。

## Alternatives considered

**继续在同一页顺序排列**：无需切换即可查看两种输入方式，而且改动最少；但两种功能仍混在一起，不能满足用户明确要求的两个标签入口。

**重新拆成两个侧栏页面**：可以直接访问各自的页面与路由；但会增加侧栏入口，丢失先前精简导航的收益。页内标签满足此次需求并保留现有导航。

## Verification

共享设置集成测试分别进入键盘与手写标签，检查 macOS 固定键盘几何、Linux 与 Windows 的尺寸能力、手写打开动作，以及 HarmonyOS 2-in-1 的说明内容。macOS 用例确认未选面板不可访问、切回键盘复用原预览节点，且不触发偏好保存；HarmonyOS 手机用例继续检查原独立页面。

- 相关设置集成用例 58 项、页面投影与导航用例 29 项通过；前端类型检查与生产构建通过。
- `MSIME_BUILD_JOBS=6 bash scripts/verify-local.sh --quick` 通过，笔记门禁通过。
- 本机设置应用重新打包、安装，Developer ID 严格签名校验通过。通过无障碍树与窗口截图确认默认键盘标签、切换到手写再返回键盘后只显示各自内容。原生输入面板未改动，本次不重复验收输入行为。

## Consequences

两类内容各自完整显示，切换标签会多一次点击。标签复用既有无障碍语义与方向键导航；偏好数据、保存协议与原生输入面板不变。共享组件测试不代表其他平台设备验收。
