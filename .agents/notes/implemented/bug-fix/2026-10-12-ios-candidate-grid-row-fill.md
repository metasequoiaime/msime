# Agent Note: iOS 展开候选网格填满每行

Status: implemented

## Problem

`KeyboardCandidatePanelView` 的网格照设计稿的 CSS `repeat(auto-fill, minmax(64px, 1fr))` 按列排，长词和带注释的候选占多列；一行剩下的列放不下下一格时换行，剩下的列空在行尾。手机上一行五列、三字词常占两列，「1 + 1 + 2」这样的行右边空一列很常见。Android 修一行只放三格的问题时（`2026-10-12-android-candidate-grid-columns.md`）决定把换行前那一格拉宽到行尾，鸿蒙同日跟进（`2026-10-12-harmony-candidate-grid-row-fill.md`），三端用同一条规则。

## Decision

- 新增 `KeyboardCandidatePanelView.place(spans:columns:)`：按顺序装行，换行条件不变；换行前那一格的实际占列延伸到行尾，最后一行不拉宽。返回每格的行、起始列和实际占列。
- `layoutGrid` 先按 `span(titleWidth:annotated:columns:columnWidth:)` 算出各格占列，再交给 `place` 摆放；格子宽度和行高按实际占列算，所以拉宽的格子按新宽度量高度。整面面板和全拼九键三栏面板的中栏共用这一套。
- Android（`CandidateGridPolicy.place`）和鸿蒙（`ExpandedCandidateLayout.rows`）是同一条规则。

## Alternatives considered

- **保持设计稿的 CSS 网格，不拉宽**：与设计稿完全一致，但行尾空列正是用户嫌弃的地方，而且 Android、鸿蒙已改成拉宽，三端会不一致。
- **把行尾的空列平均分给这一行的各格**：行内更均匀，但各行的格子边缘不再落在同一组列线上。

## Consequences

- 收益：除最后一行外每行都填到右边，与 Android、鸿蒙一致；格子边缘仍落在列线上。
- 代价：行尾那格比它的字宽宽出一截；偏离了设计稿的 CSS 网格。

## Verification

- `CandidateTranslationTests.testExpandedGridFillsEachWrappedRowToTheEdge`：`place` 的行尾拉宽、最后一行不拉宽、整行已满不变；再在 390 pt 宽的真实面板上排「好 ×4 + 一句长句」，第四格右边缘贴住网格右边、长句所在的最后一行不拉宽。
