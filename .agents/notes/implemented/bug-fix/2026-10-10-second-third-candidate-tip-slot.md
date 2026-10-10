# Agent Note: 二三候选在 TIP 中的选词位置和组字判断

Status: implemented

## Problem

[二三候选](../feature/2026-10-09-second-third-candidate-keys.md)让 TIP 把 `;` 和 `'` 归类为数字选词（`FUNCTION_SELECT_BY_NUMBER`），再由 `_HandleCandidateSelectByNumber` 在本地 presenter 里选中一项。TIP 的 presenter 有两种形态，选中的那一项在两种形态下的用途不同：

- **宿主会话的真实一页**：runtime-options.json 准备好之后，TIP 自己的宿主会话拥有组字，`GetCandidateList` 把宿主会话当前页的候选连同身份（`_EngineSession`、`_EngineGeneration`、`_EngineIndex`）放进 presenter。`_HandleCandidateWorker` 用选中的那一项向宿主会话选词并上屏，不读 Server 的回复。
- **最小镜像**：没有宿主会话时，presenter 里只有键入的原文这一项；UI-less 宿主拿到的是 Server 那一页的文字，也不带宿主会话的身份。上屏的候选来自 Server 对这次按键的选词回复，`_HandleCandidateFinalize` 读这份回复，前提是本地先选中了一项。

`CCandidateRange::GetIndex` 原来无条件返回 `0`。在最小镜像上这是对的：一项的镜像只能选第一项，Server 的回复决定上屏什么。在宿主会话的真实一页上它让 TIP 总是选第一项，Server 却按键选了第二、第三项，两边上屏的不是同一个候选。

审计时的第一版修正让 `GetIndex` 一律返回键对应的位置。宿主会话的情形因此对了，最小镜像却坏了：`'3'`、`;`、`'` 映射到第 2、3 项，一项的镜像里 `SetSelectionInPage` 越界返回失败，处理器在读 Server 回复之前就退出。Server 已经选好并清空了组字，TIP 仍显示原文，两边从此分歧。

另一处问题在归类。`IsSecondThirdCandidateKey` 只看旧的按键缓冲 `_keystrokeBuffer` 判断是否在组字。宿主会话拥有组字时，`_HandleCompositionInput` 在 `AddVirtualKey` 之前就返回，这个缓冲一直是空的，于是 TIP 从不接管这两个键：`;` 被当作标点，TIP 的宿主会话上屏高亮候选加 `；`，Server 却按自己会话的 `editing_text` 选了第二个候选。

## Decision

选词位置由 `common/SecondThirdCandidatePolicy.h` 的 `tip_candidate_selection_index` 决定，`CCandidateRange::GetIndex` 只是调用它：

- presenter 持有宿主会话的真实一页（`CCandidateSessionState::HoldsEnginePage`，判据与 `_SetText` 相同：第一项带着 `_EngineSession`）时，数字键、数字小键盘、`;` 和 `'` 都按 `candidate_selection_slot` 映射到当前页的位置，超出一页的键不选。
- 否则一律选第一项，与二三候选之前所有数字键的做法相同，Server 的选词回复决定上屏的候选。

TIP 判断这两个键时，组字状态从组字的拥有者那里读：

- 有宿主会话时，读宿主会话的视图：`editing_text` 是否为空、`caret_position`、`spelling_symbols`。这正是 Server 在 `ReplyComposer::second_third_candidate` 里从自己会话的视图读的字段。视图读不到时不接管，键照常走后面的归类。
- 没有宿主会话时，用旧的按键缓冲和本地记下的网址模式、V 模式，与原来相同。

「这个键在 Engine 当前状态下是不是输入」（视图拼写的符号，或微软双拼声母后的韵母 ing）收进 `second_third_candidate_engine_input`。Server 和有宿主会话的 TIP 都调用它，判断写在一处。

## Alternatives considered

- **恢复 `GetIndex` 恒为 0**：最小镜像回到原样，改动最小。但宿主会话拥有组字是正常情形，那时 TIP 用选中的那一项向自己的宿主会话选词，恒为 0 就意味着 `;`、`'` 和数字 2 到 9 都上屏第一个候选，和 Server 选的不一样。放弃。
- **按 `_hostEngineAdapter->valid()` 决定用真实位置还是 0**：多数时候与 presenter 的形态一致。但宿主会话有效、视图却读失败时，`GetCandidateList` 退回一项的镜像，按真实位置选会越界。presenter 里实际放的是什么才是判据，所以看第一项有没有宿主会话的身份。
- **让排队按键的投影也从宿主会话的视图起步**：`_EnsureDeferredKeyProjection` 从旧的按键缓冲取初值，宿主会话拥有组字时它从空开始。改它能让排队路径上的二三候选也读到组字，但投影是所有排队按键共用的（数字、空格、退格都靠它），改动面远超这个功能，又只能在真实 Windows 会话里验证。排队路径因此保持现状，`;` 和 `'` 在那里与数字键用同一份投影判断。

## Consequences

- 有宿主会话时，`;`、`'` 和数字 2 到 9 在 TIP 和 Server 两边选同一个位置；没有宿主会话或在 UI-less 宿主里，选词结果仍完全来自 Server 的回复，与二三候选之前一致。
- 有宿主会话时，数字 2 到 9 在 TIP 一侧的行为变了：原来 TIP 总是上屏第一个候选，现在上屏键对应的候选。这是按代码推出来的修正，还没有在真实 Windows 会话里按过键确认。
- 归类 `;` 或 `'` 时，若二三候选已打开、宿主会话有效，TIP 会多读一次宿主会话的视图（进程内调用，不走管道）。只有这两个键在开关打开时才走这条路。
- 排队按键的投影在宿主会话拥有组字时从空开始，这是现有的限制，数字键同样受影响：按键排在异步请求后面、而组字是在那之前开始的，投影会认为没有在组字。

## Testing

- `platforms/windows/tsf/tests/input/candidate_selection_slot.cpp`（`msime-tsf-candidate-selection-slot`）：数字行、数字小键盘、`;`、`'` 到当前页位置的映射；宿主会话的真实一页按位置选、超出一页不选；最小镜像对每个选词键都选第一项。
- `platforms/windows/tests/input/second_third_candidate.cpp`（`windows-second-third-candidate`）新增 `second_third_candidate_engine_input` 的断言：网址模式拼写的 `;` 和 `'`、拼音里的 `'`、微软双拼的韵母 ing 和整音节之后的 `;`。
- TSF 的策略测试在 macOS 上用 `clang++ -std=c++17 -DMSIME_EDITION_FULL` 编译运行通过；Server 测试文件和 `ReplyComposer.cpp` 在 macOS 上做了语法检查，新增断言的调用另行编译运行通过。TIP 改动只经过 CI 的 MinGW 交叉编译，没有在真实 Windows 会话里运行。
