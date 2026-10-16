# Agent Note: 外部皮肤预览共用装饰几何与背景透明度

Status: implemented

## Problem

外部皮肤卡片和外观页各自校验装饰带高度、宽度，并在背景图上乘以用户窗口透明度；输出 CSS 几何的函数又独立判断一次装饰带是否有效。分开的判据容易让 DOM 的装饰结构与 CSS 尺寸分叉，也让同一皮肤在两处显示出不同透明度。

## Decision

在 `skin/skin-preview-geometry.ts` 中统一装饰尺寸和背景透明度的纯计算。`skinDecorationGeometry` 使用既有 `dimension` 规则：高度在 `0..500`、宽度在 `0..1000`，必须都是非零有效数值才启用装饰；任一无效时输出两项零尺寸。`skinGeometryStyle`、`ExternalSkinCard` 与 `LoadedPreview` 共用这个结果。

`skinPreviewBackground` 对已经准备好的背景图应用 `candidateOpacityPercent`，先将窗口百分比除以一百，再乘背景透明度，不修改源对象。图片读取、代次、解码失败与重试仍由现有 hook 负责（解码失败归属读取结果，见[图片解码代次](../bug-fix/2026-10-16-skin-preview-decode-generation.md)）；`decorationImage` 对旧清单的回退、CSS 层叠、提示文案和主题解析保持原位置。`dimension` 保留旧模块的导出兼容性，不扩大 UI 包公共导出。

历史检索中，[macOS 候选预览 token](../bug-fix/2026-10-11-macos-candidate-preview-native-tokens.md)持有窗口透明度覆盖规则，部分重叠并互链；[外部扫描代次](../simplification/2026-10-12-external-scan-generation.md)持有读取生命周期，与纯计算无关。Windows 原生预览与 Canvas 导出预览有自己的尺寸及装饰图存在性判据，保留其契约。

## Alternatives considered

- **把这些计算并入 `useSkinPreviewAssets`。** 调用处可以一次拿到渲染模型，但会扩大资源 hook 的输入和返回契约，CSS 几何函数仍要另取相同尺寸；纯模块让三处都能复用，也不牵动读取生命周期。
- **在现有 `external-skins.tsx` 增加共享函数。** 改动路径更少，但纯计算继续依附整份目录和卡片 UI；独立叶子模块只用类型依赖声明清单与背景形状，实际运行依赖仅是现有窗口透明度校验。

## Consequences

三处装饰几何判断与两处背景淡化共用纯计算，合法小数、边界、零、异常值和浮点运算顺序保持一致。代价是新增一个内部叶子模块及类型依赖；旧 `dimension` 导出继续兼容，图片 hook 的输入、返回值和生命周期不变。

纯函数测试覆盖尺寸边界与窗口透明度的默认和非法值；组件测试覆盖两种预览的 CSS、背景淡化、图片失败、刷新和读取次数。背景透明度仍仅由资源 hook 规范化；Canvas 导出预览的装饰规则独立保留。
