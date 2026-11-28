# Agent Note: macOS 把整句联想模型挪出按键

Status: implemented

## Problem

macOS 0.60.0 上打字时偶尔「掉帧、小卡一下」。2026-11-16 这台开发机（M 系列，12 核）一天的诊断日志里，组字中的按键 `[key-latency] stage=handle` p50 19 ms、p90 56 ms、p99 360 ms，约 9% 的按键超过 60 ms；候选窗的 `build_ms` 首次弹出 p50 8–10 ms、之后 3–5 ms，慢键的时间大多不在绘制上。

原因是整句联想「增强」默认挂着的逐键模型（见[整句联想默认「增强」](../../implemented/feature/2026-11-02-keystroke-reranker-default-on.md)）。它在 IMK 的按键线程上同步跑：束搜索每一列给整句前缀打分（[模型进整句束搜索](../../implemented/feature/2026-11-03-model-in-lattice-beam.md)），最后再比较整池。挂不挂、池多大，都由一页自测决定：Mac 上 6x256 一页 1.3–1.5 ms，按[整句池系数](../../implemented/feature/2026-11-02-sentence-pool-by-device-speed.md)推算每键 p99 约 3.3 ms，于是配了最大的 32 条（[桌面小模型配整池](../../implemented/feature/2026-11-03-desktop-small-model-full-pool.md)）。这些数字都是 `rerank_latency` 只量 runtime 那一层得出的，句子之间不上屏、没有上文，而且是在模型进束搜索之前量的。

走宿主整条路径时差得很远。`rerank_replay` 经 host-api 的 C 接口、照这台机器真实的 macOS 选项，回放 typing-chat-v1 test 的 220 段、4902 键，每段都上屏：

| 配置 | 首选上屏 | 整键 p95 | 整键 p99 |
| --- | --- | --- | --- |
| 不挂模型 | 140（63.6%） | 2.3–3.7 ms | 6.4–16.5 ms |
| 6x256 配 32 条，挂在按键上 | 187（85.0%） | 9–105 ms | 24–266 ms |

挂在按键上的那一行，同一份计算量在不同时刻量出来差一个数量级。按模型调用拆开看，同样规模的一次图运行耗时相差两百多倍（每打分一个字 p10 3.2 µs、p99 50 µs、最慢 879 µs）：不是算得多，是在等。这台机器常年 CPU 空闲 0%（模拟器、编译、测试），按键线程在等 ONNX Runtime 的工作线程，被调度出去的那一刻就是一次卡顿。句子越长，每键要打分的前缀越多，越到句尾越卡。

## Decision

- **macOS 输入法在会话选项里写 `pause_rerank`。** `MSIMESessionOptions` 和 `phrase_preedit` 等宿主能力写在一起，选项文件里写 `false` 也照样打开。
- **桌面上写了 `pause_rerank` 的宿主，ONNX 模型一律延后到停顿**（host-api `defers_to_pause` 的 `PAUSE_HOST_DEFERS_ON_DESKTOP`）。手机上照旧按一页超过 `PAUSE_HOST_KEYSTROKE_PAGE_MS`（4 ms）才延后。挂不挂、挂哪一档、池多大的规则都不变，变的只是模型在什么时候跑。Windows、Linux 不写 `pause_rerank`，行为不变。
- **在打字的空隙里分片重解。** 会话挂着延后的模型时，`scheduleSettledRerank` 停顿 `kPauseRerankDelay`（40 ms）就调 `msime_client_start_settled_rerank`；其余会话（后台的 24M 落定模型）仍等 `kSettledRerankDelay`（150 ms），那个模型一次要一两百毫秒，提前起跑只会白算。宿主靠回复里的 `deferred` 区分两条路（host-api 的 `with_deferred`，`msime_client_start_settled_rerank` 和 `msime_client_finish_pause_rerank` 的每个回复都带），记在 `_deferredRerankSession`，换了会话就回到 150 ms，直到新会话的回复说明。返回 `continue` 时用 `kPauseRerankSliceGap`（1 ms）的计时器排下一片，run loop 先处理这期间到达的按键，按键最多等一片。返回里带 `view` 就应用，不只在 `moved` 时：每一片都推进候选代次，不收新视图的话，候选窗上画的是旧代次的身份，空格和数字键照它选词时会被会话当成过期而吞掉。
- **上屏首选之前把重解做完。** 新接口 `msime_client_finish_pause_rerank`：会话挂着延后的模型时，一片片把当前组字解完，代次前进就交回新 `view`；没挂时回 `{"status":"none"}`，不碰后台的落定重排。macOS 在空格和组字中的标点之前调 `finishPauseRerankBeforeCommit`，做完时只记下新视图、不写客户端，照会话里新的高亮项上屏，见[上屏前做完停顿重解时不碰客户端](2026-11-28-macos-pause-rerank-finish-key-order.md)。数字键和点选照用户看到的那一行上屏，不调。
- **延后的模型照样给上屏后联想排序。** runtime 的 `orders_prediction` 和 `order_prediction` 通过 `prediction_scorer` 认按键上的打分器，没有时认延后的那一个；`set_deferred_scorer` 同时下发联想候选池开关。不这样的话，把模型从按键上挪走以后，联想就不再按模型排。手机上延后的那一档不报在预算之内，照旧不排。

