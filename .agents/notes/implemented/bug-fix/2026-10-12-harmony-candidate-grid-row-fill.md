# Agent Note: 鸿蒙展开候选网格填满每行

Status: implemented

## Problem

鸿蒙的展开候选网格按列排（每列至少 64 vp、间隔 6 vp，长词按需占多列），行由 `CandidateWrapPolicy.rows` 分配，各行 `justifyContent(FlexAlign.Start)` 靠左。一行剩下的列放不下下一格时换行，剩下的列就空在行尾。手机上一行五列、三字词常占两列，「1 + 1 + 2」这样的行右边空一列很常见。Android 修一行只放三格的问题时（`2026-10-12-android-candidate-grid-columns.md`，与本篇同日的 Android PR）决定把换行前那一格拉宽到行尾，三端要用同一条规则。

## Decision

- `ExpandedCandidateLayout.rows(indices, spans, columns)` 负责装行：行的分配仍是 `CandidateWrapPolicy.rows(columns, 0, spans)`，换行前那一格的实际占列延伸到行尾，最后一行不拉宽。返回 `ExpandedCandidateCell { index, span }` 的二维数组。
- `KeyboardView.expandedRows` 改用它，格子宽度按实际占列算（`expandedCandidateWidth(span)`），整面网格和全拼九键三栏面板的中栏共用这一套。
- `ForEach` 的格子键加上占列数：行尾那格的宽度取决于后面的候选，键不变时 ArkUI 会复用旧格子。
- iOS（`KeyboardCandidatePanelView.place`）和 Android（`CandidateGridPolicy.place`）是同一条规则。

## Alternatives considered

- **保持设计稿的 CSS 网格，不拉宽**：与设计稿 `repeat(auto-fill, minmax(64px, 1fr))` 完全一致，但行尾空列正是用户嫌弃的地方，而且 Android 已经改成拉宽，三端会不一致。
- **把行尾的空列平均分给这一行的各格**：行内更均匀，但各行的格子边缘不再落在同一组列线上。

## Related decisions

[鸿蒙离线候选释义即时显示](2026-10-24-harmony-immediate-candidate-glosses.md) 对候选条采用相同的刷新键约束，另行负责本地查词与在线请求的时序。

## Consequences

- 收益：除最后一行外每行都填到右边，与 Android、iOS 一致；格子边缘仍落在列线上。
- 代价：行尾那格比它的字宽宽出一截；偏离了设计稿的 CSS 网格。

## Verification

- `scripts/test-harmony-expanded-candidates.py` 检查绘制宽度使用实际的 `cell.span`、文字读取 `cell.index`；单行、截断与可见宽度上限约束仍然保留。

- `bash platforms/harmony/tests/run.sh`：通过，`expanded candidates stay on one line and inside their row` 组新增行尾拉宽、最后一行不拉宽、整行已满不变、与 `CandidateWrapPolicy.rows` 的行分配一致的断言。
- `stage-settings.sh`、`ohpm install`、`hvigorw assembleHap --no-daemon`：`CompileArkTS` 通过，出包成功，改动的行没有新增 ArkTS 警告。
- 未在鸿蒙模拟器或真机上看过排版。
