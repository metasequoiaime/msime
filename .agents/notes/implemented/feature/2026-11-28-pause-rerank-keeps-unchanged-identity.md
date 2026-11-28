# Agent Note: 停顿重解没有改变候选时不推进代次

Status: implemented

## Problem

#11819 的第一条：延后到停顿的逐键模型（`Runtime::settle_deferred_scorer`）每跑一片都推进候选代次，候选没变也推进。没解完的片（`DeferredSettle::Continue`）本来就不改屏幕上的候选，解完的片在小词库、单字或首选由词库确定的组字里也常常什么都没换。代次一换，宿主手上画着的候选身份全部过期：安卓在模拟器上按停顿发布的时刻扫按压，与发布重叠的 7/7 次点击被当成过期丢掉，多数那几次 `moved` 为假（[安卓候选按住时不换代，停顿计时从输入算起](../bug-fix/2026-11-22-android-candidate-update-timing.md)）。各宿主为此都得在 `moved` 为假时照样收 `view` 重画，安卓、鸿蒙另加了「按住时不起跑」这类宿主侧保护。

## Decision

候选代次只在这一片改变了候选视图时推进。

- `settle_deferred_scorer` 开始前记下 `SettleBaseline`：整份 `EngineSnapshot`（`self.cached`）、`candidates_epoch` 和 `composition_epoch`。两条路（解完、没解完）都经 `Runtime::publish_settled_view` 收尾：先 `refresh`，再比较；三项都相同才算没变，不推进代次，否则推进。
- 「没变」比 `refresh` 里决定保不保高亮的比较更严：整份快照逐项相同（包括评分证据 `candidate_static_evidence`、裁决点、触点支持这些不画出来的列），候选视图计数没动（释义被清掉、插件标记、高亮都会让它动），组字计数也没动。这时每个座位上还是同一个词、同一个来源，旧身份选到的就是用户看到的那一个；座位到引擎下标的映射（`engine_order`）是 `refresh` 新建的，旧身份照新映射选，选中的仍是同一行。
- `moved` 的语义不变，仍只看候选文字。只换了来源、注释的片 `moved` 为假但代次前进，仍附带 `view`。
- 代次已经是 `u64::MAX` 时，挂着延后模型的会话直接报 `IdentityExhausted`，不先重解再发现推进不了：以前先 `advance` 再 `refresh`，现在顺序反过来，要在动引擎之前拦下。与落定重排（`settled_rerank_blocked`）的做法一致。
- host-api 的 `deferred_on_session` 本来就按代次变没变决定带不带 `view`，接口形状不变：没变时回 `{"status":"done"|"continue","moved":false,"current":true}`，不带 `view`。`msime_client_finish_pause_rerank` 同理，没变时不带 `view`，宿主照画着的高亮项上屏。

## 宿主兼容

调停顿重排接口的宿主（macOS、Windows、安卓、鸿蒙；Linux 和 iOS 不调）都只在回复带 `view`（或 `moved` 为真）时重画，没有 `view` 时保留画着的视图：macOS `scheduleSettledRerankAfter:` 与 `finishPauseRerankBeforeCommit`、Windows `ServerSession::rerank_settled`、安卓 `PauseRerankController.publishView`、鸿蒙 `runPauseRerank` 都是这样，Windows 不写 `pause_rerank`，本来也走不到延后模型这条路。所以这次不改任何宿主代码，只改了 macOS 那条说明「每一片都推进」的注释。安卓 `PauseRerankControllerSmoke` 和鸿蒙 `pause-rerank-identity.test.ts` 用的是合成回复，覆盖的是「带 `view` 就收」，照旧成立。

## Alternatives considered

- **比较只看候选文字（与 `moved` 相同）。** 最简单，但同一座位上的词换了来源（云行被本地同词顶替）或注释，旧身份会带着旧来源的预期选中新来源的行；选词事实、学习和释义都按来源走。所以比整份快照。
- **比较 `refresh` 里的 `candidates_held`。** 那是决定高亮留不留的谓词，不含释义被清掉、插件标记这些视图变化；释义清掉后宿主不收新视图，就会一直画着已经作废的释义。所以再加两个计数。
- **继续每片推进，靠宿主按文字找回。** 这是 #11819 第 4 条要做的事，但它解决的是「候选真的换了座位」；候选没变时根本不该让身份过期，宿主少一次无谓的重画，也不用在按住时推迟停顿。两者不冲突，这条先做。

## Consequences

- 候选没变的停顿重解不再让宿主手上的身份过期，点击、数字键、空格照画着的身份选词都有效；宿主少一次重画。没解完的片一律不换代（它们本来就不改候选）。
- 每片多克隆一份 `EngineSnapshot`（原来只克隆候选文字），停顿时才发生，代价可以忽略。
- 浮点评分证据里有 NaN 时快照永远不相等，会照旧推进代次：只多一次重画，不会选错。
- 原来按「每片都推进」写的回归改为断言新规则：`crates/host-api/src/tests/deferred_candidate_identity.rs` 的 `unchanged_deferred_candidates_keep_the_drawn_ids`（没变时不带 `view`，停顿前的代次照旧选得中）和 `finishing_the_pause_rerank_hands_back_the_view_to_commit_from`（小词库里做完重解候选不变，不带 `view`，照停顿前的代次上屏）；`crates/input-runtime/src/tests/deferred_scorer.rs` 新增 `a_settle_that_changes_nothing_keeps_the_generation`（不打分的模型解完后代次和视图计数都不动，停顿前的第二个身份选中的就是那个词）、`a_settle_that_reorders_rejects_identities_from_before`（换了首选时停顿前的身份报 `StaleCandidate`），`a_slice_that_runs_out_leaves_the_view_alone_and_the_next_one_carries_on` 加断言没解完的片不换代。撤掉修复时新增的两条断言失败。
- #11819 其余几条（非打字派发保留模型、晚到在线行不挪可见候选、按文字选词、空格上屏规则统一）仍按[候选身份与晚到结果规则](../../proposed/feature/2026-11-27-candidate-identity-late-results.md)逐条做。
- 部分取代[鸿蒙与安卓的停顿重解](2026-11-04-harmony-pause-rerank.md)里「文字没有变化时也发视图」的理由：那时代次每片都前进，必须发；现在没变就不前进，也就不用发。