## 测量

都用 `rerank_replay`（这次改成照 macOS 的空格、按屏幕上高亮项的身份上屏，并数真正上屏的文字），6x256，整句池按自测定为 32 条，模型目录是安装包的 `Contents`。机器负载在 20 到 250 之间波动，延迟只在同一时段交替跑的几组之间比较。

上屏准确率：

| 集合 | 挂在按键上 | 延后到停顿 |
| --- | --- | --- |
| typing-chat-v1 test（220 段） | 187 | 186 |
| typing-chat-v1 dev + test（330 段，各跑两轮） | 275、275 | 276、276 |

两边不同的只有个别段，方向两边都有：「下次换我请」只有延后的对，「她说上次你修好的那个灯现在还好用」在 test 上只有挂在按键上的对，另外两三段两边都错、只是错法不同。逐键模式下束搜索接着上一键的存档往下解，停顿时从头解，代词这种只差一个字、靠细微分差决定的地方会走出不同结果；停顿模式每片的预算按墙钟算，两轮之间也会有单段的差别。总数上不比挂在按键上差。

上屏的那一刻（test 220 段）：

| | 首选上屏 | 字母键 p95 | 空格 p50 / p95 |
| --- | --- | --- | --- |
| 延后，停下等重解做完再上屏 | 186 | 2.9–17 ms | 0.1 / 0.2–3.6 ms |
| 延后，打完立刻上屏、不做完 | 140 | 2.8 ms | 0.1 / 0.1 ms |
| 延后，打完立刻上屏、先 `finish` | 186 | 3.8 ms | 26 / 342 ms（负载 245） |

第二行说明上屏前的 `finish` 不能省：停顿重解没做完就上屏，首选与不挂模型时一样。第三行是最坏的情形：打完最后一个字母马上按空格，整段的模型计算全落在空格上。

照打字节奏回放（`rhythm`：每键之后留 143 ms，与 macOS 打字驱动相同；宿主在空隙里停顿多久起跑分片；一片超出空隙的部分算作下一键要等的时间；空格前先 `finish`），test 220 段，负载 6–20：

| 起跑的停顿 | 首选上屏 | 字母键 p50 / p95 / p99 | 空格 p50 / p95 / p99 |
| --- | --- | --- | --- |
| 150 ms（`kSettledRerankDelay`） | 187 | 1.0 / 5.1 / 18 ms | 19 / 280 / 698 ms |
| 40 ms（`kPauseRerankDelay`） | 188 | 0.9 / 3.5 / 11 ms | 0.15 / 0.31 / 0.60 ms |

紧接着交替再跑一轮（负载升到 125）：150 ms 空格 p50 / p95 / p99 19 / 111 / 355 ms，40 ms 是 0.16 / 0.36 / 1.5 ms，字母键 p95 两者都在 3.5–4.1 ms，首选仍是 187 和 188。

150 ms 比按键间隔长，打字途中重解从不开始，整段的计算都压到空格上，只是把卡顿从字母挪到空格。40 ms 起跑时重解在每个空隙里做掉一部分，分数备忘在同一段组字里保留，到空格时几乎不剩；字母键要偶尔等一片，量出来却不比 150 ms 时慢；两组是先后跑的，负载不同，字母键的这点差别不当作结论。

一次停顿重解（负载 20–40 时）p50 14–18 ms、p95 35–83 ms，每次 2–4 片，停顿后第一个键 p95 0.8 ms。

## Alternatives considered

