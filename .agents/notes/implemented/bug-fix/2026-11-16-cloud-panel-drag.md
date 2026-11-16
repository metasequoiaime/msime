# Agent Note: 桌面云面板标题栏支持拖动

Status: implemented

## Problem

Windows 的云剪贴板和云词库使用无边框 Tauri 面板窗口，窗口默认在工作区底部居中。共享 `CloudPanelHeader` 没有接入桌面面板已有的 `beginWindowDrag` 手势，用户无法把云剪贴板或个人词库窗口移到其他位置；云词库的目录、导入导出、应用到本机和候选排序子面板也有同一缺口。

## Decision

`CloudPanelHeader` 接收面板客户端的可选 `beginWindowDrag`，统一复用 `usePanelDrag`。标题栏上的关闭、返回按钮仍由拖动 Hook 排除；没有窗口拖动能力的移动端和内嵌页面继续保持原行为。所有云剪贴板、云词库及其子面板都把自己的客户端传给标题栏，因此 Windows 桌面宿主可以通过现有 Tauri `startDragging` 移动窗口。

## Alternatives considered

- 只在 Windows 宿主把面板改成带系统标题栏的窗口：会改变所有共享面板的视觉和尺寸契约，也无法覆盖无边框的其他桌面宿主。
- 每个云面板单独复制指针事件和拖动调用：会让云剪贴板、云词库子页面的排除按钮和失焦清理逻辑逐渐分叉；现有 `usePanelDrag` 已经为其他面板验证了这套行为。

## Consequences

标题栏空白区域现在可以拖动云剪贴板和云词库的全部独立窗口；按钮点击仍只执行按钮动作。移动端或嵌入设置页没有 `beginWindowDrag` 时不会发起窗口操作。改动只涉及共享面板标题栏契约，不改变窗口默认的底部居中位置、大小或业务请求。
