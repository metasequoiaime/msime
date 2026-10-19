# Agent Note: Android 候选皮肤的原生画布

Status: implemented

## Problem

Android 的 `CandidateSkinResolver` 已读取共享候选配色，但 `canvas` 包仍缺少九宫格、挂件和留白的实际绘制。仅接收清单或显示可读底色不能满足与 `msime-windows` 的视觉兼容。

## Decision

共享加载器继续拥有清单、路径和图片预算校验；Android 经现有 `skin_package` / `skin_resource` C ABI 在偏好工作线程取得元数据与图片。输入线程只消费不可变缓存，贴图解码失败保留共享可读回退，成功后才请求 `canvas_panel_drawn` 原配色。普通候选与展开面板复用同一原生 Canvas 场景，源切片按像素、边宽和挂件按 DIP，挂件按稳定层级绘制且不接收触摸。

APNG 按帧控制块重建 PNG 子帧，并用 Android 位图完成混合与处置；GIF/WebP 使用 Android 28 起的 `ImageDecoder` / `AnimatedImageDrawable`。系统关闭动画时显示首帧，隐藏与卸载时停止动画。资源与主题切换不替换候选按钮。

## 既有笔记审计

- [共享清单兼容](../../implemented/feature/2026-10-19-windows-skin-schema-parity.md)部分重叠：负责协议及安全回退，保留并互链。
- [macOS 原生画布](../../implemented/feature/2026-10-19-macos-candidate-canvas.md)部分重叠：采用相同像素/DIP、成功解码后切换配色的契约；平台解码与布局各自实现，保留。
- 检索 `proposed`、`implemented` 与 `rejected` 中的 `canvas`，没有另一份 Android 画布方案。

## Alternatives considered

- **只使用 `ImageDecoder`**：能直接复用系统的 GIF/WebP 解码和播放，但 Android 不保证 APNG 动画；把它的默认 PNG 当完整支持会静默丢帧，因此 APNG 单独解码。
- **用 WebView 绘制候选栏**：可以复用 Web 九宫格与浏览器动画，但发布 APK 的候选栏是原生 View，新增 WebView 会扩大进程、触摸与可访问性边界，因此保留原生 Canvas。

## Verification

API 28 与 API 35 专用 AVD 上的 `CandidateCanvasDeviceSmoke` 均通过，覆盖九宫格像素、挂件层级、四边留白、明暗换图、APNG/GIF/WebP 帧、有限循环、减弱动态效果、资源失效与候选点击路径。`CandidateSkinDeviceSmoke` 与 API 28 `smoke.sh --core` 通过；两个 API 的四个共享客户端 crate 设备单测通过。`check-host.sh` 的 247 项 JVM 冒烟和 186 项设备源编译通过，`verify-local.sh --quick` 通过。

## Consequences

APNG 子帧不能按独立完整画面解码，必须处理偏移、透明混合、恢复背景和恢复前帧。Android 固定高度候选行与桌面浮窗布局不同，留白必须进入外层测量，不能压缩候选文字。缓存每次偏好重读都会重新校验，但图片未变时应复用已解码内容。
