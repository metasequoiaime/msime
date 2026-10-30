# Agent Note: Android 剪贴板单条删除也能撤销

Status: implemented

## Problem

#10080：Android 键盘剪贴板面板里，多选删除后可以在撤销时长内撤销（[多选删除](2026-10-17-clipboard-multi-select.md)、[浮动撤销与撤销时长](2026-10-29-android-clipboard-undo-floating.md)），单条删除却是直接删掉的：长按一条后操作行里的「删除」、左滑露出的垃圾桶，点了就没了，误点无法后悔。[左滑删除](2026-10-09-android-clipboard-swipe-delete.md)当初定为两步、不做撤销，理由之一正是「长按删除和清空也都没有撤销」；多选删除有了撤销之后，这条理由只剩清空还成立。

报告者另提「撤销键增粗、增大」，和 #10073 的右下角大号浮动按钮是同一个诉求，已由那次处理，这里不改按钮的样式。多选删除和撤销只有 Android 有（iOS、鸿蒙的剪贴板面板没有 `removeMany` / 撤销），这次只改 Android。

## Decision

- **单条删除也走共享存储的 `remove_many`。** `MSIMEInputService.removeClipboardItem`（两个入口都调它）把这一条的文字包成单元素列表交给 `ClipboardHistoryStore.removeMany`，拿回删除前的完整条目（文字、时间戳、固定状态、分类）。撤销沿用 `undoClipboardItems` → `restore_many`，恢复后位置、时间戳和固定状态都和删除前一样，原来的分类还在时也归回原分类。共享层单条的 `remove` 只回答删没删，不交回快照。
- **撤销的记账收进 `MSIMEInputService.holdClipboardUndo`，单条和多选共用。** 它保存快照、撤掉上一次没执行的倒数回调、按删除那一刻的「撤销时长」算截止时刻并投递 `clipboardUndoTick`，再提示「已删除 N 项，可在 N 秒内撤销」，N 是实际删掉的条数。之后的浮动「撤销」、每秒倒数、到期收起、显示条件（`clipboardUndoAllowed`）和列表底部让位全部沿用 #10073，单条删除后面板重画到本机常态，按钮自然出现。
- **只留最近一次。** 新的删除（不论单条还是多选）替换还没到期的上一次，撤销只恢复最近一次删除，倒数从头开始。
- **什么都没删掉时不碰上一次的撤销。** `removeMany` 返回空（那条已经在别处被删掉）时 `holdClipboardUndo` 直接返回，上一次删除的快照和倒数照旧。原来多选路径会把快照先换成空列表，上一次删除的撤销跟着丢掉，按钮还要等到下一次倒数才收起。
- 设置页「撤销时长」的说明从「多选删除后，…」改为「删除记录后，剪贴板面板右下角的「撤销」按钮保留几秒」，设置键、取值和默认值不变。
- `check-host.sh` 守住 `removeClipboardItem` 里的 `holdClipboardUndo(clipboardHistory.removeMany(`：改回单条 `remove` 时撤销按钮不会出现，这条检查会失败。

## Alternatives considered

- **单条删除保留 `remove`，删除前先从 `load()` 的结果里找出这一条留作快照**：不用改删除调用。但读和删是两次加锁，中间设置页或另一个进程改过历史时，快照和实际删掉的条目可能对不上；`remove_many` 在同一次加锁里取快照、删除、落盘，多选那次就是为此加的这个契约。
- **连续的单条删除累加进同一份快照，一次撤销全部恢复**：连删几条后可以一并找回。但和多选删除「只撤最近一次」的语义不一致，倒数从哪一次删除算起也说不清；误删通常是刚才那一下，要整批找回时可以先多选再删。

## Consequences

- **收益**：两个单条删除入口都能后悔，恢复后时间戳和固定状态不变；单条、多选的撤销是同一个按钮、同一个时长、同一套到期规则。
- **代价**：每删一条都会弹一次提示、浮出「撤销」，连续逐条清理时右下角的按钮一直在，列表底部一直多让出 56dp；左滑删除和长按删除少了「删了就是删了」的确定感。撤销仍只在当前输入法进程内有效，进程退出后不保留待恢复的数据。
- 清空历史仍然没有撤销，它有就地的确认行。

## Verification

`bash platforms/android/check-host.sh` 编译服务和设备套件源码，跑 JVM smoke（其中 `ClipboardUndoPolicySmoke` 覆盖时长和倒数），并检查 `removeClipboardItem` 仍经 `removeMany` 交给 `holdClipboardUndo`。

`ToolbarEdgesDeviceSmoke.checkClipboardSingleDeleteUndo` 在 30 条合成记录上先把一条固定，再从真实渲染出的控件点下去：按 `clipboardSwipedText` 画出打开着的那一格，点可见的垃圾桶，记录被删、浮动「撤销」出现，点撤销后文字、时间戳和固定状态与删除前相同、按钮收起；按 `clipboardActionText` 画出操作行，点其中的「删除」，同样能撤销恢复；再删一次、把截止时刻改到过去走一次倒数，按钮收起、记录没有回来。这条设备检查没有在模拟器或真机上跑过，真机上左滑和长按两条路径的实际手感也没有确认。
