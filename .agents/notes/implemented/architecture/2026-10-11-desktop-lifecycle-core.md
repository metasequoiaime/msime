# Agent Note: 桌面端生命周期核心：host-api 推导输入事件，内置声音特效与将来的脚本插件共用一条事件流

Status: implemented

## Problem

在生命周期核心之前，按键音、上屏音、打字特效和背景音乐的时机由四个桌面宿主各自决定：macOS 的 `playKeySound:` 和提交音、Windows Server 的 `FocusedSession::sound_key` 与 `set_music_active`、IBus 和 Fcitx5 各自的调用点。四份实现在细节上已经漂移：IBus 和 Fcitx5 的按键音排在引擎处理之后，Windows 的上屏音按"已送达"算，macOS 失焦上屏也出提交音，Windows 进入密码框时音乐不停。宿主之间唯一共享的是 C ABI 的 transition JSON，而它只说结果（上屏了什么、视图是什么），不说过程（是不是选了候选、选的哪一格、组字是怎么结束的）。

计划中的沙箱脚本插件（观察上屏、改写上屏、重排候选）需要的正是这条"过程"：一个宿主中立、顺序有契约、覆盖所有真实选词路径的事件流。如果没有一个核心先把它定下来，脚本只能挂在各宿主自己的回调上，四个宿主就要各写一份事件顺序。

## Decision

`crates/host-api/src/lifecycle.rs` 是桌面端的生命周期核心，分阶段接入。第 1 阶段只建核心本身，没有宿主调用新接口时用户看不到任何变化，移动端和桌面的 transition JSON 都逐字节不变。第 2a 阶段 macOS 宿主接入并打开 `FEEDBACK`（见下面的「macOS 接入」）；Windows、IBus 和 Fcitx5 还没有调用新接口，行为照旧。

**事实来自 runtime 的旁路，不改 `Transition`。** `crates/input-runtime` 新增 `Runtime::facts()`，返回 `Copy` 的 `DispatchFacts`，在 `dispatch` 和 `focus` 入口重置：选中的候选（`SelectionFact{seat, source, how, outcome}`，只在 `Finish`、两条标点路由、五笔顶字和唯一码自动上屏、空格落在打开的列表上、数字、14 键原样上屏、点选、以词定字、确认高亮行这些引擎调用点写入，并要求候选文本和引擎的上屏对得上）、撤回暂存选择、暂存段是否增长或随上屏送出、是否原样上屏、按字符数计的上屏尾巴（标点、顶字的键）。另有 `has_composition()`（不看 `snapshot_valid`）和两个 epoch 计数器：高亮移动和翻页在 runtime 里提前返回、不经过 refresh，只比较 refresh 前后的快照会漏掉它们。`Transition` 和它的 JSON 是所有宿主共用的契约，不往里加字段。

**推导是纯函数。** `lifecycle::derive` 只吃调用前后各一次的 `Sample`、派发事实、上屏的字符数和当前页的三项数字，按固定顺序发出事件：`input`（派发前）→ `selection_retracted` → `candidate_selected` → `before_commit` → `commit` → `composition_end` 或 `composition_start` → `composition_update` → `candidates_shown` 或 `candidates_hidden`，focus 的 `focus_in`/`focus_out` 和 destroy 的 `session_end` 排在最后。事件不带文本，按值传递，`size_of::<LifecycleEvent>() <= 24` 由编译期断言守住，推导和按键路径不分配内存（`crates/host-api/tests/lifecycle_alloc.rs` 用只数当前线程的计数分配器断言）。

- `dispatch()` 的顺序：采样并发 `input` → `runtime.dispatch` → 原有统计 → 读事实 → `complete_transition` → `before_commit` 发出点 → 其余事件和内置消费者 → 序列化。`before_commit` 放在 `complete_transition` 之后，脚本将来看到的就是宿主拿到的字节，包括全角转换；尾巴按字符数而不是字节数记，全角把 1 字节的 ASCII 换成 3 字节的 U+FF01..U+FF5E 之后仍切在同一处。`before_commit` 目前只是内部发出点，不改写。
- 一次 transition 同时带着上屏和新组字（韩文音节延续、五笔顶字）时只发 `commit{continues}`，不发 end 和 start。
- focus(false) 的上屏一律不可改写、不出声；focus 的回复不带 `feedback`。
- 获得焦点的 `focus(true)` 只重置旧组字，不返回提交；runtime 在取消和刷新之前先丢弃 `phrase_preedit` 暂存词段及撤回记录，失败重试也不能补交。失焦的 `focus(false)` 则继续送出用户已经选好的暂存词段。两者共用的是组字结束事件，不能共用暂存词段的送出策略；只剩暂存段、读音已被 Ctrl+Backspace 删空时也一样。
- `apply_online_*`、`resolve/clear_online_candidates`、`apply_cloud_response`、`rerank_settled`、`apply_translations` 这些旁路调用只给核心发视图类事件。将来脚本也收不到它们，否则停顿时长和网络时序会经 `candidates_shown` 泄露出去。`set_english_mode`、`set_chinese_punctuation`、`set_punctuation_lock`、`update_preferences`、`set_ai_credential` 也按旁路调用推导：IBus 的 Ctrl+Shift+E 不先上屏就切专用英文，组字被丢掉；开关 AI 会在组字中途换掉 AI 座位。不推导的话组字的 end 就丢了。
- 派发、focus 和上面这些调用报错时照样推导（`HostSession::lifecycle_on_error`）：失焦先取消组字再刷新，刷新失败时组字已经没了；引擎报错前 runtime 已经刷新过；撤回暂存选择后重打可能失败。报错路径没有上屏，也不报 `candidate_selected`，focus 照样发 `focus_in`/`focus_out`。
- 命令 19 `MSIME_CANCEL_AFTER_HOST_COMMIT` 对会话同 `MSIME_CANCEL`，只在生命周期里把结束原因记成 `HostCommit`。
- 命令 20 `MSIME_FINISH_ON_FOCUS_LOSS` 对会话同 `MSIME_FINISH_COMPOSITION`，`input` 的类别记 `FocusLoss`，上屏来源记 `FocusLoss`，和 focus(false) 的上屏一样不可改写、不出声、不画（回复里 `feedback` 为 0）。它给宿主替失焦结束组字的路径用：macOS 的 IMK 在焦点移走或用户点进文档时先调 `commitComposition:`，组字在那里就上屏了，随后的 `setFocused:NO` 已经没有东西可上屏；只认 focus(false) 的话，「失焦上屏不出声」在 macOS 上大多数时候落不到实处。两个命令都经 `dispatch_call` 的 `host` 参数把类别交给 `InputClass::of`。

