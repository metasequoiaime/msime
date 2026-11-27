# Agent Note: 双拼键位图跟随候选窗的「整体大小」

Status: implemented

## Problem

#11616：Windows 11 上双拼键位提示（`ShuangpinKeymapWindow`，候选窗旁边那张 620×203 DIP 的三排键帽浮窗）里的字太小，高分辨率屏幕上「有点看不清」，用户希望能调大。

这张图此前是固定尺寸：卡片 620×203 DIP、方案标题 12pt、键名 11pt Consolas、键下编码 9pt、页脚说明 10pt，全部写死在 `ShuangpinKeymapWindow::paint()` 与 `ShuangpinKeymapLayout.h` 的 `ShuangpinKeymapMetrics` 里。它只随系统 DPI 缩放（`toolbar_pixel_unit`），不受候选窗已有的「整体大小」（`candidate_scale_percent`，Windows 上 75–150%）影响——用户把候选窗调大时，键位图仍是原尺寸。macOS 的 `MSIMEShuangpinKeymapPanel` 同样是固定 620×203。

## Decision

把 `candidate_scale_percent` 作用到键位图上，不新增设置项、不改默认值：

- `ShuangpinKeymapWindow::set_scale(double)` 接收 `/100` 之后的倍数（`CandidateWindowStyle::scale()`），非法值（0、负数、NaN）按 100%。
- 摆放时所有 DIP 乘上这个倍数（`unit = toolbar_pixel_unit(dpi, 1.0) * scale_`），窗口尺寸跟着变大；绘制时在 Direct2D 目标上压一层 `Matrix3x2F::Scale(scale_, scale_)`，卡片、键帽、文字一起放大，几何按 100% 的形状排版，键距和字距不会各自失真。窗口尺寸和绘制都乘在 DPI 之上，所以 150% 在 200% 系统缩放下是 3 倍像素。
- 驱动点在 `DesktopCandidates.cpp` 的 UI 循环里跟着 `settings->style` 一起更新（和 `candidates.set_style` 同一个分支），改「整体大小」不用重启 Server，下一次样式发布就生效。
- 纯计算一侧在 `ShuangpinKeymapLayout.h` 加 `shuangpin_keymap_scale`，把「100% = 1.0、非法值回到 1.0」的规则放进可测的头文件，`windows-shuangpin-keymap-layout` 覆盖它。

只在 Windows 做：issue 是 `platform/windows`，macOS 原生设置的迁移与面板改动另算，它的面板仍是固定尺寸。

## Alternatives considered

- **新增独立的「键位图大小」设置。** 最强理由是用户可能只想放大键位图、不动候选窗。但它要新建共享 Rust 偏好字段、接设置同步、并在六个平台各自补入口，而 `packages/ui` 只修缺陷、新设置要维护者单独授权；「整体大小」本来就是这个界面的缩放项，键位图是候选界面的一部分，跟着它是同一件事。
- **直接把默认字号调大（11/9/10 → 13/11/12）。** 最强理由是不需要任何开关、所有用户立刻看清。但它会改变 macOS 对齐的设计尺寸和卡片比例，老用户的手感无声变化，也没有留出「想更大」的余地。
- **只放大键名、不放大卡片。** 键帽高度 38 DIP、键宽按行内收，字号变大后编码行会和键名重叠，必须一起改几何；整张放大更简单，也和候选窗「字号与窗口尺寸一起缩放」的口径一致。

## Consequences

- 100% 时外观与改动前完全相同（窗口尺寸、坐标、字号都是原值），没有默认手感变化。
- 设了 75–150% 的用户现在看到键位图跟着候选窗一起缩/放大；在 200% DPI 的机器上选 150%，键位图是 3 倍像素，够看清 9pt 的编码行。
- macOS、Android、iOS、HarmonyOS、Linux 的行为不变。
- 键位图与候选卡的间距（8 DIP）、离屏幕边缘的留白（16 DIP）、避让文字行的行高（24 DIP）也随倍数放大，排布不会挤在一起。

## Testing

- `windows-shuangpin-keymap-layout` 新增 `scale_follows_the_candidate_window`：100% 原值、150%/75% 按倍数、0/负数/NaN 回到 1.0。该测试不依赖 Windows 头文件，在本机 clang++ 与 `python3 scripts/test-windows-native-run.py` 里都真跑。
- 两个改动的 Windows 翻译单元（`ShuangpinKeymapWindow.cpp`、`DesktopCandidates.cpp`）用本机 MinGW 以 `-fsyntax-only -DMSIME_EDITION_FULL` 编过。
- 绘制与摆放只能在 Windows 上验收：`Windows native settings`、`Windows GNU cross build` 覆盖编译，150% 下的实际画面需要 Windows 实机截图（见 PR）。
