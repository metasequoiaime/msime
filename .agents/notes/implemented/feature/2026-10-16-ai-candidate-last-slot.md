# Agent Note: AI 候选不再占座位，回答到达时坐第一页最后一格

Status: implemented

## Problem

[AI 候选先占座位](2026-10-11-ai-seat-placeholder.md) 为了 #6888（AI 回答到达时本地候选整体后移，用户按下的数字键选中了另一个词）给每个能问 AI 的组字在第 2 位放一行占位：等回答时是「…」，落空后是「—」，回答到达时由第一条 AI 候选替换。它确实做到了回答到达时一格都不动，代价却落在每一次组字上：第 2 位几乎总被「…」占着，按 2 什么都不上屏，真正的第二个本地候选被挤到第 3 位；模型慢、网关不兼容或请求失败时，这一格从头到尾就是「…」或「—」。用户的反馈是第一页白白少了一个能选的格子，而且是最常用的第二格。

防误选只需要在回答到达的那一刻成立，不需要让 AI 在回答之前就占住位置。

## Decision

不再有占位行。AI 回答到达时，第一条 AI 候选插进第一页最后一格，前面的格子一格都不动；原来在最后一格的候选和它后面的行各往后挪一格，成为下一页的第一个。

### 放在哪里

- 座位由 input-runtime 决定：`Runtime::normalize_online_slots`（`crates/input-runtime/src/runtime.rs`）。其余行先排好：有云候选时按原来的分组座位表（首选、云候选、日文、英文、emoji、颜文字的优先段，然后各组剩余段），AI 不在表里；没有云候选时保持引擎的次序，和回答到达之前一模一样。然后第一条 AI 行插进下标 `page_size - 1`，同一批其余 AI 行接在整个列表最后。
- `page_size` 是 Runtime 的每页候选数，也就是宿主画出来的那一页。host-api 用 `host_page_size` 按 `HostPlatform::max_candidate_page_size` 截断偏好或宿主覆盖的值（Windows、iOS、HarmonyOS 最多 9，macOS、Linux、Android 最多 10）后交给 `Runtime::new` / `replace_engine`，同一个值也作为 `first_page_rows` 交给引擎。组字中途 `set_page_size` 返回 `CompositionActive`，所以一次组字里座位不会变；翻页和展开刷新（`expand_initial_candidates` 之后的 `order_presentation`）按同一个值重算，AI 行始终在第一页最后一格。视图和数字键用同一份排列（`engine_order` 映射回引擎下标），按 `page_size` 那个数字键选中的就是画出来的 AI 候选。
- 每页只有一格时座位是下标 1，不顶掉首选（空格上屏的那一个）。其余行不足一页时 AI 行紧跟在它们之后，什么都不挪。
- 有云候选时固定位置的英文行先在其余行里归位，再插入 AI 行：固定在第一页前 `page_size - 1` 格的英文位置不变；固定在最后一格或更后的英文和那里的本地候选一样往后挪一格。没有云候选时固定位置由引擎排好，Runtime 不再动它们。
- 云候选不变：仍在第 2 位，到达时第 2 位以后的行后移一格；它和 AI 行同时在时，AI 行仍坐第一页最后一格。

### 引擎这一层

- 引擎不知道宿主一页画几格以外的座位规则，只把 AI 行放在最后：`replace_online_candidate_batch`（`crates/engine/src/ime/online_batch.rs`）去掉同来源之前的行（`holds_ai_seat`，旧版本留下的占位行一起去掉）后，把整批按回答的顺序接在列表末尾；云候选插在第 1 位，列表里只有 AI 行时插在它们前面。固定位置归位（`apply_fixed_positions_with_state`）把云候选放回第 1 位、AI 行按原顺序接在末尾。
- 英文和日文混输的优先位置（`priority_slot`）只看云候选，不再因为有 AI 行往后让。混排英文、emoji、符号（`SYMBOL_AMBIGUOUS_SLOT`、`SYMBOL_UNANCHORED_SLOT` 是绝对下标）和固定位置因此都按没有 AI 行的列表计算，AI 回答到达前后其余行在引擎里的相对次序不变，Runtime 的「没有云候选时保持引擎次序」才成立。
- 重排模型照旧不比较来源 3 和 12（`ordering.rs` 的 `AI_SEAT_SOURCES`）。回答的词全都已经在列表里时，批次被接受但列表不变。

### 去掉了什么，留下了什么