**选择加入走会话级 FFI，不进 HostOptions。** `msime_client_set_lifecycle(session, flags)` 不落盘，返回实际启用的位；`HostSession.lifecycle: Option<Box<LifecycleState>>` 只由它设上。`HostOptions` 带 `deny_unknown_fields`，每个输入法会话都会重读那份文件，往里加键会让仍在读它的旧版输入法再也开不了会话。另有三个按键路径调用：`msime_client_key`（每个物理按下一次，开启新的一轮；`PASSTHROUGH` 的键不开）、`msime_client_set_field_traits`（`SECURE`、`PRIVATE`、`FULLSCREEN`、`ELEVATED` 等，没有 `KNOWN` 位按未知处理，上屏不可改写；但 `SECURE` 和 `FULLSCREEN` 位本身仍让声音、特效和音乐关闭或静音，宁可多静音一次）、`msime_client_set_active`。顺序契约、PASSTHROUGH 例外和"Windows TIP 会话不调用它们"都写在 `msime_client.h` 里。

**内置消费者只在 `FEEDBACK` 位下生效。** 按键音和按键特效来自 `msime_client_key`，上屏音和上屏特效（typing_effect 事件 4）来自核心的上屏事件，经派发回复的 `"feedback":{"typing_effect":N}` 交给宿主画；背景音乐由 `set_active` 仲裁：进程级 `AtomicU64` 记所有者会话，只有所有者能开关，所有者被销毁时自动关闭，实际状态是"所有者 ∧ 激活 ∧ 非 SECURE ∧ 非全屏"，把 macOS 的 `MSIMEMusicOwner` 下沉到了核心。`FEEDBACK` 下本会话的 `key_sound`、`commit_sound`、`typing_effect`（0-4）和 `music_set_active` 变为空操作；事件 5（绕过按键音的删除）不经过 `msime_client_key`，核心看不到，仍由宿主调用。`SECURE` 字段里没有任何声音和特效，音乐关闭。`HOST_HOLDS_PIECES`（Windows 自己把部分选词暂存在前缀里）下，部分选词上屏的那一段还没进文档，消费者不为它出上屏音、不画上屏特效，整段送出的那次上屏再出，一次上屏仍只响一次；事件照常发（第 2 阶段 Windows 接入时补上，第 1 阶段只记录这一位）。每个消费者各包一层 `catch_unwind`，panic 只丢掉这一个事件、报告一次诊断，绝不落到 `response()` 那一层把整次上屏变成 `"internal runtime failure"`。

**分阶段**：第 2 阶段各桌面宿主接入并打开 `FEEDBACK`，带来五项可感知变化（IBus/Fcitx5 和 Windows 的按键音前移到引擎处理之前、Windows 上屏音从"已送达"提前到"已暂存"、Windows 进入密码框时音乐停止、macOS 失焦上屏不再出提交音、宿主合成的上屏不再算作已上屏），并补上 Windows TIP 到 Server 的字段属性事件；macOS（2a）已接入，Windows（2b）和 IBus/Fcitx5（2c）各自一个 PR。第 3 阶段加 wasmi 解释执行的观察类脚本 hook（`PluginKind::Script` 及其桌面 cfg 骨架、非桌面 `PluginKind::ALL.len() == 8` 的不变量测试都在那一阶段）；第 4 阶段接通 `before_commit` 改写和候选重排、注释、过滤；第 5 阶段是授权页；第 6 阶段是作者工具。脚本只在 macOS IMK 进程、Windows Server、IBus 引擎进程和 Fcitx5 addon 里运行，Windows TIP 和提权窗口里永远不运行；移动端零变化。

## macOS 接入（第 2a 阶段）

