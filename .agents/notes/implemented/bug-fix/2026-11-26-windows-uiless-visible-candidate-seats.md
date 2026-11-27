# Agent Note: UI-less 候选页与数字键共用「看得见的行」规则

Status: implemented

## Problem

#11834：Windows 版在《魔兽世界》里用双拼，敲 2 或 4 个字母后候选的第二项永远是一个无法识别的方框，按 `2` 上屏的却是第 3 项、按 `3` 上屏第 4 项。

这个偏移来自两条路径对「第几行」算得不一样：

- 数字键由 Server 处理，`ServerSession.cpp` 和 `ReplyComposer.cpp` 都经 `visible_page_candidate`（`platforms/windows/common/CandidateSources.h`）按候选窗上看得到的行选词，会跳过引擎里的 `CandidateSource::CloudPending` 占位席位。云请求在途时引擎在下标 1 放这一行（文字是「☁」），候选窗不画它。
- UI-less 宿主由游戏自己画候选页。Server 侧拼这一页的 `UiLessCandidateSync::updated`（`platforms/windows/src/ipc/UiLessCandidateSync.h`）把整份 `view.candidates` 原样转成文字页发给 TIP，没有按来源跳过占位席位；TIP 的 `_ReplaceCandidateListFromPage` 再按逗号切开交给宿主。于是游戏画出的第 2 行就是「☁」，在游戏字体下成了缺字方框，而它后面每一行都比数字键能选到的位置大一位。

候选窗路径从 #9273 起就隐藏这一行，只有 UI-less 这一侧漏了同一条规则；报告里的「方框在第 2 位、按 2 出第 3 项」与它完全对上，所以按这条修，不先猜游戏侧的字体或输入模式。

## Decision

- `UiLessCandidateSync::updated` 按 `candidate_source_is_visible` 过滤候选，与候选窗、数字键、二三候选键共用同一份判定；页里只放看得见行的文字。
- 高亮所在的引擎行被过滤掉时（方向键理论上不会停在那里，`leave_hidden_seat` 会绕开）这一帧不发：宿主没有这一行可画，保留上一页比发一个没有选中项的页更稳。高亮落在可见行时，`selection` 用过滤后的页内位置，与数字键数的一致。
- 整页只剩占位席位时发空页，宿主清掉旧列表；这与「没有候选项」在宿主侧的表现相同，不会留下一个选不中的格子。

## Alternatives considered

- **在 TIP 的 `_ReplaceCandidateListFromPage` 里过滤**：这一页进了 TIP 只剩文字，来源已经在 Server 侧被丢掉，TIP 分不出「☁」是占位行还是一个真的候选。
- **让数字键按引擎下标选，跟着宿主画的列表走**：会把 #9273、#9849、#10714 那条「候选窗显示第 N 行就是第 N 个候选」的规则退回去，而且候选窗本来就不画占位行，两边仍然各说各的。
- **只把「☁」在游戏字体下换成可见字形**：方框没了，按 2 仍然上屏第 3 项，偏移还在。

## Consequences

- 游戏等 UI-less 宿主里，云候选请求在途时候选页不再有占位行，编号与数字键一致。
- 候选窗、数字键、`;`/`'`、方向键和 UI-less 页面现在都按 `candidate_source_is_visible` 这一条规则；以后新增占位来源只需要改这个谓词。
- 未在报告者的游戏里复现：本机没有 Windows 主机，也没有《魔兽世界》客户端，无法确认他的第二项确实就是「☁」，也没有验证游戏字体。修复按代码里可证实的编号偏移落地；合并后请报告者用开启云候选的最新构建复验，并把第二项的文字（不是截图里的方框）回报一次。

## Verification

- `platforms/windows/tests/core/uiless_candidates.cpp` 新增一组用例，覆盖「占位席位不进页面」「高亮在占位席位时不发帧」「高亮在占位席位后面时 selection 是过滤后的 1 而不是引擎下标 2」「整页只剩占位时发空页」。
- 用 `x86_64-w64-mingw32-g++` 和 `i686-w64-mingw32-g++`，带 CMake 的 `-Wall -Wextra -Werror -DMSIME_EDITION_FULL` 编译 `windows-uiless-candidates`（`uiless_candidates.cpp` + `ReplyCodec.cpp`），两个架构都无警告通过。
- 未运行：`windows-uiless-candidates` 是 Windows 原生测试，本机没有 Windows 主机也没有 Wine，测试只编译未执行；CI 的 Windows GNU 交叉作业同样只编译这些测试。游戏内候选页与数字键的实际对齐需要报告者在 Windows 上复验。