- 去掉：`InputSession::reserve_ai_seat`、`ai_seat_query`、`set_ai_seat`、`resolve_online_candidates` 和会话的 `ai_seat`、`ai_seat_resolved`；`Session::set_ai_seat`、`resolve_online_candidates`（引擎内部和 `host::Session`）；`Runtime::set_ai_seat`、`resolve_online_candidates_owned`；host-api 的 `ai_seat_ready`、`refresh_ai_seat`、`AI_CREDENTIALS_IN_PROVIDER`；常量 `AI_SEAT`、`AI_PENDING_TEXT`、`AI_UNAVAILABLE_TEXT` 和 `rehome_ai_seat`。这些只为决定放不放占位行、放在哪、写什么字而存在。
- 留下：`CandidateSource::AiPending = 12`（值不复用）；引擎里来源 12 不上屏、不学习、不能固定置顶删除、不能进入改字、收尾时按首选处理的判断；`holds_ai_seat`、`takes_online_seat`；transform 里来源 12 锁定座位；各宿主画来源 12 的代码（Android `AiPendingCandidatePolicy`、iOS `CandidateMarker.aiPendingSource`、Windows `candidate_source_ai_pending`、macOS `MSIMECandidateIsAIPlaceholder`、Fcitx5 斜体、HarmonyOS 次要色）。旧的库或数据里出现这种行时仍按占位行处理。
- C ABI `msime_client_resolve_online_candidates` 保留，签名不变：缓冲、来源（0 或 1，其余报错）和查询文档照旧校验，合法调用一律回答 `{"applied": false, "view": 当前视图}`，不改候选、不换代、不推导生命周期事件。宿主照旧在 AI 落空时调用它。

### 宿主

| 宿主 | 改了什么 |
|---|---|
| macOS | AI 空回答或失败时保留查询身份，同一组字不再重发；有回答却被 apply 拒绝时清掉身份，等下一次渲染重试。以前不重发靠 resolve 返回 applied:true，现在它总是 false |
| iOS | 展开网格的 `onlyTheAISeatChanged` 改判：组字相同，除 AI 行（来源 3、12）外各行的文字和来源先后相同，新列表有来源 3 的行。回答插进最后一格时网格原地刷新、不收起、不跳回开头 |
| Android、iOS、HarmonyOS | 「同一组字 AI 落空后不再问」（`aiResolvedSignature`）照旧，否则键盘每次重画都再发一次请求 |
| Windows、Linux | 只改注释；`AiCandidateWorker` 照旧把空回答交给 owner，owner 调用的 resolve 不再改视图 |

各宿主说明 resolve 和来源 12 的注释改成现状。Android 设备验收 `AiPlaceholderDeviceSmoke` 改名 `AiCandidateSeatDeviceSmoke`，按新规则验收。

## Alternatives considered

- **保留占位行（2026-10-11 的做法）。** 最强的理由：回答到达时页上任何一格都不动，最后一格也不动；失败时「—」明确告诉用户 AI 没有结果；读屏用户能听到「AI 候选等待中」。不用：代价落在每一次组字上，第 2 位常年是一个按了没反应的格子，而防误选只在回答到达的那一刻需要。新规则把会被挪动的格子限制在最后一格，前 `page_size - 1` 格保持 #6888 的保证。
- **回答到达后插在第 2 位（#6888 之前的做法）。** 最强的理由：AI 候选最显眼，引擎和参考实现原来就这样排。不用：这就是 #6888 的误选，第 2 位以后的每一格都会挪。
- **只把 AI 行接在整个列表末尾。** 最强的理由：回答到达时一格都不动，规则最简单。不用：默认每页 6 个，常用读音的本地候选远不止一页，开着 AI 的用户几乎看不到它，等于没开。
- **引擎按 `first_page_rows` 直接把第一条 AI 放在最后一格。** 最强的理由：引擎快照就是显示顺序，没有云候选时 `normalize_online_slots` 是恒等排列，不建索引表，保住旧实现在「已经坐好」时零分配的路径。不用：座位规则要在引擎和 Runtime 各写一遍，两边会漂移；引擎在固定位置之后还有只出单字过滤、表情不领先、网址补全等步骤会移动下标；混排在 AI 行之后才插行，AI 行若排在前面，符号行的绝对下标会错开。代价是有 AI 行时 Runtime 每次刷新建一次 O(n) 的排列并重排平行数组。
- **固定位置的英文优先于 AI。** 最强的理由：用户明确固定的位置一格不差。不用：固定在最后一格的英文不让，AI 只能坐到前一格，那一格的候选就被挪动，破坏「前 `page_size - 1` 格不动」；固定在前面几格的英文不受影响。
- **保留 `set_ai_seat`、`ai_seat_ready` 作为不起作用的开关。** 最强的理由：改动面小，引擎、Runtime 和 host-api 的接口不变。不用：没有读者的开关只会让下一个人以为它还决定着什么；宿主从 C ABI 看不到它们，需要保留的只有 `msime_client_resolve_online_candidates`。

## Consequences

