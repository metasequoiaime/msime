# Agent Note: Android 候选皮肤的原生画布

Status: implemented

## Problem

Android 的 `CandidateSkinResolver` 已读取共享候选配色，但 `canvas` 包仍缺少九宫格、挂件和留白的实际绘制。仅接收清单或显示可读底色不能满足与 `msime-windows` 的视觉兼容。

## Decision

共享加载器继续拥有清单、路径和图片预算校验；Android 经现有 `skin_package` / `skin_resource` C ABI 在偏好工作线程取得元数据与图片。输入线程只消费不可变缓存，贴图解码失败保留共享可读回退，成功后才请求 `canvas_panel_drawn` 原配色。普通候选与展开面板复用同一原生 Canvas 场景，源切片按像素、边宽和挂件按 DIP，挂件按稳定层级绘制且不接收触摸。

APNG 按帧控制块重建 PNG 子帧，并用 Android 位图完成混合与处置；GIF/WebP 使用 Android 28 起的 `ImageDecoder` / `AnimatedImageDrawable`。系统关闭动画时显示首帧，隐藏与卸载时停止动画。资源与主题切换不替换候选按钮。展开画布盖住普通键区时，`ImeFrame` 将整行键区设为 `INVISIBLE`，保留候选覆盖层所依赖的测量高度，并隐藏按键及单手侧栏；收起后恢复。外接键盘的收起状态仍优先使用 `GONE`。顶部候选条保留在展开层上方：`alignOverlaysBelowTopRow` 把偏移设到外层 `expandedCandidateCanvas`，内部滚动区只使用画布内边距，不能把窗口坐标写成嵌套滚动区的边距，否则画布会盖住候选条并在顶部留下空白。

## 既有笔记审计

- [共享清单兼容](../../implemented/feature/2026-10-19-windows-skin-schema-parity.md)部分重叠：负责协议及安全回退，保留并互链。
- [macOS 原生画布](../../implemented/feature/2026-10-19-macos-candidate-canvas.md)部分重叠：采用相同像素/DIP、成功解码后切换配色的契约；平台解码与布局各自实现，保留。
- 检索 `proposed`、`implemented` 与 `rejected` 中的 `canvas`，没有另一份 Android 画布方案。

## Alternatives considered

- **展开时将底层键区设为 `GONE`**：可同时移除绘制和触摸，但覆盖层依赖普通键区撑起高度，移除测量会使键盘塌缩，因此使用 `INVISIBLE`。
- **隐藏整列**：能一起消除底层按键和旧候选条的透出，但会移除原有的顶部候选条及展开切换入口，因此只隐藏被覆盖的按键行，并把画布外层正确对齐到顶部候选条下方。
- **使用不透明面板遮挡**：能保留底层布局，但会破坏上游半透明材质和外留白，因此不采用。

- **只使用 `ImageDecoder`**：能直接复用系统的 GIF/WebP 解码和播放，但 Android 不保证 APNG 动画；把它的默认 PNG 当完整支持会静默丢帧，因此 APNG 单独解码。
- **用 WebView 绘制候选栏**：可以复用 Web 九宫格与浏览器动画，但发布 APK 的候选栏是原生 View，新增 WebView 会扩大进程、触摸与可访问性边界，因此保留原生 Canvas。

## Verification

API 28 与 API 35 专用 AVD 上的 `CandidateCanvasDeviceSmoke` 均通过，覆盖九宫格像素、挂件层级、四边留白、明暗换图、APNG/GIF/WebP 帧、有限循环、减弱动态效果、资源失效与候选点击路径。`CandidateSkinDeviceSmoke` 与 API 28 `smoke.sh --core` 通过；两个 API 的四个共享客户端 crate 设备单测通过。`check-host.sh` 的 247 项 JVM 冒烟和 186 项设备源编译通过，`verify-local.sh --quick` 通过。完整上游验收修复后，重新构建完整原生 APK，在 API 28 与 API 35 上重跑 `CandidateCanvasDeviceSmoke`、`CandidateSkinDeviceSmoke`、`CandidatePanelDeviceSmoke`，六组均通过；设备测试源与默认测试 APK 也重新编译通过。

## Consequences

固定上游的完整皮肤验收使用 `tests/device/upstream-skins.sh`：`build-editor.sh` 仅在显式指定 `MSIME_ANDROID_UPSTREAM_SKINS` 时从固定提交归档两款原始包到测试 APK，默认包不含上游资源。Android 15 限制单个 APK 最多 100 个 instrumentation 入口，专用上游构建在生成的清单中替换合成画布入口，不增加默认清单的条目。独立 instrumentation 使用真实 JNI 检查面板和全部挂件解码，再通过实际输入法保存普通／展开、浅／深色截图并检查点击上屏；图片需要人工查看，不能把保存成功报告为像素或动画一致。运行只允许专用模拟器，拒绝覆盖已有同名皮肤，finally 恢复偏好并删除本次安装的目录。截图通过应用外部私有目录导出后清理，不要求产品开启 debuggable 或 root。

独立裁切九宫格隔绝 Android 双线性采样的相邻颜色：设备像素验收发现直接用源矩形缩放会让红色角块混入绿色，不能用放宽像素断言代替修复。静态与 APNG 切片在工作线程缓存，GIF/WebP 的当前帧复用九张缓冲；这比只保留整图占用更多位图内存，但不在逐键路径重新解码。APNG 合成保留偏移、透明混合、恢复背景与恢复前帧；有限循环停在末帧。普通候选与展开面板的移动端布局保持各自的宿主结构，绘制能力不等于照搬桌面浮窗的全部版式。

固定上游 `dd018f79858314ed542a47c528608752b13bd261` 的 `green_leaf`、`sakura-night` 已在 API 28、35 专用 AVD 的完整原生 APK 上验收：面板和全部挂件经 JNI 解码成功，两款皮肤 × 明暗 × 普通／展开各八张截图（合计十六张）已实际查看，未再出现底层字母键透出或画布盖住上方候选条。自动断言覆盖键区不可见、顶部候选条位置保留、展开／收起时窗口尺寸不变、顶部切换和底部返回两种收起路径，以及候选上屏。原始 APK 上的底层按键断言先失败，再验证修复通过。没有物理设备结果。系统减弱动画的首帧断言直接覆盖 APNG 的帧选择；系统设置通知与原生 GIF/WebP 停播仍需进一步运行时覆盖。其他平台、非 canvas 的全部装饰行为、所有 Windows 几何/材质槽位及社区上传契约仍未完成，详见关联笔记，不能把本改动报告为全平台完成。
