# Agent Note: Linux IBus 候选点选按面板快照解释，译文临时页不比它显示的那一页活得久

Status: implemented

## Problem

IBus 的 `candidate_clicked` 只带面板上那一页里的槽位号和按键，不带任何身份。宿主唯一能拿来解释它的是自己最后画出去的那份快照（`rendered_view` / `rendered_candidates`）。审计指出 `ClientEngine.cpp` 的点选在三处没有这样做：

- **译文临时页**（Ctrl+Enter 遇到多个译义时打开）的左键按点击时的状态现算全局序号 `translation_page * page_size + index`，不看面板上画的是什么。正常流程里 `translation_page` 和画出去的页总是一致，审计说的「点击排在翻页之后」本身无法区分：点击不带页码，宿主无从知道用户点下去时看到的是哪一页，普通候选页同样如此。真正出错的是另一种情形：译文页开着时别的视图换掉了 `s.view` 并画了出来——菜单改每页候选数、异步到达的译文（离线命中先画、网络结果后到）、云候选、稳定重排、偏好更新都会这样做，而它们都不知道译文页的存在。面板上显示的已经是普通候选，`translation_candidates_active` 却还开着，于是点第一行上屏的是「first sense」这样一条已经不在屏幕上的译义；数字键和空格走同一份状态，同样选错；Esc 则把打开译文页时保存的那份视图恢复回来，它的代次已经落后于运行时，接下来的点选被运行时以 `StaleCandidate` 拒绝。
- **普通候选的左键**只核对会话，不像滚轮和 `page()` 那样核对快照的代次与当前视图一致，完全靠运行时的代次栅栏兜底。运行时拒绝一个落后的选择时 `response()` 抛异常，`guarded()` 的兜底是关会话、清组字，所以「靠运行时兜底」的代价是用户的整段组字没了。
- `pointer_turn()`（`msime_client_key(MSIME_KEY_POINTER)`，开启新的一轮）在任何校验之前就调用了，被拒绝的点击、关着「鼠标滚轮」时的滚轮、右键提示也都开了一轮。macOS 宿主的约定是「被拒绝的点选什么都不报告」，报告和派发的条件一致。

Fcitx5 没有这个问题：每个候选词对象在渲染时记下 `(session, generation, global index)`，点击落在对象本身上。

## Decision

- **点选按渲染时记下的身份解释。** `candidate_clicked` 在做任何事之前先核对快照：本会话、快照非空、`rendered_view` 与 `s.view` 的代次相同（与滚轮、`page()` 同一道栅栏），不通过就直接返回。普通候选从 `rendered_candidates[index].id` 取代次和全局序号，这一点原来就是如此。
- **译文页的序号也取自快照。** `render_translation_candidates` 在临时页的视图上写宿主自己的标记 `translation_page`，它随 `render` 进入 `rendered_view`。点选要求快照的 `translation_page` 等于当前页，再从被点那一行的 `id.index`（渲染时写入的全局序号）取要上屏的译义，不再在点击时现算。
- **译文页不比它显示的那一页活得久。** `render()` 画一份不带 `translation_page` 的视图时，说明别的视图已经顶替了译文页：丢掉临时页的状态（选项、页码、光标和保存的视图），不恢复保存的视图——新画上去的视图才是权威。这样点选、数字键、空格和 Esc 都作用于面板上真正显示的候选。录音时的渲染在这之前提前返回，不丢临时页，录音结束后照旧画回它。
- **轮次只开给真正派发的点击。** `pointer_turn` 移到每条派发之前：选择、滚轮翻页、译义上屏、译文页翻页。被拒绝的点击、关着鼠标滚轮时的滚轮、右键提示都不再开一轮。

## Alternatives considered