宿主是 `platforms/macos/src/input/InputController.mm`，FFI 经共享的 Objective-C++ 包装 `shared/apple/MSIMEClientSession.mm`（只有 macOS 编译它，iOS 不链接）。

- **选择加入**：`prepareSession` 新建会话后立刻 `setLifecycle:FEEDBACK`，再报告字段属性，然后才 `setFocused:YES`，核心从第一次聚焦起就有事件。会话在英文模式下激活时不建，没有会话就没有事件，与以前一样。
- **字段属性**：`syncFieldTraits` 把 `IsSecureEventInputEnabled()` 映射成 `SECURE`、英文模式映射成 `HOST_ENGLISH`，两项都读得到，所以带 `KNOWN`。每个物理按下之前比较一次，变了才调 `msime_client_set_field_traits`；激活时也报告一次。每次 `setFocused:YES` 之前（`prepareSession` 里新建或复用的会话，包括快照替换和英文模式之后 `_focusPending` 的那次，以及按键换了客户端时）不论变没变都由 `confirmFieldTraitsBeforeFocus` 再报告一次：第 3 阶段的脚本运行时要在每次 focus(true) 之后先确认字段属性（聚焦前的同值报告也算，或聚焦后的第一个键）才投递事件，只在变了时报告的话，复用会话的 `focus_in` 实际上到不了脚本。新建的会话也走这一处，建会话时不另报。安全输入的开和关不再由宿主去开关音乐，核心看到 `SECURE` 就关掉声音、特效和音乐。
- **按键**：`handleEvent:client:` 对每个物理按下（`!event.isARepeat`、不是本进程自己发的事件）先报属性，再调 `msime_client_key`，然后才进入派发。Command/Control 快捷键和英文模式不调 `msime_client_key`（照旧不出声），也就不开新的一轮，这是有意偏离契约的两处：它们以前就不出声，Linux 和 Windows 也一样；安全输入时照样调用，核心什么都不出。返回值就是这个键的打字特效，宿主原样排队去画，不再调 `msime_client_key_sound` 和 `msime_client_typing_effect`（0-3）。按下到达时还没有会话（词库维护刚结束，或快照恢复没留下会话）的那个键，在 `handleKeyEvent:` 里 `prepareSession` 建好会话后补报，仍先于为它做的派发。
- **上屏**：上屏音由核心在派发里出；上屏特效取自派发回复的 `"feedback":{"typing_effect":N}`，`apply:` 只在回复带着它、并且确实上屏时画。失焦（`setFocused:NO`）、语音转写的回复不带 `feedback`，所以失焦上屏和听写既不出声也不画。IMK 的 `commitComposition:`、词库维护关会话前（`releaseDictionarySessionIfQuiesced`）和「准备词库」关会话前（`prepareDictionary:`）这三处由宿主替失焦结束组字，改发命令 20：核心按失焦上屏处理，不出声，`feedback` 为 0。其余宿主发起的 `MSIME_FINISH_COMPOSITION`（切方案、开字符面板、开始语音、悬浮工具栏切标点）是用户点了菜单或按钮，照旧按普通上屏出声，和接入前一样。宿主不再调 `msime_client_commit_sound` 和 `msime_client_typing_effect(4)`。事件 5 macOS 本来就不调用，照旧留给宿主。
- **音乐**：`claimBackgroundMusic` / `releaseBackgroundMusic` 改调 `msime_client_set_active(true/false)`，由核心记所有者会话。宿主保留 `MSIMEMusicOwner` 这个弱引用，只用来决定哪个控制器在偏好更新、会话新建和词库快照替换会话之后重新报告激活：核心没有查询所有者的接口，而按 IMK 的顺序，上一个客户端的停用可能晚于下一个的激活，重新报告必须只由最后激活的那个发出。英文模式下激活的控制器还没有会话，它认领时核心里的所有者仍是上一个控制器的会话，而宿主这边所有权已经转走，上一个控制器迟到的停用不会再报告；所以没有会话的控制器认领时替上一个控制器的会话报告一次停用，否则音乐会在离开输入法之后一直放着（接入前同样如此，宿主那时直接调 `msime_client_music_set_active`）。
- **会话重建**：词库快照替换会话时，包装层在 `restoreLiveModes:` 里把选择加入的位和最后报告的字段属性恢复到新句柄上；激活不由包装层恢复，旧会话销毁时核心已关掉音乐，宿主在 `snapshotSessionReplaced:` 里由音乐跟随的控制器重新报告。
- **指针**：点选候选（`selectCandidate:`）、候选窗的翻页按钮（`changeCandidatePage:`）、面板的 `pageHandler` 和候选右键菜单的置顶、删除、固定位置（`candidateMenuAction:`）经 `reportPointerToLifecycle` 先比较字段属性、变了先报告，再调 `msime_client_key(MSIME_KEY_POINTER)`，不出声不画。单按 Shift、Control 或 Caps Lock 切换中英文时若有组字，先原样上屏（`MSIME_COMMIT_RAW`）：这是一次物理按下，却走 `flagsChanged`，不经过 `handleEvent:` 的按下报告，所以派发前同样按指针开一轮。`MSIME_KEY_POINTER` 是契约里唯一不出声的键类，修饰键单按以前就没有按键音；原样上屏的提交音照常由核心出。
- **宿主写入的上屏**：Ctrl+Enter/Option+数字写释义列、释义页选义项，这两处是宿主自己把文字插进文档后丢弃组字，从 `MSIME_CANCEL` 改成 `MSIME_CANCEL_AFTER_HOST_COMMIT`：对会话的作用一样（只取消一次，macOS 本来也只发一次），生命周期里结束原因记 `HostCommit`。

