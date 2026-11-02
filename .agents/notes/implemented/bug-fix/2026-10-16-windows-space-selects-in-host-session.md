# Agent Note: Windows 空格和点选也在 TIP 的宿主会话里选词

Status: implemented

## Problem

用户报告：打字时输入法「把所有拼音都记着」，切成英文之后全部打出来。在 Windows 11 上用 SendInput 往记事本里打字复现（安装包是 develop `435aa8c5` 的本机构建）：

- `nihao` 空格、`shijie` 空格，文档里是「你好世界」，按一下 Shift 变成「你好世界nihaoshijie」。在此之前空格上屏过的拼音也一起出来，越积越长。
- 鼠标点选候选完全一样：点「你好」「世界」，按 Shift 后多出 `nihaoshijie`。
- 分段选词：`nihaoshijie` 用方向键移到第 5 项「你好」再按空格，组字显示「你好nihaoshijie」，正确的应该是「你好shijie」；按 Shift 只剩 `nihaoshijie`，已经选好的「你好」也丢了。
- 数字键选词没有这个问题。

TIP 进程里有自己的宿主会话，生产环境里由它拥有组字：预编辑、Shift（`_HandleToogleIMEMode` → `_HandleHostRawCommit` → `MSIME_COMMIT_RAW`）和回车都读它。数字键走 `_HandleCandidateWorker`，用 `host->select` 在宿主会话里选中高亮候选，宿主会话随之前进。空格走 `_HandleCandidateFinalize`，只把 Server 回的文字写进文档：`Normal` 回复写完就结束 TSF 组字，`NeedToCreateWord`（只选了一段）按旧处理器的写法重建按键缓冲，两条路都没碰宿主会话。点选上屏的分支只对韩文、注音、越南文、藏文这些内联方案丢掉宿主会话的组字。于是宿主会话一直留着每次空格或点选上屏过的拼音。Server 自己的会话每次都清空，候选窗也由它画，所以打字时看不出异常，要等 Shift 或回车读宿主会话时才暴露出来。

这条路是 2026-09-15 的 `d02869152` 留下的：那次让空格在宿主会话拥有组字时读 Server 的回复（之前根本读不到，上屏的是原始拼音），但宿主会话没有跟着前进。

## Decision

- `_HandleCandidateWorker` 里「在宿主会话里选中高亮候选」那一段抽成 `_SelectHighlightedHostCandidate`：候选窗与宿主会话的列表对得上（`MatchesEngineView`）就 `host->select`，选完上屏并结束组字，只选了一段就按宿主会话的结果继续组字；对不上或选择失败返回 `S_FALSE`，什么也不写。数字键和空格都调它。
- `_HandleCandidateFinalize` 读到 Server 的 `Normal` 或 `NeedToCreateWord` 回复时，宿主会话拥有组字就先调 `_SelectHighlightedHostCandidate`，它接手就直接返回。只有它返回 `S_FALSE` 时才照旧处理：`Normal` 写 Server 的文字，然后 `_CancelHostComposition`，让宿主会话里这段原文不会在之后的 Shift 或回车时再出现。日语空格的转换回执、`CommitExactText`（释义）、`TransportUnavailable` 和 `OutofRange` 的处理不变。
- 点选上屏（`pendingCommitCandidate`）的分支对所有宿主会话拥有的组字调用 `_CancelHostComposition`，不再只限内联方案。
- `tsf/tests/input/space_host_selection_wiring.cpp`（`msime-tsf-space-host-selection-wiring`）核对这三处接线，对改动前的源码 7 条断言全部失败；`gloss_column_wiring.cpp` 改为在数字路径里认 `_SelectHighlightedHostCandidate`。

## Alternatives considered

- **空格上屏后只调 `_CancelHostComposition`，照旧写 Server 的文字** — 改动最小，和 `_CommitServerExactText` 的写法一致，整句上屏的情形就此修好。否决：分段选词时组字还要继续，丢掉宿主会话就把剩下的拼音也丢了；而让宿主会话只选这一段，正需要在宿主会话里做一次选择。保留它作为候选窗与宿主会话对不上时的兜底。
- **空格整个改走 `_HandleCandidateWorker`** — 和数字键共用一条入口，最对称。否决：日语的空格是「変換」，Server 回导航回执，`_HandleCandidateFinalize` 据此记下转换已经开始（`JapaneseSpaceReplyKeepsComposition`），工作函数不处理这种回复；`TransportUnavailable` 时要把按键留着重放，工作函数也不认。所以空格保留原来的入口和对回复的分流，只在要上屏或分段选词时交给共用的宿主选词。
- **[数字选词后结束组字](2026-10-11-windows-digit-commit-and-hud-anchor.md) 否决过的反方向：数字改走 `_HandleCandidateFinalize`** — 那篇的理由在这里同样成立：宿主会话拥有组字时，上屏的应该是宿主会话自己选的候选，Server 的回复和宿主会话的候选身份不一定一致。所以这次是空格向数字靠拢，而不是反过来。

## Consequences

- **收益**：空格、数字、点选三种上屏都让宿主会话和 Server 一起前进。Shift 切英文和回车只交出还没上屏的原文；分段选词后组字显示「已选汉字 + 剩余拼音」，Shift 上屏的也是这两部分。
- **代价与已知上限**：空格上屏的文字现在来自宿主会话的选择，而不是 Server 的回复。两个会话的状态一致时两者相同，数字键一直就是这样。候选窗与宿主会话对不上时退回 Server 的文字，宿主会话被丢掉，这种情形下分段选词剩下的拼音不会保留。
- **后续**：点选上屏后来也改为先按文字在宿主会话里跟着选，跟不上才丢掉宿主会话的组字，宿主会话才能接着出上屏后联想，见 [Windows 上屏后联想](../feature/2026-11-02-windows-post-commit-prediction.md)。
- **重访信号**：如果宿主会话和 Server 选出的候选出现分歧（比如一边应用了另一边没有的候选变换），就要重新评估由哪边决定上屏文字。这个信号在 2026-10-22 触发了：两个会话各自学习后同一个下标指向了不同的词，打 `bushang` 时候选窗高亮「补上」而空格和数字键都上屏「不伤」。结论是上屏仍由宿主会话决定（本篇的收益不变），但候选身份改为[按文字在宿主会话里唯一匹配](2026-10-22-windows-host-candidate-identity-by-text.md)，不再套用候选窗带来的下标。

## Verification

- 改动前，在本机安装的 develop `435aa8c5` 上用 SendInput 往记事本里打字（`nihao` 空格 `shijie` 空格、数字 1、点选第一项、分段选「你好」）再按 Shift，结果见上面 Problem 一节。
- 改动后的安装包还没有用同一个脚本重测：提交时安装包正在构建。重测之前，修复只有源码接线检查这一级证据。
- `space_host_selection_wiring.cpp` 用 MSVC `/W4 /WX` 编译，对改动后的源码通过，对改动前的源码失败；`gloss_column_wiring.cpp` 对改动后的源码通过。