- **收益**：第 2 位总是第二个本地候选，等回答时第一页全是能选的候选，不再有常年显示的「…」「—」。回答到达时第一页前 `page_size - 1` 格不动，#6888 的保证只在最后一格让步。
- **代价**：用户正要按最后一格的数字键时回答恰好到达，会选中 AI 候选；被挤走的候选到了下一页。AI 落空时没有任何提示。读屏不再念「AI 候选等待中」。
- **行为变化**：没有云候选、有 AI 行时，Runtime 不再把 emoji、英文拉进分组座位表，而是保持引擎的次序（emoji 跟着它描绘的词）；`candidate_limit` 默认 3 时第一页仍只有一条 AI 候选。
- **触屏宿主**：Android、iOS、HarmonyOS 的候选条横向滚动、列出全部候选，AI 行落在第 `page_size` 个（偏好默认 6），窄屏上可能要滑一下才看得到。
- **开销**：有 AI 行的列表每次刷新多一次 O(n) 排列（索引表加平行数组重排）；没有 AI 行、或已经坐好的列表仍走零分配路径（`online_slot_allocations` 的预算测试）。
- **golden**：引擎的 AI 行位置和英文优先位置变了，7 个在线场景的期望顺序更新，`tools/engine-golden/README.md` 记了这次偏离。
- **重访信号**：如果用户反映最后一格经常误选，考虑在回答到达后的短时间内暂不插入，或把 AI 放到下一页第一格；如果宿主开始按可见宽度而不是 `page_size` 决定一页，座位要改成跟着宿主报的可见格数。

与其它笔记的关系：[AI 候选先占座位](2026-10-11-ai-seat-placeholder.md) 的占位行、第 2 位座位、resolve 换「—」和「谁打开 AI 座位」由本篇取代，该篇仍持有宿主不重问 AI、macOS 定时器与批次预算、`MSIMEViewContainsAICandidate` 和在线归位两次扫描的事实；[日文混输](2026-10-12-mixed-japanese-candidates.md)、[英文领先](2026-10-12-mixed-english-leads-unsegmentable-input.md)、[macOS 设置精简](../simplification/2026-10-16-macos-tools-support-settings-prune.md)、[脚本插件改候选](../architecture/2026-10-12-script-plugins-modify-commit-and-candidates.md) 和 [AI 凭据轮换](../bug-fix/2026-10-13-ai-credential-rotation-invalidates-candidates.md) 里关于优先位置和 AI 座位的事实已就地更新；[MCP 关 AI](2026-10-15-mcp-ai-assistant-off-switch.md)、[AI JSON 网关](../bug-fix/2026-10-15-ai-candidate-json-mode-gateways.md)、[释义代次](../bug-fix/2026-10-11-generation-gloss-merge.md) 的 Problem 描述的是当时的占位行，不改；[macOS 读屏高亮](../bug-fix/2026-10-13-macos-candidate-voiceover-highlight.md) 的占位行读法仍在代码里，不改。

## Verification

- `rbuild cargo test -p msime-engine -p msime-input-runtime -p msime-host-api -p msime-client-core`：引擎库 2201 项、golden 31 项、input-runtime 295 项、host-api 641 项、client-core 389 项通过。
  - 引擎：`online_batch` 的 AI 行接在末尾、不挪已有行、去掉旧占位行、只有 AI 行时云候选在前；`positions` 的 AI 行排最后且本地和固定行位置与没有 AI 时相同；`queries` 的 `an_ai_row_does_not_move_the_english_slot`；`session` 的 `no_ai_placeholder_is_ever_inserted`、`an_ai_answer_goes_after_the_local_rows_without_moving_them`、`an_answer_of_listed_words_changes_nothing`。
  - input-runtime：`the_first_ai_candidate_takes_the_last_slot_of_the_first_page`（每页 5、9、10、1 个，第一页排不满，引擎把 AI 放在中间，有云候选、英文、emoji，固定在第 2 位和第 5 位的英文，前 `page_size - 1` 格与回答前相同）；真 Engine 的 `the_ai_answer_takes_the_last_slot_of_the_first_page_without_moving_the_rest`（每页 5 个，翻到下一页再翻回，按 5 上屏 AI 候选）和 `a_short_first_page_takes_the_ai_answer_after_its_last_row`；`online_slot_order_equivalence` 的独立参考实现按新规则穷举对照；`online_slot_allocations` 的已坐好列表零分配。
  - host-api：`an_ai_answer_takes_the_last_slot_of_the_first_page_and_resolve_changes_nothing`（没有来源 12，resolve 来源 0 和 1 都答 applied:false 且视图不变，来源 2 和坏文档报错，回答到达后坐第 5 格）。
- `rbuild cargo clippy -p msime-engine -p msime-input-runtime -p msime-host-api -p msime-client-core --all-targets -- -D warnings` 和 `cargo fmt --check` 通过。
- 宿主原生代码只做了静态检查：`platforms/android/check-host.sh`（237 项 JVM 冒烟、144 个设备套件源文件编译），改过的 Swift 文件 `xcrun swiftc -parse`，`ShortcutTest.mm`（引入 `InputController.mm`）`clang++ -fsyntax-only -Wall -Wextra -Werror`；`scripts/test-candidate-sources.py`、`scripts/test-android-device-ime-rebind.py` 通过。iOS XCTest、macOS `ctest`、Android 模拟器验收、Windows、Linux、HarmonyOS 的构建和真机都没有跑。