macOS 接入带来的可感知变化：

- 失焦上屏不再出提交音、不再画上屏特效（设计里的第 4 项），包括 IMK 经 `commitComposition:` 结束的组字。
- 全屏前台时，连击升档音照样排队。以前宿主给 `msime_client_typing_effect` 带 0x200 位压住它；`msime_client_key` 没有这个位，而 macOS 不报 `FULLSCREEN`（报了的话核心会把按键音、上屏音和音乐也一起静音，是更大的行为变化）。特效在全屏时照旧不画。
- 词库维护期间宿主关掉会话，核心随所有者会话销毁关掉音乐，等维护结束、下一次建好会话时才由所有者重新报告激活；以前音乐会在没有会话时继续放。

## 第 1 阶段的取舍

- **`Pinned` 比"整句改字钉住一段"更宽**：打开的候选列表里一次返回 handled 却不上屏的选择也记 `Pinned`，因为注音列表里的选字就是这样（`su3cl3` 打开列表后按 `3` 钉住"郝"而不上屏）；只限 `Select`、`AnyCandidate`、`Digit`、`Highlighted` 和 `SpaceOverList`。韩文汉字列表也是打开的列表，但它的选择上屏，所以仍是 `Committed`。
- **`DispatchFacts` 多出 `held_flushed`、`raw`、`retracted` 三项**：上屏来源的 `HeldFlush` 和 `Raw` 只看动作本身判断不出来，撤回也只有 runtime 知道。
- **命令 19 只取消一次**：方案里第一次取消只退一层的（韩文汉字列表、藏文、越南文），组字没结束时宿主照旧再发一次，与 Windows 现在对同一个键再发 `MSIME_CANCEL` 的做法一致。在 runtime 里另加一个"彻底丢弃"的动作会改 `Action` 这个公开枚举，本阶段不值得。
- **第 1 阶段不记录轮次计数**：轮次只用于第 3 阶段按轮计的 fuel 预算，现在存一个没人读的计数器只会留下死代码；契约已经写进头文件，宿主从第 2 阶段起照它调用。
- **`input` 事件要读派发前的本地模式**，为此 runtime 新增一个零成本的 `local_mode()` 访问器，不在按键路径上克隆整个 `View`。
- **`PluginKind` 的 cfg 骨架和非桌面 `PluginKind::ALL.len() == 8` 的不变量测试挪到第 3 阶段**：设计把它们列在第 1 阶段，但本阶段不加 `PluginKind::Script`，非桌面的 `ALL` 本来就是 8 项，这时写的测试守的是一个还不存在的 cfg 分支；和 `Script` 变体一起加，测试才第一次有东西可守。本阶段不动 `crates/client-core`。
- **零分配断言要量到内置消费者的上屏分支**，为此 `lifecycle` 导出一个 `#[doc(hidden)]` 的 `replay`：拿给定的采样和事实走一遍真实会话的 `LifecycleState::after`。集成测试是独立的 crate，`#[cfg(test)]` 的钩子到不了；在库的单元测试里装计数分配器又会把几千个无关测试都换到它上面。
- **设置类调用里只给可能在组字中途改动组字或候选的那几个接了推导**。`set_candidate_page_size`、`set_nine_key_mode`、`set_key_grid` 组字中会被拒绝或推迟，`set_character_width`、`set_paired_punctuation` 不碰快照，接上也不会多出事件。

## 第 2c 阶段：IBus 与 Fcitx5 接入

两个 Linux 宿主每建一个会话就 `msime_client_set_lifecycle(FEEDBACK)` 并立刻报告字段属性，旧的 `key_sound`、`commit_sound`、`typing_effect` 和 `music_set_active` 调用全部去掉。两边共用的、不依赖输入框架的那一半在 `platforms/linux/src/system/LifecycleFeedback.h`：属性换算、"变了才报告"、按键编码和读回复里的 `feedback`，由 `tests/input/lifecycle_feedback.cpp` 单测，调用顺序由 `tests/core/fcitx5_contract.py` 守住。

