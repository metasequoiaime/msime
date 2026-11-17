# Agent Note: Windows 候选窗启用彩色 Emoji 字体

Status: implemented

## Problem

Windows 候选窗的混输列表会把 Emoji 当作普通单色字形绘制。候选窗使用 Direct2D 的 `DrawText` 和 `DrawTextLayout`，但传入的绘制选项没有包含 `D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT`；Windows 11 的彩色字体因此只能按单色笔刷绘制，用户看到的 Emoji 与系统原生显示不一致。

## Decision

候选窗所有经过 `Windows7EmojiFont` 兼容入口的文本，在内核版本不低于 Windows 8.1 时加入 `D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT`；复用 `WindowsVersion.h` 缓存的真实内核版本，查询失败或旧系统保留调用方原选项。候选窗中为局部字形样式创建的三个 `DrawTextLayout` 调用也使用同一入口，保证普通候选、带局部样式的候选和装饰文字的绘制行为一致。Win7 私有字体回退仍按原路径工作；不改变字体选择、测量或候选排序。

2026-11-17 在 Windows 7 SP1（6.1.7601）复现：同一内存 DC 的 `DrawText` 传原选项时 `EndDraw` 成功，传彩色字体标志时返回 `0x80070057`。不能把旧系统的行为当作「忽略不支持的标志」。新增 `windows-color-font-render` 在真实 Direct2D 内存目标上覆盖普通文本和局部样式布局，以及无选项、裁剪和裁剪加不吸附六种组合，既检查绘制成功，也检查非空字形像素；静态 CRT 让它能独立复制到 Win7 运行。该回归在无条件加标志的版本上以 `0x80070057` 失败，修复后在 Win7 SP1 与 Windows 11 Pro（10.0.26300）上六组全部通过。

## Alternatives considered

- **只在混输 Emoji 的调用点单独加标志。** 最强理由是改动更小，但候选文字、释义、装饰和局部样式各有绘制入口，容易留下同一候选中部分彩色、部分单色的差异，因此没有采用。
- **更换候选字体或强制使用随包 Emoji 字体。** 最强理由是可以统一字形外观，但会改变用户字体设置和字形覆盖范围，且 Windows 11 已有系统彩色字体支持，因此没有采用。

## Consequences

候选窗在支持彩色字体的 Windows 版本上会显示彩色 Emoji；不支持彩色字体的系统仍使用原有单色回退。绘制选项的统一入口增加了一层小的兼容封装，后续新增候选文本绘制应复用它。Windows 11 Pro 的原生 `CandidateWindow` 使用固定合成候选快照验收，恢复旧绘制选项时卡片内彩色像素为 0，启用彩色字体时为 676，改前、改后截图存入 PR；这证明宿主绘制，不替代 TSF 和真实编辑器输入验收。
