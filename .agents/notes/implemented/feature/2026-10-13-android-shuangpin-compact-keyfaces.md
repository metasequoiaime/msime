# Agent Note: Android 九键双拼收简键面提示

Status: implemented

## Problem

九键双拼把每格的声母和韵母表同时铺在四行网格里，数字与小字挤在同一行。用户反馈满屏字母、难看。[九键双拼](../../implemented/feature/2026-10-10-nine-key-shuangpin.md)定义的键位和输入行为仍然成立，这里只收简触摸键面的提示。

历史检索中，九键双拼笔记与此部分重叠，双方互链；[九键字号适配](../../implemented/bug-fix/2026-10-13-android-nine-key-label-fit.md)主要约束拼音九键，其双拼键面引用同步到本篇。

## Decision

每键主标签采用 22sp 数字，下一行用 11sp 完整提示声母项，零声母编码从这个键开始时前面加「零」；只有韵母的键显示「韵母」。完整声韵母表仍在设置编辑页与无障碍说明中提供。输入、分词、0 组码、方案切换继续使用引擎映射。

## Alternatives considered

- **把声母提示限制为三项并加省略号**：可约束每格的小字宽度，但默认 9 键的 `z` 会被藏成省略号，用户无法从键面判断完整声母映射，因此不采用。
- **保留完整声韵母表，仅缩小文字**：初学者可在键盘上查全部映射。但四行布局空间不足，更小的文字会继续降低可读性。
- **键帽只显示数字**：视觉最简洁，但会失去寻找声母键的提示。保留一行简短提示，兼顾辨认与留白。

## Consequences

数字主标签更易辨认，四行网格不再铺满声韵母表。韵母映射不再直接全铺在键面上，学习时需要查看设置中的表；输入行为和完整无障碍说明保持原有契约。

## Verification

`NineKeyShuangpinKeypadSmoke` 检查简短声母提示、韵母类别、四项声母不被省略，9 键完整显示 `w x y z`，以及完整映射说明。`build-apk.sh`、`scripts/verify-local.sh --quick` 和 `pnpm run verify-notes` 通过。API 35 专用模拟器的截图确认数字独占主行、9 键完整显示 `w x y z`。`NineKeyShuangpinDeviceSmoke` 验证两位组码、0 组码、分词和方案切换通过。Android 12 真机覆盖安装返回 `Success`，未在真机执行输入验收。
