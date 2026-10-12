# Agent Note: Android 展开候选网格按列占位并填满每行

Status: implemented

## Problem

Android 展开候选面板在手机上每行只放三格，右边空出将近一列宽（用户截图：拼音 `youdian`，深色皮肤）。原因有两层。一是格子用的是 `Button` 默认样式，它自带 88 dp 的 TextView `minWidth`；`ImeCandidates.expandedCandidateButton` 调的 `ViewPolicy.setMinimumSize(button, 64dp, 44dp)` 走的是 `View` 重载，只改了 View 级的最小宽度，盖不掉 TextView 的那一份，于是「优点」和「有点意思」都是 88 dp。二是 `CandidateWrapLayout` 是按字宽自由换行的流式布局，放不下下一格就换行，行尾剩多少空白都不管。iOS（`KeyboardCandidatePanelView`）和鸿蒙（`ExpandedCandidateLayout`）一直按列排：每列至少 64、间隔 6，长词按需占多列，只有 Android 不一样。

## Decision

- `CandidateWrapLayout` 改成列网格，算法在纯 Java 的 `CandidateGridPolicy`，由 `CandidateGridPolicySmoke` 覆盖。列数、占列数与 iOS、鸿蒙相同：可见宽度按每列至少 64 dp、列间 6 dp 等分，候选按自然宽度占 `ceil((宽 + 间隔) / (列宽 + 间隔))` 列，不超过一行；行的分配沿用 `CandidateWrapPolicy.rows`（列数当宽度、间隔为 0），与鸿蒙相同。
- 一行剩下的列放不下下一格时，换行前那一格拉宽到行尾，网格右边不留空。设计稿的 CSS 网格（`repeat(auto-fill, minmax(64px, 1fr))`）不拉宽，iOS 和鸿蒙原先也不拉宽；它们改成同一条规则的 PR 与本篇同日（`2026-10-12-harmony-candidate-grid-row-fill.md` 及 iOS 的对应改动）。最后一行不拉宽，一两个候选不会被撑成一整条。分批追加（见 [展开候选网格分批建格子](2026-10-09-android-candidate-grid-batches.md)）后原来的最后一行变成中间行，下次布局时才拉宽，这是预期行为。
- 列的左边缘按 `column * (可用宽 + 间隔) / 列数` 取整，除不尽的像素摊进各列，最后一列和拉宽的格子都正好贴住右边缘。
- 同一行的格子等高：先按各自高度量一遍，矮的再按行高精确量一次。
- 格子清掉 TextView 的 `minWidth`（`ViewPolicy.clearMinimumWidth` 的 `TextView` 重载），宽度只由所占的列决定；最小高度仍为 44 dp。
- 两处网格都受影响：整面展开面板和全拼九键三栏面板的中栏，都经 `ImeCandidateGrid` 建 `CandidateWrapLayout`。

## Alternatives considered

- **只清掉 88 dp 的 `minWidth`，保留流式布局**：改动最小，格子会按字宽变窄、一行放得更多，但每行右边仍剩一截长短不一的空白，与 iOS、鸿蒙的列对齐也不一致。
- **照搬 iOS、鸿蒙原先的做法，不拉宽行尾**：与设计稿完全一致。但 360 dp 手机上五列、三字词占两列时，「1+1+2」这样的行常剩一列空着，用户这次抱怨的正是右边的空白。拉宽只改一行最后一格的宽度，不改列数和换行位置，所以三端改成同一条规则，而不是只有 Android 拉宽。
- **每行把剩余列平均分给各格**：行内格子更均匀，但不同行的格子边缘不再落在同一组列线上，看起来比拉宽最后一格更乱。

## Consequences

- 收益：手机竖屏一行能放五列，长词占两列，每行（除最后一行）都填到右边；格子边缘落在同一组列线上。九键三栏面板的中栏得到同样的排版。
- 代价：三字词在 64 dp 列宽下要占两列，看起来比按字宽排更宽；行尾那格会比它的字宽宽出一截。测量时每格最多量三次（自然宽度、列宽、行高），批大小是 48 格，量的成本可以忽略。
- 带释义的多行格子仍是 44 dp 高，与改动前一样，这次没有改。

## Verification

- `ANDROID_SDK_ROOT=~/Library/Android/sdk bash platforms/android/check-host.sh`：通过，224 个 JVM 冒烟，含新增的 `CandidateGridPolicySmoke`（列数、余数摊分、占列、行尾拉宽、最后一行不拉宽、非法输入）。
- 未在模拟器或真机上看过：本机唯一的模拟器 `emulator-5580` 正被另一个 worktree 的设备套件占用，本 worktree 也没有原生库构建。合并前要用 `build-apk.sh` 出包，在 API 35 模拟器上展开候选看一眼排版，并跑 `smoke.sh` 里的 `CandidatePanelDeviceSmoke`、`NineKeyPanelDeviceSmoke`。
