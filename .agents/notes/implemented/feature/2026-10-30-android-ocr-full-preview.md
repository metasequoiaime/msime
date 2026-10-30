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

API 28 专用 arm64 AVD 的原生产品专项通过：预览边界逐像素对照原工具面板的 ScrollView，拍照及返回／设置按钮均位于键区内；横向、纵向及不裁剪三组合成像素保持中心。固定合成相机图经过真实驱动拍照、离线识别、整行上屏、只选 `2468` 上屏、隐藏重开及编辑器切换作废均通过。预览与部分上屏截图已目测，画面无拉伸且没有覆盖工具栏和编辑器。首轮因忘记关闭专用 AVD 的网络被离线前置断言拒绝，断网后完整重跑通过，未改断言。

原生 Gradle release APK、JNI 导出、ELF／16KB 对齐及测试 APK 编译通过。远端两轮 `bash scripts/verify-local.sh --quick` 通过；缺失工具链的阶段按脚本跳过，本机另跑 `pnpm run verify-notes`。新增真实返回工具面板、从预览打开设置并重开编辑器的点击检查在 API 28 完整复跑通过。API 35 专项仍待完成，不以 API 28 结果代替。

## Build provenance

Rust、共享代码及资源锁与固定提交 `587b0f9304` 无差异，复用它已经验证的 Rust `.so`、固定完整资源及依赖声明；JNI 从本分支 C++ 重新编译，Java 由本分支 Gradle 重编并按现有开发签名脚本签名。没有复制其他检出的未提交源码，不复用其他分支的 JNI。相机输入由仓库固定 `OcrCameraImage.java` 生成，不读取用户图片。

## Consequences

TextureView 需要硬件加速；尺寸变化必须重新计算变换。视野比完整照片窄，识别输入也相应裁剪。设备相机预览与 JPEG 的实际视野仍受驱动影响。
