# Agent Note: Windows 指针翻页把转移里的 view 交给候选信箱

Status: implemented

## Problem

Windows 候选窗用鼠标滚轮（打开「鼠标滚轮翻页」之后）或点首行的 ‹ › 箭头翻页时，引擎已经翻到下一页，候选窗却一直画着翻页前的那一页，页码也不动，直到下一次按键才跳到新页。真机验收（Win11 虚拟机，develop 与 PR 构建都一样）里滚一格后仍是 `1 / 68`，接着按 PageDown 直接到 `3 / 68`；点 › 后同样停在 `1 / 68`。键盘翻页正常。

两种指针翻页都走 `SessionController::request_page`（`platforms/windows/src/ipc/SessionController.cpp`）。它把 `InputState::page_candidate` 的返回值原样交给 `CandidateMailbox::action`。而 `ServerSession::page_candidate` 返回的是 `msime_client_command` 的整次转移（`{commit, view, ...}`），信箱的 `refresh_view` 按 view 去读 `session`、`generation`、`preedit`，拿到转移时 `at("session")` 抛出，被那里给可选数据准备的 `catch (...)` 静默吞掉，快照不更新。这行从 `feat(windows): page candidates with mouse wheel`（2685fd41e1）起就是这样。

「点 › 根本不翻页」是同一个原因的第二个表现：候选窗画着的还是旧一代，再点一次箭头或再滚一格，请求带的是旧一代的身份，`FocusedSession::page_candidate` 对不上引擎当前的代次就拒绝，所以第二次真的没翻。验收里「点一下再滚一格再 PageDown 停在第 3 页」正是点那一下翻了、滚那一格被拒、PageDown 再翻一页。

## Decision

信箱的契约不变：`CandidateMailbox::action` 收的是 view，和 `online`、`translations` 一样。`request_page` 像紧挨着的 `request_candidate_action` 那样取 `transition->at("view")` 再交进去。`ServerSession::page_candidate` 的返回值也不变，它和 `candidate_action` 一样回整次转移，调用方按需取 view；`tests/input/shuangpin_paged_selection.cpp` 也在调用它。

## Alternatives considered

- **让 `ServerSession::page_candidate` 直接回 view** — 最强的理由是 `apply_cloud_response`、`apply_translations` 这些函数就是只回 view 的，返回值和用途对得更紧。否决：同一层的 `candidate_action` 回整次转移，翻页和它同属「指针动作」，改一边会让两条相邻的路一条取 view、一条不取，下次照着抄的人更容易抄错；而且转移里的 `commit` 已经在 `page_candidate` 里核对过，丢掉它没有收益。
- **让 `refresh_view` 同时认转移和 view** — 能挡住同类错误。否决：信箱是界面线程和输入线程之间的单值快照，把「是转移就取 view」放进去等于给每个调用方开一个模糊的入口，调用方传错东西时更不容易被发现。这里的问题是调用方传错了，修调用方。

## Consequences

- **收益**：点箭头、滚一格之后候选窗立即画出新的一页（页码、候选、高亮），连续点或连续滚也按新一代的身份一页一页翻；翻页之后的释义查询和卷轴展开本来就照新一代发起，现在画出来的也是同一代。
- **代价**：无。
- **未处理**：`refresh_view` 的 `catch (...)` 仍会把格式不对的 view 静默丢掉，这是为云候选、释义这类可选数据准备的；本来该报错的调用方错误（像这次）在那里看不出来。调用方都已经核对过只传 view，这次不改它。
- **重访信号**：以后再有新的 Server 动作要刷新候选窗，按 `request_candidate_action` 和 `request_page` 的写法取 `at("view")`；如果要改信箱入口的形状，先把这几处调用方一起改。

## Verification

- `platforms/windows/tests/runtime/session_workers.cpp`（属于 `windows-session-smoke`）新增一段：真实引擎、日期时间模式 `Trq`（每页两个候选，不需要词库），点 › 之后 `candidate_view()` 必须是第 2 页、新一代、首个候选换了；再从新画出的那一页点 ‹，必须回到第 1 页；拿旧一页的身份再翻必须被拒，连接不断、控制器不报错。修复前这段在「点 › 之后是第 2 页」那条断言失败。
- 整套 `windows-session-smoke` 在 Wine 下停在这一段之前的已知失败上（`scripts/known-failures.txt`），所以照 `--local-modes` 的先例加了 `--session-workers`，只跑 `session_worker_tests`。MinGW x64 交叉编译后在 Wine 下运行：`SessionController.cpp` 用修复前的版本时报 `Session worker test failed at line 708`（`request_page` 已经回 `Sent`，快照仍是第 1 页），换成修复后整段通过。
- 滚轮翻页（`from_wheel`）和箭头走同一个 `request_page`，测试只用箭头，因为夹具的偏好没有打开「鼠标滚轮翻页」。真机上的滚轮与箭头复验列在 PR 的「真机验收（待设备轮次）」里。
