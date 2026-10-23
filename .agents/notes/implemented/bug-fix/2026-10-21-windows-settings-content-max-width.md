# Agent Note: Windows 设置窗口的正文列限宽 900，不再跟随 macOS 的「不限宽」

Status: implemented

## Problem

把设置窗口拉宽（Windows 上最常见的做法是直接最大化），正文跟着无限变宽：1700 × 900 下一行里「输入语言」的标签贴着最左、下拉框贴着最右，中间一千多 DIP 全是空白；「中文方案」的六个单选项被拉成每个四百多 DIP 的大块，里面只有「全拼」两个字。

来历不是疏漏，是跟随：共享样式 `packages/ui/src/settings/settings-style.ts` 的 `contentColumn` 给 Windows 和 macOS 的都是 `max-w-none`，只有 Linux（680）、HarmonyOS 2-in-1（760）、iPad（720）和默认值（900）限宽。[照 macOS 密度重做这个窗口](../feature/2026-10-20-windows-settings-mac-density.md)时，正文边距 24/48/44 连同「不限宽」一起搬了过来，而那份规格是按 macOS 定的——macOS 用户很少最大化设置窗口，Windows 用户经常。

## Decision

正文列（`render_page_at` 里那个 `page`）取 `MaxWidth(900)` 并水平居中。900 是共享样式自己的基准值，不是新拍的数。

初始窗口是 1000 × 780，侧栏常驻 270 DIP，正文只有 730 DIP，**never** 触到这个上限：默认尺寸下的版面一个像素都不变，只有窗口拉到 1200 DIP 以上才开始把正文居中、两侧留白。最窄端不受影响，那里由 `PreferredMinimumWidth(800)` 兜底。

这是 Windows 原生设置窗口单方面偏离共享样式的 `win:max-w-none`；共享页（Tauri 外壳）和 macOS 都没有跟着改。

## Alternatives considered

- **把窗口做成不可调整大小** — 提出这条的直觉是对的：固定尺寸确实不会有超宽排版。否决：它把「某些尺寸下难看」换成了「某些机器上不好用」——1366 × 768 的笔记本上 1000 × 780 几乎占满屏幕而用户不能缩小，系统文字放大后内容放不下也不能拉大；Windows 设置、Office 选项以及这个窗口对标的 macOS 设置全都可调整。
- **改共享样式里 `contentColumn` 的 `win:` 分支** — 一处改动覆盖原生窗口和 Tauri 外壳，不留分歧。否决：那会同时改变共享设置页在 Windows 上的所有场合（面板、嵌入式设置），影响面比这个窗口大得多，该由共享 UI 单独评估；本条只管原生窗口。
- **正文左对齐而不是居中** — 视线从左开始，和侧栏连续。否决：Windows 设置应用与 macOS 系统设置在宽窗口下都把正文列居中，居中也不会让侧栏与正文之间出现一道过宽的空白。
- **维持现状** — 与共享设计完全一致。否决：最大化是 Windows 上的常态，一行里标签与控件相隔一千多 DIP 已经影响可读。

## Consequences

- **收益**：最大化或宽屏下正文保持可读宽度并居中；默认尺寸下版面不变。
- **代价**：这一项上 Windows 原生窗口与共享样式的 `win:max-w-none` 不再一致，读共享样式的人不会从那里看出 Windows 的实际上限。
- **重访信号**：共享样式给 `contentColumn` 的 `win:` 分支加了具体的 `max-width`，或 macOS 改掉自己的 `mac:max-w-none` 时，这里的上限要跟着对齐。
- **本篇的实现是错的，已被取代**：`MaxWidth` 与同时写上的 `HorizontalAlignment::Center` 互相抵消，居中让面板按自然宽度排版、不再拉伸，900 这个上限从未生效（实际宽度约 870，肉眼难分，所以当时的验证看上去是通过的）。限宽这个决定仍然成立，实现与取值改为[靠内边距实现、上限 1800](2026-10-23-windows-settings-width-by-padding.md)。
- **不在范围内**：最窄的 800 × 560 下「输入」页六个标签的文字会折成两行。那是 `segmented_control` 既有的取舍（`main.cpp` 注释记着「与共享页按钮里的字折行相同」），本条没有改动它。

## Verification

Windows 11、Windows App SDK 1.7.250606001，`RelWithDebInfo` 构建后打开真实窗口对照三档：1700 × 900 下正文收在 900 并居中，一行的标签与控件不再分列两端，六个单选项恢复常规宽度；1000 × 780 与改动前逐项一致；800 × 560 下与改动前一致。
