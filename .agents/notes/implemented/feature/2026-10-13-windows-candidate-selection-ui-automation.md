# Agent Note: Windows 候选窗把高亮的候选报告为 UI Automation 的选中项

Status: implemented

## Problem

[自绘窗口的 UI Automation 读屏](2026-10-10-windows-server-window-ui-automation.md) 让候选窗的每一行成了带 Invoke 的 ListItem，但没有哪一行报告「选中」：没有 SelectionItem 模式、没有 IsSelected，高亮换行时也没有事件。读屏能读出列表里有什么，却不知道空格、回车会上屏的是哪一个；用户用方向键、Tab 移动高亮或继续打字时，读屏什么也不说。

## Decision

- `AccessibleElement::selected`（`AccessibleElements.h`）有值时元素提供 SelectionItem 模式，true 是选中。`candidate_accessible_tree` 给每一行候选设 `selected = candidate.highlighted`；logo、预编辑、页码和翻页按钮没有这个值。工具栏和托盘卡片的树不受影响。
- 树里有带 `selected` 的元素时（`accessible_selectable`），窗口这个片段根另外提供 Selection 模式：单选（`CanSelectMultiple` 为 false）、要求有选中项，`GetSelection` 交出选中的那一行。每一行的 `SelectionContainer` 是这个根。
- 选中项跟着用户在正在打字的应用里按的键走，读屏不能挪它：`Select`、`AddToSelection` 对已经选中的那一行算成功，其余改动选中的请求回答 `UIA_E_INVALIDOPERATION`。点选一行仍是 Invoke，会直接上屏，两者不混用。
- `AccessibleWindow::publish` 在换树时比较前后两棵树的选中项（`accessible_selection_change`）：选中的换了一行，或者同一行的名字变了（继续打字、翻页之后高亮下面是另一个候选），就在 `StructureChangeType_ChildrenInvalidated` 之后对新选中的那一行发 `UIA_SelectionItem_ElementSelectedEventId`；新树没有选中项（候选窗收起）不发。和结构事件一样，只在有读屏在听、并且已经交出过根时才发。

## Alternatives considered

- **高亮变化时发焦点事件** — 读屏对焦点变化的支持最普遍，几乎都会念出来。仍然不用，理由同上一篇：这些窗口不接受焦点，焦点事件可能把读屏从正在打字的编辑框拉走。ElementSelected 不移动焦点。
- **只报告 IsSelected、不发事件** — 不会多念，避开和 TSF `ITfCandidateListUIElement` 那条路重复播报的可能。没有采用：读屏不会自己轮询一个不接受焦点的窗口，没有事件就等于高亮移动时什么也不说，只解决了一半。
- **只在选中的那一行换了（id 变了）时发事件** — 打字时不会每个键都播报。没有采用：打字时高亮通常一直在第一行，按 id 比较的话读屏永远不会念出新的首选，而那正是空格要上屏的内容。

## Consequences

- **收益**：读屏能问出当前高亮的是哪一个候选，高亮移动、继续打字、翻页之后都会念出现在选中的那一行。
- **代价与已知上限**：没有在真实的讲述人、NVDA 上验证。要在真机上确认的是：讲述人是否会对这个 `WS_EX_NOACTIVATE` 窗口里的 ElementSelected 播报；如果它同时经 TSF 的候选 UIElement 读候选，同一个候选会不会被念两遍；每个按键都换首选时播报是否过于频繁。出现重复播报时，要么在这里节流，要么改成只在高亮换行时发。

## Verification

- `platforms/windows/tests/ui/accessible_elements.cpp`（`windows-accessible-elements`）：带选中项的树才提供 Selection 模式；高亮换行、同一行换了候选时报告新选中的那一行，只是预编辑变了、候选窗收起时不报。
- `platforms/windows/tests/candidate/candidate_accessibility.cpp`（`windows-candidate-accessibility`）：高亮行 `selected` 为 true、其他行为 false，非候选元素没有这个值；高亮移到第二行时选中项跟着换。改动 `CandidateAccessibility.h` 之前这条断言失败。
- 这两个测试在宿主机上用 `clang++ -std=c++17 -Wall -Wextra -Werror -DMSIME_EDITION_FULL` 编译并通过，也在 Wine 下通过；`AccessibleWindow.cpp` 随 Server 用 MinGW x64 交叉编译通过。
