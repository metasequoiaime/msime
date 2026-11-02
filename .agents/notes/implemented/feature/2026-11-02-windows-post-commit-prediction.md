# Agent Note: Windows 上屏后联想由 TIP 在光标处显示、Server 画候选窗

Status: implemented

## Problem

[上屏后联想](2026-10-24-post-commit-prediction.md)要在 Windows 上打开（#9433），六个平台同一条规则：点选选中点的那一条，数字键选本页对应的一条，空格选高亮的一条；回车、标点和退格照常交给编辑器，Esc 关闭联想。Windows 有三处和这条规则直接冲突：

- 候选窗由 Server 画（`CandidatePresentation.h`），`editing_text` 为空就当作组字结束、不画；TIP 只在组字区间上开候选 UI，位置也来自组字区间的文字范围。联想没有组字，候选窗根本出不来。
- TIP 结束组字时总会给 Server 发 `HideCandidateWnd`，Server 收到就取消会话里的组字（`FocusedSession::hide_candidate`）。上屏那一下刚出来的联想会被同一次上屏顺带的隐藏清掉。
- TIP 进程里另有自己的宿主会话（`EngineSessionAdapter`），候选非空就当作在组字（`HostCharacterResult.h`）。联想会被当成组字开一段 TSF 组字，回车或失焦可能把用户没选的词上屏。反过来，没有组字时 TIP 把数字和空格都交给应用，它们到不了选词。

## Decision

`HostPlatform::shows_next_word_prediction` 加上 Windows，TIP 的宿主会话和 Server 的会话都按偏好出联想。两边分工如下。

**TIP 决定显示与关闭，位置用光标。** 联想不算组字（`HostPrediction.h` 的 `HostViewComposes`），上屏之后照常结束 TSF 组字。每次按键编辑会话结束时 `_RefreshPrediction` 读宿主会话：出了新的一代联想，就取当前选区折叠到末端的空区间量出光标位置（量不出高度时量光标前一个字的右端），给 Server 发 `ShowCandidateWnd`；联想没了就发 `HideCandidateWnd`。这一段不建 TSF 组字，也不建候选 UI 元素，`_candidateMode` 一直是 `CANDIDATE_NONE`，组字的按键分类不会认领任何键；联想要的数字和空格由下面的规则单独认领。UI-less 宿主（游戏自己画候选）和 Server 连不上时不显示。

**按键。** 联想显示着时，`OnTestKeyDown`/`OnKeyDown` 先看 `PredictionKey`：不带修饰键的空格和本页行数以内的 1–9（主键盘或小键盘）是选词，`_IsKeyEaten` 把它们认领为 `FUNCTION_PREDICTION_SELECT`，照常带请求号送给 Server，Server 走原有的选词路径（空格是 `MSIME_COMMIT_CANDIDATE`，数字按可见行号 `select`，共享运行时在联想上接受这两种选择），TIP 在编辑会话里读回复，选中了就和点选一样插在光标处（`_HandlePredictionSelectKey`）；不带修饰键的 Esc 吃下并关掉联想；单按的修饰键不动；其余键（回车、标点、退格、超出行数的数字、带修饰键的数字和空格）先关掉联想（给 Server 发隐藏、清掉宿主会话的联想），再走原来的分类，交给应用或开始组字。交给应用的键不一定还会到 `OnKeyDown`，所以关联想放在 `OnTestKeyDown`。前面还有键在排队时，排队的分类按将来的组字状态算，认不出联想选词，这时数字和空格也先关掉联想、照常交给应用。

**光标移动和焦点。** 显示时记下的空区间两端都向后吸附，光标处再插入文字时区间留在原地。`OnEndEdit` 里光标不在这个位置、或者上下文换了，就关掉联想；我们自己上屏那一次在同一个编辑会话里已经按新光标重新记下，正好对得上，不算。文档焦点变化、线程失去焦点、中英切换也关掉它。

**Server 只画已经被 TIP 显示的联想。** 候选窗投影认 `idle_prediction_view`（`prediction` 为真、没有组字、有候选），拼音为空，序号照画（数字键能选）。`CandidateMailbox::delivered` 收到联想先藏着，等 TIP 的 Show 才露出来。`FocusedSession` 记下 TIP 为哪一代联想发过 Show：在那之前到达的隐藏是上屏结束组字顺带发的，会话保留联想、候选窗先藏起来（`HideCandidateDisposition::PredictionKept`，候选窗收到带 `internal_prediction_hide` 的隐藏）；发过 Show 之后的隐藏才是关闭联想，照旧取消会话里的组字，联想随之清掉。