- **按键在引擎之前。** IBus 的 `process_key_and_count` 和 Fcitx5 的 `keyEvent` 先比较并报告属性、再 `msime_client_key`，然后才交给 `process_key` / `key()`。IBus 的会话由 `process_key` 惰性创建，词库维护结束后、或 `guarded` 出错关掉会话后，聚焦期间的第一个键到 `key_down` 时还没有会话；`key_down` 因此在判断会话之前先按 `process_key` 同样的条件 `open()`，和 Fcitx5 在 `keyDown` 之前跑的 `ensure()` 对齐，否则这个键既没有按键音，它引起的派发也落在一个没开过轮次的新会话上。原来按键音在引擎处理之后才出，而上屏音在引擎处理中就出了，于是上屏的那个键是"上屏音 → 按键音"；现在是"按键音 → 上屏音"。按键返回的连击数在引擎处理前就记下，引擎画的辅助行直接用它；连击数变了而这个键没经过渲染时，处理完再单独重画辅助行，和原来一样。
- **上屏音来自核心。** 上屏后连击数取自派发回复的 `feedback.typing_effect`；没有这个键的回复（focus、语音这些不是派发的调用）不动连击数。两个宿主的解析器只按键名取值，多出的 `feedback` 键原本就会被忽略，契约测试现在断言它被读取。
- **字段属性逐个标志映射，不复用宿主那个把隐私和不做拼写检查并在一起的布尔值。** IBus 的 `IBUS_INPUT_HINT_NO_SPELLCHECK`、Fcitx5 的 `NoSpellCheck` 只报 `NO_SPELLCHECK`，这类字段（终端、代码编辑器常这样标）从此照常出声、照常放音乐。IBus 的 `IBUS_INPUT_HINT_PRIVATE` 和 Fcitx5 的 `Sensitive` 都报 `SECURE | PRIVATE`，总带 `KNOWN`。密码、PIN、数字一类字段两个宿主仍然不建会话。原来的布尔值 `private_input` / `privateInput()` 照旧管学习、联网、剪贴板这些隐私行为，不动。输入框属性变了而会话不重建时（隐私与不做拼写检查之间切换），IBus 的 `content_type` 和 Fcitx5 的 `InputContextCapabilityChanged` 当场报告，音乐不等下一个键。
- **音乐只说激活与否。** `sync_music` / `syncMusic` 改调 `msime_client_set_active`，条件去掉了隐私判断，SECURE 交给核心。IBus 两个输入上下文先进后出时，后获得焦点的成为所有者，先失焦的那个再 `set_active(false)` 也关不掉它的音乐。设置里的 `plugins` 变了而本会话正激活着时，宿主再调一次 `set_active(true)`：进程里什么声音都没开时核心不起播放器，激活那一刻的音乐请求没人收到，而 `set_active` 对选择加入的会话总是返回 true，宿主原来那套"被拒就下次再说"的重试（`MusicActivity`）不再起作用；不重新声明，刚在设置里打开的背景音乐要等下一次换焦点才响。
- **轮次只给打字键开。** `msime_client_key` 只在原来会出按键音的那些按下时调用：会话在、中文输入开着、不在录音、不是松开、不是单独的修饰键、不是带 Ctrl/Alt/Super/Hyper/Meta 的快捷键。其余按下不开新的一轮，它们触发的派发算在上一轮里。顺序契约要求每个交给输入法的物理按下都开一轮，而 `msime_client_key` 没有"开一轮但不出声"的位（`MSIME_KEY_POINTER` 有这个效果，但拿它标键盘键是误报来源）；第 3 阶段按轮计 fuel 的脚本运行时（#7253）不会因为这类没有先调 `msime_client_key` 的派发熔断，宿主因此维持现状；在核心里加一个位还是宿主改用别的办法，以后再定。这个例外写进了 `msime_client.h` 顺序契约下面的"过渡期的例外"，不只留在这里：头文件是公开契约，按轮计数的使用方读的是它。IBus 屏幕键盘面板经 `panel_input_deliver` 直接调 `process_key`，原来不出声，现在也不开轮次；Fcitx5 的面板键走 `keyEvent`，原来出声，现在照样出声。
- **录音时不出声由宿主挡着。** 核心认得 `TRAIT_VOICE_ACTIVE`，但只按 `SECURE` 和 `FULLSCREEN` 静音。所以录音期间宿主不调 `msime_client_key`，并对音乐说"未激活"，保持原来的"录音时没有按键音和音乐"；这一阶段不报 `VOICE_ACTIVE`。按 Esc 取消录音的那个键因此也不出声：`key_down` / `keyDown` 在引擎之前运行，读到的是录音还在的状态；原来按键音在引擎处理之后才问，那时录音已经取消，这个键会响。不为它开例外：它是录音期间的键，取消录音的键不出声和录音期间不出声是同一条规则。
- **每次获得焦点之前确认一次字段属性。** 脚本运行时（#7253）在每次 `msime_client_focus(true)` 之后，要等宿主确认过字段属性才往脚本投递事件：focus 之前的一次 `set_field_traits`（值相同也算），或之后的第一个键。宿主"变了才报告"的去重会把这次确认省掉：IBus 同一个会话重新获得焦点、切换方案或中英文时属性没变，Fcitx5 切换方案时 `ensure()` 刚为新会话报告过又紧接着再 `focus(true)`，于是从获得焦点到第一个键之间脚本什么都收不到。两个宿主因此各只留一处 `msime_client_focus` 调用（IBus 的 `focus_session`、Fcitx5 的 `focusSession`），传 true 时先经 `FieldTraitsReport::confirm` 不去重地报告一次；传 false 不报告。
- **自动重复照旧出声**（设计待决问题 Q3），只是带上 `MSIME_KEY_REPEAT`，画出来不计连击。Linux 两个宿主这一条已被 [Linux 切换提示和输入法菜单对齐 macOS](../feature/2026-10-20-linux-mode-badge-and-menu-mac-parity.md) 取代：与 macOS 一样按住只在第一次按下出声，但每次重复借 `MSIME_KEY_POINTER` 开新的一轮，脚本 fuel 仍按每次按下给，`msime_client.h` 的过渡期例外随之写明。Windows 仍按本条。
- **指针点选和翻页先开一轮。** IBus 的 `candidate_clicked`、`page_up`/`page_down`、滚轮的 `cursor_up`/`cursor_down` 和托盘菜单「候选翻页」里的上一页、下一页（`property_activate`），Fcitx5 候选的 `select()` 和候选列表的 `prev()`/`next()`，都先报告属性再 `msime_client_key(MSIME_KEY_POINTER)`，不出声不画。
- **宿主写入译文后用命令 19。** 两个宿主各有两处先自己 `commit_text` 一条译文、再丢弃组字（`Ctrl+Enter` 只有一个义项时、从译文候选里选一个），这两处改发 `MSIME_CANCEL_AFTER_HOST_COMMIT`，核心把组字的结束记成 `HostCommit` 而不是取消。对会话的作用同 `MSIME_CANCEL`，用户听不出区别。

