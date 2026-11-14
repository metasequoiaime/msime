# Agent Note: Windows 候选窗启用彩色 Emoji 字体

Status: implemented

## Problem

Windows 候选窗的混输列表会把 Emoji 当作普通单色字形绘制。候选窗使用 Direct2D 的 `DrawText` 和 `DrawTextLayout`，但传入的绘制选项没有包含 `D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT`；Windows 11 的彩色字体因此只能按单色笔刷绘制，用户看到的 Emoji 与系统原生显示不一致。

## Decision

候选窗所有经过 `Windows7EmojiFont` 兼容入口的文本都自动加入 `D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT`。候选窗中为局部字形样式创建的三个 `DrawTextLayout` 调用也使用同一入口补上标志，保证普通候选、带局部样式的候选和装饰文字的绘制行为一致。Win7 私有字体回退仍按原路径工作；该标志只改变支持彩色字体的 Direct2D 绘制，不改变字体选择、测量或候选排序。

## Alternatives considered

- **只在混输 Emoji 的调用点单独加标志。** 最强理由是改动更小，但候选文字、释义、装饰和局部样式各有绘制入口，容易留下同一候选中部分彩色、部分单色的差异，因此没有采用。
- **更换候选字体或强制使用随包 Emoji 字体。** 最强理由是可以统一字形外观，但会改变用户字体设置和字形覆盖范围，且 Windows 11 已有系统彩色字体支持，因此没有采用。

## Consequences

候选窗在支持彩色字体的 Windows 版本上会显示彩色 Emoji；不支持彩色字体的系统仍使用原有单色回退。绘制选项的统一入口增加了一层小的兼容封装，后续新增候选文本绘制应复用它。新增 Windows UI 测试检查该入口始终携带彩色字体标志；真实 Windows 11 设备验收仍需在 PR 上完成。
