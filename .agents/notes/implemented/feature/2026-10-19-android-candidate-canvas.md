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

独立裁切九宫格隔绝 Android 双线性采样的相邻颜色：设备像素验收发现直接用源矩形缩放会让红色角块混入绿色，不能用放宽像素断言代替修复。静态与 APNG 切片在工作线程缓存，GIF/WebP 的当前帧复用九张缓冲；这比只保留整图占用更多位图内存，但不在逐键路径重新解码。APNG 合成保留偏移、透明混合、恢复背景与恢复前帧；有限循环停在末帧。普通候选与展开面板的移动端布局保持各自的宿主结构，绘制能力不等于照搬桌面浮窗的全部版式。

本次验收只使用合成图片，没有上游两款完整皮肤的 Android 栅格或物理设备结果。系统减弱动画的首帧断言直接覆盖 APNG 的帧选择；系统设置通知与原生 GIF/WebP 停播仍需进一步运行时覆盖。其他平台、非 canvas 的全部装饰行为、所有 Windows 几何/材质槽位及社区上传契约仍未完成，详见关联笔记，不能把本改动报告为全平台完成。