可感知的变化有五项：上屏的那个键先出按键音再出上屏音；只带"不做拼写检查"的字段开始出按键音、上屏音并放背景音乐；IBus 两个输入上下文交接焦点时音乐不再被先失焦的那个关掉；失焦时写出的上屏不再出上屏音（Fcitx5 在不支持预编辑的客户端里为韩文、注音、越南文、藏文写出的组字，IBus 经 `msime_client_focus(false)` 送出的暂存分段选词，原来都经宿主的 `apply` 响一声，现在核心按 `FocusLoss` 处理，同头文件"失焦上屏不出声也不画"）；录音中按 Esc 取消录音的那个键不再出按键音。隐私字段仍然完全静音。

## Alternatives considered

- **焦点的两个方向都送出暂存词段。** 能保留用户已经选择的文字，也能沿用同一条收尾路径；但获得焦点的入口既有契约是纯重置，送出只属于结束旧字段的 `focus(false)`。要保留旧组字的宿主应在获得焦点前显式结束它，不能把送出推迟到新的聚焦。
- **把事件推导放进 runtime，或让 `Transition` 多带几个字段。** 最强的理由是 runtime 最清楚"发生了什么"，放在那里不需要旁路接口。不采用：`Transition` 和它的 JSON 是六个宿主共用的契约，加字段就是改移动端也在读的格式；而且声音、音乐所有权、字段属性都是 host-api 的状态（`SessionSound` 和播放器都在这里），runtime 只负责组字编排。旁路的 `facts()` 让 runtime 只陈述事实，解释留给 host-api。
- **用 `is_idle()` 判断组字开始和结束。** 现成、所有宿主都在用。不采用：它把 `snapshot_valid` 也算了进去，一次失败的 refresh 会让一段并不存在的组字"开始"又"结束"；`has_composition()` 只看用户眼里的组字。
- **只靠 refresh 前后的快照比较判断视图变化。** 不需要 runtime 多维护两个计数器。不采用：高亮移动和翻页在 `dispatch` 里提前返回，根本不经过 refresh，`candidates_shown` 会漏掉它们。
- **改写点放在 `complete_transition` 之前，让脚本输出再经过全角转换。** 改写的字节少走一遍转换，脚本也不用关心全角。不采用：脚本看到的就不是宿主最终拿到的文本，pangu 一类插入的半角空格还会被转成 U+3000。
- **选择加入写进 HostOptions 或偏好文档。** 宿主少一次调用，设置也能随偏好走。不采用：`HostOptions` 拒绝未知键，旧版输入法读到新键就开不了会话；生命周期是宿主的接入能力，不是用户偏好，也不该随账号同步。
- **内置消费者在 `FEEDBACK` 关闭时也照常运行，旧调用继续工作。** 迁移期两条路都能出声。不采用：同一个会话里核心和宿主都出声就是响两遍，所以 `FEEDBACK` 下旧调用变成空操作，没开 `FEEDBACK` 的会话完全不受影响。
- **macOS 不报 `KNOWN`，或者只在确认过字段类型时才报。** 理由是 `IsSecureEventInputEnabled()` 是窗口服务器的全局状态，别的进程开着它也会让这里"看起来"在密码框里，算不上精确的字段属性。不采用：这个误差只会往"更安全"的方向偏（多静音一次），而 `KNOWN` 缺席会让第 3 阶段的脚本在 macOS 上永远不运行；IMK 没有别的途径知道字段类型。
- **macOS 也报 `FULLSCREEN`，让核心压住全屏时的连击升档音。** 和以前只压升档音的行为最接近。不采用：核心对 `FULLSCREEN` 的约定是声音全部静音、音乐关闭（对齐 Windows 的 `sound_allowed`），在 macOS 上就是全屏看视频时按键音和音乐都没了，比少压一个升档音的变化大得多；何况全屏状态要遍历窗口列表，只能异步读，赶不上"每个键之前"报告。
- **宿主去掉 `MSIMEMusicOwner`，激活一律交给核心。** 核心已经仲裁所有者，宿主少一份状态。不采用：偏好更新和会话重建之后要有人重新报告激活，而 `msime_client_set_active(true)` 会把所有权抢过来；没有宿主侧的记录，迟到的那个控制器就会把音乐从真正处于激活的客户端那里抢走。
- **宿主替失焦结束组字时用一个临时的生命周期位，而不是命令 20。** 像 Windows 的 `COMMIT_OBSERVE_ONLY` 那样派发前设上、派发后清掉，不占命令号，也不动 `InputClass`。不采用：位是会话状态，宿主漏清一次，之后所有上屏都静音；包装层还得决定快照替换时恢复不恢复它。命令只作用于这一次派发，和命令 19 是同一个形状。
- **只改笔记和 README，承认 `commitComposition:` 的上屏照旧出声。** 不动第 1 阶段的核心。不采用：macOS 上失焦上屏绝大多数走这条路，设计第 4 项和验收里的「失焦上屏无声」就等于没做到。
- **`msime_client_set_lifecycle` 失败时宿主退回旧的 `key_sound`/`commit_sound` 调用。** 多一层保险。不采用：它只在未知会话、错线程、重入或 panic 时失败，这些情况下其余调用一样不工作；两条路径并存还要多测一套组合。