- **只在点击里加栅栏，拒绝译文页状态与面板不一致的点击**：这是审计建议的最小改动，最强的理由是改动面只在 `candidate_clicked` 一个函数里。但被替换掉的译文页状态还会让数字键、空格选错译义，让 Esc 恢复一份过时的视图；而且用户点的是面板上看得见的普通候选，拒绝它等于让那一下点击失灵。根因是临时页的状态比它在面板上的那一页活得久，所以在唯一的绘制出口 `render()` 里收掉它，点击里的栅栏只留作「渲染中途失败、快照没更新」这种情形的兜底。
- **在每个会换掉 `s.view` 的地方先退出译文页**：语义最直白，但这样的地方有十几处（菜单项、异步回调、偏好更新），以后新加一处就会再漏一处；`render()` 是它们共同的出口。
- **给 IBus 的点击补上身份**：IBus 的 `CandidateClicked` 协议只有槽位、按键和修饰键，宿主控制不了面板，做不到。
- **把普通左键也改成像 Fcitx5 那样在点击里带代次**：同上，协议里没有位置可放。

## Consequences

收益：译文页被别的视图替换之后，点选、数字键和 Esc 都作用于面板上真正显示的候选；落后的左键在宿主这一层就被挡住，不再把运行时的拒绝变成关会话、丢组字；被拒绝的点击不再开新的一轮，Linux 与 macOS 的轮次约定一致。

代价与缺口：点击排在一次已经画出去的翻页之后（用户看着第一页点下去、宿主先处理了 Page Down）仍会按新的一页解释，这是 IBus 协议的限制，普通候选页一直如此，不在本次范围内。`page()`（IBus 的 page_up/page_down 和滚轮的 cursor_up/cursor_down）和托盘菜单的「候选翻页」仍在校验之前调用 `pointer_turn`，被拒绝的翻页会多开一轮，没有派发也就没有别的副作用；本次没有改它们。普通左键的代次栅栏没有可以从外部稳定触发的场景：现有每条换掉 `s.view` 的路径都会立刻画出来，只有 `render()` 中途抛异常时快照才会落后，所以它由静态契约而不是运行测试守住。

## Verification

- `platforms/linux/tests/runtime/engine_smoke.cpp` 新增 `--stale-candidate-click` 模式（ctest `ibus-stale-candidate-click`，需要已校验的词库）：每页一条时翻到第二页译文后点选上屏「second sense」；每页两条打开译文页后用菜单改每页候选数换掉它，点第一行、或按数字键 1，上屏的必须是面板上显示的第一个候选「你好」而不是任何译义；普通候选页第二页的点选按全局序号选，Reset 之后的点选和滚轮什么都不做。在 `build-container.sh` 的镜像里对按 `resources/desktop-dictionary.lock.json` 取回并校验的词库运行：换回未修复的 `ClientEngine.cpp` 时停在「点选上屏的不是面板上显示的第一个候选：shown=[你好 · first sense; second sense] committed=[first sense]」，修复后通过。单独成一个模式，是因为默认模式在 develop 上停在更前面一处无关的失败（「English-mode recording did not show streaming preedit」，见 [legacy 包笔记](../feature/2026-10-09-linux-legacy-glibc228-deb.md)），到不了这些用例。为确认默认模式里原有的点选用例不受影响，在同一镜像里临时把录音相关的断言改成只打印、不中断（不提交）再跑默认模式：修复后的程序走过了多义译文的点选、滚轮和 Tab 用例，停在后面一处同样与录音状态有关的「Forward delete cancelled incremental composition instead of editing at caret」，未修复的程序停在同一处。
- `platforms/linux/tests/core/fcitx5_contract.py`（ctest `linux-fcitx5-contract`，构建门禁里运行）静态钉住：`candidate_clicked` 的代次栅栏在译文页分支和每个 `pointer_turn` 之前，每次 `pointer_turn` 之后、下一个 `return` 之前必有一次派发，译文页的序号来自快照行的 `id`、不再现算，`render()` 丢掉被替换的译文页。在未修复的源码上它失败在第一条顺序断言上。
- `scripts/test-linux-rendered-view-guard.py`、`scripts/test-linux-glib-callback-view-guard.py` 通过；`bash platforms/linux/build-container.sh` 编译 IBus engine、Fcitx5 插件与全部单测并跑 ctest。