**点选。** 候选窗点选照旧由 Server 在自己的会话里 `select`（共享运行时在联想上接受按身份选择），再经 `CommitCurCandidate` 把文字送回 TIP。TIP 没有组字、联想显示着时，`_HandleCandidateFinalize` 交给 `_CommitPredictionCandidate`：宿主会话按文字跟着选同一个联想（`SelectHostCandidateByText`），再把文字插在光标处；编辑会话结束时从宿主会话读到下一代联想，就在新光标处再发 Show。组字中点选候选也改为先按文字在宿主会话里跟选（`_FollowServerClickInHost`），上屏的文字一致、宿主会话也不再组字才算跟上，否则照旧丢掉宿主会话的组字；以前点选总是丢掉宿主会话，点选之后宿主会话就出不了联想。

**设置。** WinUI 设置和 Win7 设置的「候选与联想」组在「学习选词习惯」之后加「上屏后联想」，标题和说明与 Android、共享页相同，关着时不写键。桌面没有联想栏按键样式这一项。

## Alternatives considered

- **TIP 的宿主会话不出联想，只由 Server 出** — 两个会话不会因各自的个人词序出不同的联想，Server 是唯一的来源。否决：TIP 要在上屏的那个编辑会话里决定显示、要在 `OnTestKeyDown` 里同步决定 Esc 吃不吃，这些都只能读本进程的宿主会话；从 Server 得知联想有没有出来要么改回复帧，要么等异步的工作线程帧，Esc 会在两者之间被错判。两个会话出的联想有没有并不依赖个人词序以外的东西（都是词库续词加个人词序），不一致时的后果也有边界：TIP 没出就不发 Show，Server 的那份一直藏着，下一个键清掉；TIP 出了而 Server 没有，就是一个空的 Show，最多白吃一次 Esc。
- **沿用候选 UI 元素（`CCandidateListUIPresenter`），只把组字区间换成光标区间** — 布局变化（滚动、窗口移动）时能跟着重新定位，读屏器也能从 TSF 的候选 UI 元素读到候选。否决：这个类和 `_pCandidateListUIPresenter`、`_candidateMode` 绑在组字上，`_candidateMode` 一旦不是 `CANDIDATE_NONE`，回车、标点、`Ctrl+Enter` 都按组字里的候选键分类（回车选高亮、标点带出高亮候选），数字和空格也走组字的选词路径（`_HandleCandidateWorker`、`_HandleCandidateFinalize` 先找组字、再在宿主会话的组字候选里选），和联想的规则对不上；它的销毁路径还会在组字开始后发隐藏，取消 Server 的新组字。联想存在的时间很短，光标一动就关，不需要跟随布局；候选窗自己的读屏元素树（`AccessibleWindow`）照样能读到它。
- **上屏时不发隐藏，或者给上屏顺带的隐藏加一个「联想跟着来」的线上标记** — 不改变「隐藏就是取消」的语义。否决：上屏时的隐藏来自好几处（候选 UI 结束、组字被应用终止时的兜底清理），给它们全部加条件或标记要改到组字生命周期的各个出口；而 Server 只要知道 TIP 有没有为这一代联想发过 Show，就能分清顺带的隐藏和关闭联想的隐藏，旧的隐藏语义对组字一点不变。
- **候选窗收到联想就直接显示在原来的组字位置** — 不需要 TIP 发 Show。否决：位置是组字开头，不是上屏后的光标；而且 TIP 不知道联想有没有显示，Esc 和光标移动都无从处理。

## Consequences

- 联想在记事本这类普通文本框里按规则工作：上屏后在光标下出现，点选、数字键、空格上屏并接着出下一代，Esc 关闭且不交给应用，回车、标点照常输入并关掉联想，点别处或打字让光标离开时关掉。
- 候选窗在联想上仍画首行（拼音行为空）。要不要在联想上收起首行留给以后看实际效果再定。
- Chromium 系宿主编辑时常换 `ITfDocumentMgr`，文档焦点一变 TIP 就关掉联想；如果在这些宿主里上屏本身就触发换文档，联想会一闪而过。这种情形没有在本次验证中复现，遇到时要改的是焦点变化的判断，而不是联想的显示规则。
- 组字中点选候选现在让宿主会话跟着前进；[空格和点选也在 TIP 的宿主会话里选词](../bug-fix/2026-10-16-windows-space-selects-in-host-session.md)当时只让点选丢掉宿主会话，这里补上它，跟不上时的兜底不变。

## Verification

- `msime-tsf-host-prediction`（联想不算组字、数字和空格选词、超出行数和带修饰键的照常、编辑会话结束时显示还是关掉）、`windows-prediction-presentation`（两条投影）、`windows-session-smoke` 里候选窗信箱的联想段（送达先藏、顺带的隐藏保留、Show 露出、关闭的隐藏清空、UI-less 不露出）、`msime-tsf-engine-response`（解析 `prediction`）、`tests/ui/windows7_settings.cpp`（Win7 开关缺省为关、打开写键、关掉删键）。
- `crates/client-core` 的 `host_surface` 测试：Android 与 Windows 声明能力，其余四个宿主不声明。
- 实机验收结果见 PR 正文。