## Consequences

[可选罗马字布局](../feature/2026-10-23-korean-romaja-layout.md)沿用这里的提交和组合生命周期，偏好只在空闲时更换引擎。

- 收益：macOS 的按键音、上屏音、打字特效和背景音乐由核心一处决定，失焦上屏的提交音这类宿主各自漂移的细节由核心统一；第 3 阶段脚本需要的轮次、字段属性和事件流在 macOS 上已经接好。
- 代价：macOS 在全屏前台时会响连击升档音；词库维护关掉会话的那段时间音乐停止。旧的 `keySound:`、`commitSound`、`setMusicActive:` 在包装层里留着，macOS 不再调用它们（`FEEDBACK` 下它们本来就是空操作）。
- 风险：Windows（2b）和 IBus/Fcitx5（2c）的 JSON 解析器能否容忍新增的 `feedback` 键要各自在接入 PR 里加契约测试（macOS 用 `NSJSONSerialization`，`session-smoke` 已经覆盖）；移动端目标的交叉编译由 CI 的平台线确认；IBus 两个输入上下文的焦点进出顺序未核实；第 2c 阶段已经让后激活的会话成为所有者，所以无论哪个顺序，音乐都跟着最后获得焦点的那个，但这仍要在真实桌面上确认。Linux 宿主不为修饰键、快捷键、松开、录音时的键和 IBus 屏幕键盘的键开轮次（见第 2c 阶段一节）：脚本运行时不因此熔断，但这些按下引起的派发记在上一轮里、用上一轮的 fuel，补上"开一轮但不出声"的位之前一直如此。

## Verification