- **缩小整句池或换大模型配 9 条。** 最强的理由是改动最小，按键上照旧有模型。6x256 配 9 条整键 p95 4.9 ms，首选 186；8x448 配 9 条 p95 10.7 ms，首选 183。都是靠降准确率换延迟，而且在机器忙的时候照样被放大。没有采用。
- **ONNX Runtime 改单线程或关掉线程池自旋。** 最强的理由是正好对着「按键线程等工作线程」这个机理，不改任何行为。在负载 100–200 时交替跑了 2 线程自旋、2 线程不自旋、单线程：三者整键 p95 118–165 ms，没有一个明显更好，机器满载时什么配置都慢。没有采用。
- **只改系数表，让自测按真实开销推算池子。** 能让池子贴着预算，但预算挡的是平均意义上的开销，挡不住机器忙时的成倍放大；按真实开销定，Mac 上会落到 9 条，就是第一条。
- **把模型放到工作线程上、按键来时中断。** 按键最多等几毫秒，又不降准确率。会话按线程存放，引擎在会话里，要先把引擎拆出来；[鸿蒙那篇](../../implemented/feature/2026-11-04-harmony-pause-rerank.md)也因此没做。
- **上屏时不做完，照屏幕上那个上屏。** 不会让空格变慢。手快的人上屏的就是不带模型的首选，准确率掉回 63.6%，违背这次改动的底线。

## Consequences

- **收益**：字母键上不再跑模型，整键延迟回到不挂模型的水平，机器忙时也不再被模型放大；首选准确率与挂在按键上相当（330 段上 276 比 275）。上屏后联想仍按模型排。
- **代价**：
  - 停顿后候选会换一次位置（与鸿蒙、安卓相同）。
  - 打完最后一个字母不到 40 ms 就按空格时，剩下的重解落在空格这一键上，最坏时与挂在按键上的长句尾键相当。照 143 ms 一键的节奏打，空格 p99 0.6 ms。会话的第一段组字还没从回复里学到 `deferred`，按 150 ms 起跑，那一段的空格可能重一些。
  - 字母键遇上正在跑的那一片时要等它，一片约 8 ms 的模型时间，忙时更长。
  - 停顿 40 ms 起跑意味着打字时后台一直在做模型计算，CPU 用量和挂在按键上时相当，只是不再挡着按键。
- **接口**：`msime_client_finish_pause_rerank` 是新的 C 接口，`MSIMEClientSession` 加了 `finishPauseRerankWithError:`。鸿蒙、安卓的键盘上屏前不调它，手快时它们上屏的是哪一个首选没有量过。
- **重访信号**：真机上空格或字母键的体感延迟比上表明显差时，先看起跑的停顿和一片的长短；Windows、Linux 出现同样的按键卡顿时，照 macOS 写 `pause_rerank`、按 `deferred` 选起跑的停顿，并在上屏前调 `msime_client_finish_pause_rerank`。

## Verification

- `crates/input-runtime/src/tests/prediction_rerank.rs`：`a_deferred_scorer_still_orders_prediction`（延后的打分器上屏时排联想，换词库代次后也排）、`a_slow_deferred_scorer_leaves_prediction_as_without_one`（不报在预算之内时与没挂逐行相同）。
- `crates/host-api/src/tests/deferred_candidate_identity.rs`：`finishing_the_pause_rerank_hands_back_the_view_to_commit_from`（没挂延后的模型时回 `none`；打字时模型不跑；`finish` 做完重解、代次前进、交回视图；再调立即返回不再跑模型；照新代次上屏的是视图里的首选）。`crates/host-api/src/tests.rs` 的 `only_a_pausing_host_defers_a_model_too_slow_for_keys` 改成桌面上一律延后、手机上按 4 ms。
- `platforms/macos/tests/input/ShortcutTest.mm` 的 `TestSessionOptions`：会话选项带 `pause_rerank`，选项文件写 `false` 也照样打开。
- Mac Studio 上 `cargo test -p msime-input-runtime --lib` 484 项全部通过；`cargo test -p msime-host-api --features onnx-rerank --lib` 1182 项通过、6 项忽略，失败的一项是 `dictionary::tests::a_lookup_keeps_the_users_japanese_mixing`，它单独连跑三次也失败一次，查的是日文混输，与这次改动无关；`clippy -D warnings` 干净。
- 测量见上，复现：`cargo build --release -p msime-host-api --example rerank_replay --features onnx-rerank`，`rerank_replay <runtime-options.json> <cases.json> <模型目录> <on|pause|rush|rhythm>`，cases 由 `typing-chat-v1.tsv` 的 test 或 dev+test 按标点切段生成。
