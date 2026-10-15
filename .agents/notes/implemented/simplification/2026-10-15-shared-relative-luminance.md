# Agent Note: 共享相对亮度计算

Status: implemented

## Problem

触屏皮肤的 `skinLuminance`、候选自定义选中底色和 macOS 预览的文字推导分别实现相同的 sRGB 线性化与相对亮度公式。公式重复容易漂移，且键盘颜色工具为计算亮度而依赖包含皮肤预设和设计规则的模块。

## Decision

在 `core/color.ts` 提供接受 `0xRRGGBB` 数值的 `relativeLuminance`。三个公式和键盘颜色工具共用它；皮肤模块保留 `skinLuminance` 导出别名。颜色字符串解析和校验、alpha 处理及 0.179、0.45 两种阈值仍由原调用方持有。

## Alternatives considered

- 直接让主题导入皮肤的 `skinLuminance`：代码量更小，但主题的基础颜色计算会依赖键盘设计模块，层级不合适。
- 同时共享文字颜色选择器：可以再少几个分支，但键盘与候选的大小写输出、macOS 的深色文字值、透明度回退和阈值不同，会把平台规则塞进通用颜色方法。

## Verification

公共函数覆盖黑白、RGB 三原色、灰色与线性化分界两侧；原皮肤接口和对比度、0.179 阈值两侧、macOS 0.45 阈值两侧、半透明回退和 alpha 不混入 RGB 都有测试。现有候选与 Rust 调色板对照、macOS 原生 token 对照、皮肤预设和候选窗口样式测试继续覆盖调用方。类型检查、生产构建、lint、格式、笔记和 quick 门禁作为提交前验证。

## Consequences

相对亮度公式只维护一份，键盘颜色工具不再为基础运算引入整份皮肤设计模块。公共方法接受 `0xRRGGBB`，不承担字符串校验或透明度合成；颜色字符串调用方继续规范化并只解析前六位 RGB。0.179 与 0.45 两种阈值、文字颜色的大小写、macOS 的深色文字值和透明度回退仍由原调用方决定。皮肤模块保留 `skinLuminance` 导出别名，既有调用不变。本片验证共享 UI，不替代原生宿主验收。

## Existing notes audit

按 `skinLuminance`、相对亮度、sRGB 和对比度检索活跃笔记。[macOS 原生预览 token 笔记](../bug-fix/2026-10-11-macos-candidate-preview-native-tokens.md)与本片部分重叠，继续持有透明度与 0.45 阈值的规则；本片只收拢纯数值公式。Windows 原生 UI、鸿蒙功能键笔记描述各宿主配色，不改变其实现。
