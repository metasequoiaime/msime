# Agent Note: 三个移动端统一并扩大触屏手写区

Status: implemented

## Problem

维护者在 iPhone XR（414×896 pt）上反馈手写模式的书写区太小：左侧四个标点键、右侧 ⌫ 与「重写」把一行分成 .7 / 3 / .8，书写卡片只占一行约三分之二，卡片里的虚线辅助框还要再内缩 14 pt，实际看到的书写框只有约 236×150 pt。HarmonyOS 在 [离线手写识别](2026-10-25-harmony-handwriting-input.md) 里已经把份额改成 .6 / 4 / .7，iOS 与 Android 仍停在旧比例，三端的手写键盘不一致。

## Decision

iOS、Android、HarmonyOS 的触屏手写区使用同一组行宽份额：标点列 .6、书写区 4、⌫ 与「重写」列 .7，书写区约占一行的 75%。键盘总高、底行、候选条都不变，书写区高度仍等于三行键区。

- iOS：`KeyboardViewController.handwritingPunctuationShare / handwritingCardShare / handwritingToolsShare`。
- Android：`KeyboardGeometry.HANDWRITING_PUNCTUATION_WEIGHT / HANDWRITING_AREA_WEIGHT / HANDWRITING_TOOLS_WEIGHT`，`ImeLayoutRows.rebuildHandwritingRows` 引用它们，不再写死数字。
- HarmonyOS：`KeyboardView.ets` 的 `HANDWRITING_*_WEIGHT`，数值沿用 #9630。

虚线辅助框距卡片边缘从设计稿的 14 收窄到 8（iOS `HandwritingCanvasCard.guideInset`、HarmonyOS `HANDWRITING_GUIDE_INSET_VP`）。Android 的书写卡片只画十字线、没有虚线框，卡片本身距区域边缘 3 dp，不受这一项影响。

`scripts/test-mobile-handwriting-pad-parity.py` 比对三端份额与两端内缩，并要求书写区不少于一行的 74%；任何一端单独改动都会让契约检查失败。

## 坐标

放大不影响识别坐标。iOS 与 Android 的 ML Kit 收到的是画布坐标，`WritingArea` 取画布实际宽高；HarmonyOS 按实际画布尺寸记录笔画，再映射到引擎固定的 420×420。三端都不假设旧尺寸。

## 测量

| 平台与宽度 | 改前 | 改后 |
| --- | --- | --- |
| iOS 414 pt 卡片 / 虚线框 | 264×178 / 236×150 | 298.5×178 / 282.5×162 |
| iOS 320 pt 卡片 / 虚线框 | 201.5×178 / 173.5×150 | 228×178 / 212×162 |
| Android 411 dp 书写卡片 | 264×171 | 300×171 |
| Android 360 dp 书写卡片 | 230×172 | 261×172 |

iOS 数据来自模拟器 XCTest 中真实 `KeyboardViewController` 的布局；Android 来自 API 35 模拟器上 uiautomator 读到的画布边界（卡片为画布减去两侧各 3 dp）。320 pt 时 iOS 标点键约 34 pt 宽，360 dp 时 Android 标点键约 34 dp 宽，仍不窄于 26 键字母键。

## Alternatives considered

- 只把 iOS、Android 改成 HarmonyOS 的 .6 / 4 / .7，保留 14 的内缩：改动最小，三端立即一致；但 iOS 虚线框只从 236 pt 宽到约 270 pt，高度不变，面积只增加约 15%，达不到「明显变大」。
- 把书写区向上延伸进候选条：手写模式的识别结果就显示在候选条里，占用它会让候选无处显示。
- 加高手写模式的键盘：会让切换方案时键盘总高跳动，HarmonyOS 也没有这样做。
- 进一步收窄两侧列（如 .5 / 4 / .6）：320 pt 时标点键会窄到约 29 pt，接近误触边界，收益只有十几 pt。

## Consequences

收益：iOS 虚线框面积在 414 pt 宽时增加约 29%，320 pt 时增加约 32%；Android 书写卡片宽度增加约 13%–14%；三端比例由契约检查锁定。代价：两侧标点、删除、重写键变窄约 30%，在窄屏上仍能按到但余量变小；虚线框不再严格符合设计稿的 14 内缩。HarmonyOS 只改了内缩常量，ArkTS 编译与真机外观依赖 CI 的 HAP 构建和后续真机验收。
