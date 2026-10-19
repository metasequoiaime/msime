# Agent Note: HarmonyOS 画布资源安全快照与显式 APNG 解码

Status: implemented

## Problem

[HarmonyOS 候选画布](2026-10-19-harmony-candidate-canvas.md) 允许设备解码器只返回 APNG 默认图。SDK 的 `createPixelMapList` 只承诺 GIF/WebP 多帧能力，无法据此保证 Windows APNG 皮肤播放。默认图还可能根本不在动画内，静态回退不能作为动画兼容的完成条件。

## Decision

设备验收还暴露了所有画布图片共有的失效点：`@Prop` 深拷贝后的 `Uint8Array` 不再具有内部 TypedArray 槽位，首次读取长度就报 `This value does not have a [[TypedArrayName]] internal slot`。因此组件快照使用 Base64 字符串，解码租约内部才恢复 `Uint8Array`，避免将原生字节对象传入观察状态；循环读取器也仅接收已经恢复的字节。[OpenHarmony 的 @Prop 限制](https://github.com/openharmony/docs/blob/master/zh-cn/application-dev/ui/state-management/arkts-prop.md#限制条件)明确要求深拷贝后只有基本类型、Map、Set、Date 和 Array 保留类型。字符串约增加三分之一的编码长度；相比 `number[]` 传每个字节，它不会把最大 16MiB 压缩快照膨胀成大量数字元素，也不另建跨组件资源注册表和释放协议。

增加不依赖 ArkUI 的有界 PNG/APNG 分块解析与预乘 RGBA 合成。Image Kit 只负责重建后的静态 PNG 子帧解码与最终 PixelMap 创建；动画时钟、循环和候选层继续用既有实现。支持默认图包含或排除两种结构、SOURCE/OVER、NONE/BACKGROUND/PREVIOUS，首帧 PREVIOUS 按 BACKGROUND 处理。解析在位图分配前校验 CRC、序号、帧数、矩形和累计像素预算。CRC、容器、zlib 头校验或原生解码失败时，动画整张失败，交给既有可读回退，不把默认图伪装成成功动画。

检索归属：[HarmonyOS 候选画布](2026-10-19-harmony-candidate-canvas.md) 部分重叠，本篇替代其中 APNG 依赖系统多帧解码的决定，其他画布及生命周期契约保持归原篇；[Android 候选画布](2026-10-19-android-candidate-canvas.md) 部分重叠，是本仓库已有的拆帧方案参考，仍拥有 Android 实现；[v2 动画契约](../architecture/2026-10-13-candidate-skin-v2-motion-contract.md) 与[跨平台绘制边界](../architecture/2026-10-13-cross-platform-candidate-rendering-boundaries.md) 保留共同边界；其他宿主、社区及图库笔记与本篇的 ArkTS 解码决定无关。不新增解析依赖，规范来源为 [PNG 第三版 11.3.6](https://www.w3.org/TR/png/#11apng)。

设备截图还要求原始 RGB 值完全相等，不能只看到动画就通过。[Image Kit 的 readPixels](https://github.com/openharmony/docs/blob/master/zh-cn/application-dev/reference/apis-image-kit/arkts-apis-image-PixelMap.md#readpixels7)固定输出 BGRA_8888，即使源 PixelMap 请求了 RGBA_8888 也不改变该边界。读取后先交换 R/B，再按源 alpha 类型归一化并合成；忽略这个约定会让红/蓝和绿/黄验收色互换，像素断言必须拒绝。

面板与下层挂件必须是实时 Canvas 节点，不能放进 `background(CustomBuilder)`：背景 builder 的内容更新不保证触发背景重绘，设备会停在第一帧。使用 `Stack` 内前后两个 Canvas，以 `LayoutPolicy.matchParent` 在候选内容确定尺寸后匹配宽高；内容决定 `wrapContent` 高度，避免旧的 100% 高度反向撑满。九宫格源矩形保持像素语义，但本宿主最低 API 21，`drawImage(PixelMap)` 的源坐标自 API 18 起按 vp 解释，因此传入前使用 `px2vp`。设备像素检查要求每种面板色覆盖至少屏宽 × 20 个像素，拒绝高密度下只画出一小块角图的假通过。

坏图验收使用 CRC 正确但 zlib 头损坏的首帧。设备解码器会容忍这种损坏并继续播放后续有效帧，因此拆帧器在原生解码前额外检查每帧 zlib 头：deflate 方法、窗口上限、FCHECK 和禁止预设字典，并允许头部跨 PNG 数据分块。像素解压仍属于 Image Kit，不复制完整 deflate 实现，也不声称能独立识别原生解码器接受的所有压缩流损坏。

## Alternatives considered

- 继续只用 Image Kit：原生代码最少，可自动享受设备解码器更新，但无法保证 APNG 动画，且默认图可能不属于动画，不能满足本任务。
- 用 OffscreenCanvas 合成：平台替我们做透明混合，但需要额外图形上下文、快照和释放约定，难以在无设备环境验证 disposal 的像素；使用可在 Node 上测试的有界 RGBA 合成，只保留 PNG 解码在设备层。

## Testing

`bash platforms/harmony/tests/run.sh` 通过完整逻辑回归，其中 APNG 有 57 条断言，既有 canvas 有 41 条断言；覆盖子帧、两种混合、三种 disposal、默认图排除、有限循环、BGRA 转换、跨块 zlib 头及异常输入。远端 macOS 构建机完成独立测试 HAP 与恢复注入后的常规 HAP 编译，最终 `bash scripts/verify-local.sh --quick` 通过。

在 HarmonyOS 6.0.2 / API 22 手机模拟器安装独立 `app.msime.hmos.canvasprobe` 包，四阶段全部通过：浅色动画、深色动画、浅色坏图回退、深色坏图回退。每阶段实际点击原生键盘输入合成拼音 `nihao`，验证候选 `你好`、候选点击写入原生 TextArea、隐藏再聚焦恢复键盘；正常面板红/蓝与挂件绿/黄的 RGB 完全匹配，每种面板色覆盖超过屏宽 × 20 个像素，坏图只有挂件继续动画。测试包不声明原应用共享沙箱，退出恢复原输入法并卸载测试包。验收脚本与重现步骤见 [设备 README](../../../../platforms/harmony/tests/device/README.md)。

## Consequences

APNG 动画不再依赖设备是否支持多帧解码，九宫格裁切和真实 Canvas 更新也有设备像素证据。代价是 Base64 约三分之一的编码增量，以及 UI 线程 RGBA 合成和额外工作缓冲。保持已有 2048 边长、120 帧、800 万累计像素，以及共享加载器每资源 8MiB 的输入限制，按资源剩余预算预检；普通 PNG 不因解析器而多出 1MiB 限制。CRC 用固定查表避免逐字节八轮循环。只在外观资源更换时处理，不在按键路径解码。原生 RGBA 的 alpha 类型和 stride 必须显式校验与转换。

本次证据限于上述手机模拟器和合成资源；物理设备、2in1 桌面窗口、系统减弱动态效果，以及固定 Windows 上游皮肤的完整截图对比仍需单独验收，不据此宣称全平台全部兼容。