- 焦点与暂存词段：`taking_focus_discards_held_phrase_while_leaving_focus_commits_it` 用合成词库经真实 C ABI 部分选词，覆盖剩余读音非空和已删空两种状态。获得焦点无提交，失焦送出暂存段，事件顺序为组字结束、候选隐藏（原先可见时）、焦点事件；旧候选失效，退格不能恢复旧读音，下一段只提交新文字。`taking_focus_discards_held_phrase_even_if_refresh_fails` 在 runtime 中注入快照错误，确认获得焦点先清掉暂存及撤回记录，下一次聚焦不补交。旧实现分别因返回旧词段和错误后仍有 `phrase_prefix` 失败。
- 暂存词段的焦点修复在 macOS 上运行 `cargo test -p msime-host-api -p msime-input-runtime --locked`：host-api 644 项单测、15 项集成测试和 runtime 297 项单测通过，另有 2 项按现有配置忽略；两项新增回归从旧实现失败转为通过。`cargo clippy -p msime-host-api -p msime-input-runtime --all-targets -- -D warnings`、`cargo fmt --all -- --check`、`pnpm run verify-notes` 和 `git diff --check` 通过；独立审查未发现可证实的问题。未做系统输入框或真机验收。
- 该焦点修复的 `cargo build -p msime-host-api --locked` 和 `bash scripts/verify-local.sh --quick` 通过：165 项 Windows 可移植宿主测试、Rust 工作区、Android 目标、两种 WASM 配置、Android Java 宿主、macOS 原生宿主与共享 Apple 桥接层构建通过。Windows 交叉构建、Linux 宿主与外壳、HarmonyOS 真打包因本机缺少前置环境跳过；Tauri 外壳因所需位置没有原生 bundle 跳过。
- 第 1 阶段：`cargo test -p msime-host-api` 和 `-p msime-input-runtime` 守着拼音、两条标点路由、韩文延续、韩文汉字 Esc、Windows 式"选词后 CANCEL"、命令 19、失焦和 destroy 的黄金轨迹，start/end 成对，SECURE 下零输出，全屏和失焦上屏不出声，lifecycle 为 `None`（以及选择加入但不开 `FEEDBACK`）时 transition JSON 逐字节相同，全角切分，必定 panic 的消费者不影响上屏，零分配，以及 `the_c_header_and_the_rust_exports_agree`；不新增依赖。
- 第 2a 阶段（macOS），全部在分支头上量：`cargo test -p msime-host-api` 全部通过，包括命令 20 的轨迹测试（同一段组字用命令 9 结束出提交音、画特效，用命令 20 结束上的是同一段文字，来源记 `FocusLoss`、不可改写、不出声、`feedback` 为 0）。在 Mac Studio 上按 `platforms/macos/README.md` 先 `CARGO_TARGET_DIR=target/macos-cargo cargo build -p msime-host-api`，再编 `target/macos-isolated`，跑 `ctest --test-dir target/macos-isolated -E '^shortcut$'`，148 项全部通过（rebase 到 2026-10-12 的 develop 之后 `settings-route-coverage` 失败，原因是 develop 上 #7225 新加的 `privacy` 设置页没有路由，在不含本分支的 develop 上同样失败），其中 `dictionary-quiesce-controller` 用真实会话走词库维护关会话前的命令 20；需要图形会话的 `shortcut`（SSH 会话里等偏好回调超时）把同一次构建的二进制拉回本机图形会话里单独运行通过。`shortcut` 覆盖调用顺序（属性先于按下、按下先于派发，指针点选、翻页按钮、面板的 `pageHandler`、候选右键菜单和单按修饰键的原样上屏同样先报属性再报 `MSIME_KEY_POINTER`，被拒绝的点击什么都不报告）、打开会话的那个键在会话建好后补报、没有会话时不报告也不画、键类、自动重复/松键/英文模式不报告按下、安全输入只改属性、上屏特效只取自 `feedback`、`commitComposition:` 发命令 20、命令 19、音乐归属（包括没有会话的控制器认领时替上一个所有者报告停用），并断言旧的三个声音调用一次都没有。`session-smoke` 用真实 host 库走一遍包装层的往返：按键音和上屏音开着（音量 0，不出声）时旧调用在选择加入之前排队、之后成了空操作，按键特效来自 `msime_client_key`、上屏写进 `feedback`、`SECURE` 下全为 0、命令 19 丢弃组字、失焦回复没有 `feedback`。`snapshot-activation-test` 用 Studio 上的一份桌面词库资源跑：第一轮里失败恢复（版本对不上）和真正激活之后，选择加入和 `SECURE` 都恢复到了新会话上；同一轮随后的 `snapshotVersion` generation 断言失败，未改动的测试在同一份资源上同样失败在这一行，与本分支无关，第二轮因此没跑到。评审补上候选右键菜单按指针开一轮、每次 `setFocused:YES` 之前确认字段属性之后，在 Studio 上重编 `target/macos-isolated` 跑 `ctest -E '^shortcut(-translations)?$'`，147 项全部通过（包括 `settings-route-coverage`）；`shortcut` 和 `shortcut --translations` 拉回本机图形会话运行通过，其中新的用例断言复用会话的聚焦和按键换客户端时的聚焦之前都紧挨着报告了属性，右键菜单每一项都是 `MSIME_KEY_POINTER` 紧接着派发。`cargo clippy -p msime-host-api --all-targets -- -D warnings` 只报 `crates/host-api/src/tests.rs` 里一处 develop 上已有的 `assertions_on_constants`，本分支的改动没有新的告警。
- 第 2c 阶段（Linux）：`platforms/linux/build-container.sh` 在容器里编过 IBus 引擎、Fcitx5 插件和全部单测，`ctest` 全绿，其中 `linux-lifecycle-feedback` 覆盖属性换算、只在变了时报告、每次获得焦点前不去重地确认、按键编码和 `feedback` 解析，`fcitx5_contract.py` 覆盖"属性 → `msime_client_key` → 引擎"的顺序、旧调用已经去掉、选择加入紧跟会话创建、每个宿主只有一处 `msime_client_focus`、指针轮次和命令 19。真机验收还没做：GNOME（IBus）和 KDE（Fcitx5）上顺序是"按键音 → 上屏音"，隐私字段（如浏览器隐私窗口）静音，只带不做拼写检查的普通字段照常出声，两个输入上下文交接焦点时音乐不断。
- 没有验证的：装到真机上手测（Safari 登录框里打字无声无特效、英文模式无声、失焦上屏无声、背景音乐随切换应用和密码框开关）。这台开发机的输入法是日常在用的，不能在上面装测试包。

相关：[Windows 打字特效对齐 macOS](../../implemented/feature/2026-10-10-windows-typing-effects-caret-overlay.md) 是第 2b 阶段 Windows 接入时要改的那套按键音、特效和 Aux 管道路径；[插件选择同步打开播放开关](../../implemented/bug-fix/2026-10-10-plugin-selection-playback-gates.md) 的播放开关在 macOS 上由这里的内置消费者读取。

第 3 阶段的执行器见 [桌面脚本插件的沙箱运行时](../../implemented/architecture/2026-10-12-script-runtime-sandbox.md)：脚本 ABI 的 `Hook` 编号沿用本篇 `LifecycleEvent` 的判别值。本篇推导出的事件如何按授权投递给脚本、哪些不投递，见 [脚本插件在输入法进程里](../../implemented/architecture/2026-10-12-script-plugins-in-the-input-method-process.md)；`before_commit` 的改写和候选变换见 [脚本插件改写上屏与重排、注释、过滤候选](../../implemented/architecture/2026-10-12-script-plugins-modify-commit-and-candidates.md)。
