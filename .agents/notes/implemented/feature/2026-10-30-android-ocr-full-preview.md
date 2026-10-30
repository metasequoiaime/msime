# Agent Note: Android OCR 全键区取景

Status: implemented

## Problem

#10233 要求拍照识字的相机画面覆盖整个键区。当前预览上限 180dp，返回、说明和拍照分别占行，小键盘中真正可见的画面更小。

## Decision

预览使用键区内的 `TextureView` 等比例中心裁剪，返回／设置在顶部、拍照在底部叠放，不挤占画面。保留工具栏和编辑器边界。拍照时冻结取景框比例，JPEG 先按 EXIF 转正，再按同一比例中心裁剪后识别，避免识别画面外的文字。

相机仍在专用线程串行启动、拍照和关闭。纹理销毁由相机关闭后释放，面板切换、隐藏键盘及编辑器切换继续作废代次。离开预览恢复公共面板原有边距，识别结果继续滚动显示。

## Related notes audit

- [离线 OCR](../../implemented/feature/2026-10-24-android-offline-ocr.md)部分重叠：保留模块信任、相机串行和会话代次；只替代 SurfaceView 必须完整容纳的预览选择，双方互链。
- [Android nightly 设备失败](../../implemented/bug-fix/2026-10-28-android-nightly-device-failures.md)与[手写遥测](../../implemented/bug-fix/2026-10-25-android-mlkit-handwriting-telemetry.md)只涉及已有设备基线及模型，不改变。
- 其他检索命中为 HarmonyOS 或内联填充，与取景无关。

## Alternatives considered

- 放大 SurfaceView 并完整容纳：保持整张照片可见且改动最少，但横向键区中的竖向画面仍很窄，不能覆盖整个键区。
- 放大 SurfaceView 后依赖父容器裁剪：可直接填满，但旧设备截图已经证明独立 Surface 会越界覆盖编辑器，不采用。
- 只裁剪预览、识别整张照片：保留更多照片内容，但会识别用户在取景框外看不到的文字，不采用。

## Verification

设备验收要求：API 28/35 原生产品中预览与键区同宽高；返回、设置和拍照可操作；合成照片取景不变形、不越界；裁剪覆盖横竖比例和中心标记；真实相机识别、整行及部分上屏、隐藏后重开和切编辑器作废通过。quick、notes 和构建通过。

## Consequences

TextureView 需要硬件加速；尺寸变化必须重新计算变换。视野比完整照片窄，识别输入也相应裁剪。设备相机预览与 JPEG 的实际视野仍受驱动影响。
