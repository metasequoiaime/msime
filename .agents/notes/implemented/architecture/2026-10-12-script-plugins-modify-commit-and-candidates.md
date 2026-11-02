# Agent Note: 脚本插件改写上屏与重排、注释、过滤候选：派发里运行、按身份重放、隐藏的行降到末尾

Status: implemented

## Problem

观察类钩子接通之后（见 [脚本插件在输入法进程里](2026-10-12-script-plugins-in-the-input-method-process.md)），脚本还不能改变任何东西：`rewrite_commit`、`candidates_order`、`candidates_hide`、`candidates_annotate` 已经在 ABI 里，调用一律 `DENIED`。接通它们要回答几件事。

候选一侧：runtime 里有 7 个平行数组和 `engine_order` 座位映射，选中的必须是用户看到的那一行；引擎有自己按首位取词的路径（失焦上屏、14 键原样上屏），`engine_index` 对超出列表的座位原样透传，删掉一行就会让映射错位；固定位置的词、云和 AI 行、五笔四码唯一时第四键自动上屏的首位，是别的机制已经排好的座位；`rerank_settled` 和 `apply_online_*` 由宿主在打字停顿或网络回答到达时调用，这时运行脚本等于把停顿时长和网络时序交给脚本；多个插件同时改列表时，结果不能取决于谁先看到谁的改动。

上屏一侧：脚本看到的要是宿主最终拿到的字节（全角转换之后）；标点和顶字的尾巴只读；改写不能回灌引擎学习和 AI 上文，否则插件能借此污染用户词库；Windows 上 TIP 已经从自己的会话把文字写进文档的路径不能改写；改写的结果不能带进双向控制符、零宽字符、控制字符或别的断行字符；脚本失败时上屏照常。

## Decision

### 候选变换（input-runtime）

**内置排序收进 `order_presentation()`**：逐键重排、整句读法降位、在线候选座位三步，refresh、展开和 `rerank_settled` 三处都调它。插件变换排在它之后，只在桌面编译（`crates/input-runtime/src/transform.rs`，cfg 同 script-runtime）；移动端和网页引擎里同名的几个方法什么都不做，可见前缀就是整个列表。

**什么时候运行脚本。** `Runtime::set_candidate_transform` 挂上一个 `CandidateTransform`。`Runtime::dispatch` 在桌面上把一个标记置位再派发，标记期间的 `refresh` 和 `expand_cached_candidates` 调用变换，也就是运行脚本；派发 panic 时先放下标记再继续展开，之后的旁路调用不会误跑脚本。其余所有调用（`apply_online_*`、`resolve/clear_online_candidates`、`rerank_settled`、`all_candidates`、`apply_translations`、`set_caret`、设置类调用）不运行任何脚本代码，只按候选身份重放本组字内最近一次记下的操作。身份的细分规则见[重复候选的身份修复](../bug-fix/2026-10-12-script-duplicate-candidate-identity.md)。列表为空时记下的操作清掉，读取引擎快照失败而换上空占位快照时也一样；跳过的列表（见下）在派发里清掉记录。

**上屏只能观察时停下。** 宿主设着 `COMMIT_OBSERVE_ONLY` 时，host-api 经 `Runtime::suspend_candidate_transform` 把变换暂停：派发里不运行插件，旁路调用也不重放，记下的操作清掉，列表保持内置排序。Windows 上 TIP 在自己的会话里选词的列表（注音等 AlwaysInlinePreedit 方案）按那个会话的顺序选，那个会话从不运行脚本；Server 一侧重排或隐藏，显示的顺序就和数字选中的对不上。所以这类列表的每一次派发都要设着这个位，头文件里写明了。

**插件看到什么。** 窗口是内置排序之后、任何插件变换之前的前 20 行，带引擎原始的注释。每个插件看同一份窗口，交回的操作用窗口行号表达，runtime 立即把行号换成候选身份记下，再按授权顺序合成：重排是后面的插件优先（倒序收集点名的行，同一行只收第一次），隐藏取并集，同一行的注释后面的覆盖前面的。下一次派发里插件看到的仍是原始窗口，读不到任何插件的改动。

**怎么排。** 先定隐藏的行：被点名隐藏、不是锁定座位、引擎下标不是 0。锁定座位是固定了位置的词（`candidate_positions > 0`）、来源 2、3、12（云、AI、AI 占位）的行，以及五笔四码唯一（`wubi_unique_four_code`）时的 0 号座位。引擎下标 0 的行永远可见，所以至少有一行可见。可见行里锁定的座位不动，其余座位先放插件点名的行（按合成后的优先次序，锁定的行不认点名），再放没点名的行（保持原来的相对顺序）；隐藏的行按引擎原来的次序排在最后，末尾里的位置变动（落定重排提上来一行隐藏行之类）不改变座位顺序，`rerank_settled` 不会把它当成挪动去复位高亮、推进代数。得到的排列同时作用于 7 个平行数组和 `engine_order`，缺一个或重一个下标时什么都不动。

**可见前缀。** runtime 记下隐藏的行数，`view()`、`candidate_page_len()`、`translation_candidates()`、`all_candidates()`、翻页、移动高亮、展开的判断，以及 `Select`、`SelectAnyCandidate`、数字、空格、14 键原样上屏的座位检查，都只认可见前缀。空格、结束组字、两条标点路由和 14 键原样上屏直接按高亮的座位交给引擎，不再查座位，所以每次排好之后高亮都拉回可见前缀：派发里的展开会重新运行插件，插件那一次可以多藏几行，而翻页填满当前页时高亮原地不动，不拉回就会停在被隐藏的末尾。`engine_order` 仍覆盖全部候选，`engine_index` 的透传规则不变。

**跳过**：没有候选、平行数组长度不一致（`normalize_online_slots` 用的同一条守卫）、整句改字中（`conversion` 非空）、方案的候选列表打开着（`candidate_list_open`）。本地模式的白名单（U6）是插件一侧的规则，由 host-api 判断。

**注释另放一格。** 插件的注释在 `Candidate::script_annotation`，JSON 键 `script_annotation`，没有时不出现，所以没有插件时 transition JSON 逐字节不变；引擎的 `annotation` 不动。注释只显示在可见行上，跳过的列表不显示。只改了注释、没改顺序时也推进候选视图的 epoch，生命周期照样发 `candidates_shown`。

### 修改类 import（script-runtime）

动作只暂存在本次调用的上下文里，钩子正常返回（非负）时才交给宿主（`ScriptInstance::actions()`），trap、fuel 耗尽或返回负数时全部丢弃；每次投递开始时清空。约定写在 `abi::modification`：

- `rewrite_commit(src, len)`：只在 `before_commit` 里、`REWRITABLE == 1` 时可用，否则 `DENIED`；需要授予 `rewrite_commit`。区间越界 trap；超过 4096 字节或 `max(4 × 原 body 的 UTF-16 码元, 256)` 返回 `LIMIT`；不是 UTF-8、为空、含 `abi::is_forbidden_plugin_char` 拦下的字符（见下）返回 `INVALID`；再调一次覆盖。"原 body"指改写之前的原文：多个插件依次改写时，宿主在事件里带上原文的码元数（`CommitTexts::original_body_utf16`，插件读不到），后面的插件拿到的 body 已经被改过，上限仍按原文算，`LIMIT` 和宿主采不采用的判断一致。
- `candidates_order(src, count)`、`candidates_hide(src, count)`：只在 `candidates` 里可用。`count` 个小端 u32 窗口行号，越界、重复或多于窗口行数返回 `INVALID`。`order` 覆盖上一次，`hide` 取并集。锁定座位和引擎下标 0 由 runtime 静默忽略，返回值照样是 0。
- `candidates_annotate(index, src, len)`：只在 `candidates` 里可用，≤ 32 个 UTF-16 码元（超出 `LIMIT`），不含 `abi::is_forbidden_plugin_char` 拦下的字符（否则 `INVALID`），空串表示不加，同一行再调覆盖。
- 本地模式不在白名单里时窗口按屏蔽处理（`ABSENT`），三种候选修改都做不了。

`abi::is_forbidden_plugin_char` 拦这几类，只给插件用，不改 client-core 的 `text` 模块：

- Cc（`char::is_control`）和 Cf（按 Unicode 15.1 写死的码位表）。
- 行分隔符 U+2028 和段分隔符 U+2029。它们不是 Cc，却和换行一样断行，macOS 宿主就拿 U+2028 当释义的换行（`InputController.mm` 的 `MSIMEGlossLineSeparator`）；只拦 `\n` 不拦它们，插件照样能往上屏里塞换行。
- 不是 Cf、却看不见或只改前一个字字形的字符：U+034F（组合用字形连接符）、U+115F、U+1160、U+3164、U+FFA0（韩文填充符）、U+17B4、U+17B5（高棉文固有元音）、U+180B–U+180D、U+180F（蒙古文自由变体选择符）、U+2800（盲文空白）、U+FE00–U+FE0D（VS1–VS14）、U+E0100–U+E01EF（表意文字变体选择符）。它们能让一段看起来一样的文本在字节上不同，或者让一行看起来是空的；插件在 `/` 指令里给的行（见 [插件在自己的 `/` 指令里出候选](2026-10-13-script-plugin-command-rows.md)）也要过这一关。
- U+FE0E、U+FE0F（VS15、VS16）放行。这里是逐字符的判断，分不出它是不是跟在 emoji 后面，而 emoji 注释离不开 VS16（❤️ 是 U+2764 加 VS16）；单独出现的 VS15、VS16 不改变字义。

### 宿主一侧（host-api）

**共用一个脚本集合。** `LifecycleState` 持有 `Rc<RefCell<ScriptSet>>`，会话选择加入生命周期时把同一个集合包成 `ScriptTransform` 挂到 runtime 上（移动端什么都不挂）。生命周期一侧和 runtime 一侧都只在一次调用之内用 `try_borrow_mut` 借它，借不到就跳过，不 panic。`ScriptSet` 记下宿主最近一次报告的字段属性，runtime 调用变换时据此判断运行条件。

**`candidates`**：运行条件同观察类钩子（进程开关、选择加入、属性确认过且是普通字段），再加本地模式在白名单里。每个声明了 `candidates` 的插件收到同一份窗口（`page`、`highlighted` 缺失），单次上限 125 000 fuel，同时受每轮 125 000 的配额约束；失败计入熔断。两者相等，同一轮里 `input` 只要先花过 fuel，`candidates` 里的死循环就总是先撞上轮配额而不是自己的上限，所以被轮配额截断时按这一次自己用掉多少判定：不少于 62 500（单次上限的一半）就和超出上限一样算 fuel 耗尽的失败，计入熔断；不到一半才是不算失败的 `TurnExhausted`（见 [沙箱运行时](2026-10-12-script-runtime-sandbox.md)）。`candidates_shown` 照旧读视图：显示的顺序（重排和隐藏是用户看得到的），引擎的注释（插件的注释不进去）。

**`before_commit`**：派发类调用的上屏在 `complete_transition`（全角转换）之后先发 `before_commit`，再发 `commit`。`lifecycle::derive` 拆成三段（`derive_selection`、`before_commit_event`、`derive_rest`），`derive` 本身按原样串起三段、不改写；`LifecycleState::after_rewriting` 在 `before_commit` 和 `commit` 之间调用 `ScriptSet::before_commit`。每个声明了它的插件都收到这个事件，`REWRITABLE` 告诉它能否改写；改写只在可改写时采用。可改写的条件不变（`lifecycle::rewritable`）：来源是选中候选、原样上屏或暂存段送出，本地模式 none 且算打字，不是以词定字，没设 `COMMIT_OBSERVE_ONLY`，字段属性已知且不是 SECURE、PRIVATE、ELEVATED。另加一条：设着 `HOST_HOLDS_PIECES` 的会话里，部分选词（`SelectionOutcome::Partial`）的上屏不可改写，这次组字里有过部分选词之后，补齐整个词的上屏也不可改写（`LifecycleState` 记着，组字结束时清掉，经 `Context::pieces_held` 交给 `rewritable`）。"这次上屏是宿主暂存的一段"只有一处定义（`lifecycle::held_piece`：设着 `HOST_HOLDS_PIECES` 并且是 Partial），`rewritable`、`pieces_held` 的累计和内置消费者（#7239：暂存的一段不出上屏音、`feedback` 为 0）都用它；它在 `after_texts` 和 `after_rewriting` 共用的入口 `begin_after` 里每次调用重算一次。两者不是同一件事：补齐整个词的那次上屏不可改写（看 `pieces_held`），但整个词这时才写进文档，照常出声。合并 #7239 时它原先只在 `after` 里算，而派发走的是 `after_rewriting`，经 C ABI 的部分选词仍出上屏音、`feedback` 不为 0；这是改到共用入口的原因。Windows Server 的 `ReplyComposer` 自己暂存各段，补齐时写进文档的是暂存的前几段加上这次的一段：逐段改写看不到整个词，改了最后一段还会拼出原文的前几段加改写过的最后一段。这样的会话里 `commit` 事件也是按段投递的。

每次 `before_commit` 都在复位到 `msime_init` 之后快照的新实例上运行（见 [沙箱运行时](2026-10-12-script-runtime-sandbox.md)），只改 body：`DispatchFacts::split_commit` 按字符数从末尾切出 tail，改写后的 body 接上原来的 tail 写回 `transition.commit`。多个插件按授权顺序依次改写，后面的插件读到的 body 和 COMMIT 是前面改写过的；每次改写的长度上限都按原 body 算，超出的那一次不采用。`commit` 事件带改写后的字符数和 `rewritten`，观察类插件读到的是改写后的文本；`candidate_selected` 的选中文本仍是原文。runtime 在 `dispatch` 里已经按原文调过 `remember_commit`（AI 上文和引擎的上文），改写不回灌引擎学习。插件 trap、fuel 耗尽、返回负数或结果不合规时上屏用原文。

**`before_commit` 的 fuel 单独记。** 每个插件每轮给 `before_commit` 另一份 100 000 的配额，不从其余钩子每轮 125 000 的配额里扣，也不让它们扣；宿主在同一轮里重建实例时各份剩余都交回去（`resume_turn`，`command` 另有一份，见 [插件指令的行](2026-10-13-script-plugin-command-rows.md)）。共用一份时，持久实例在同一轮里先收到 `input`、`candidate_selected`、`candidates`，它按之前攒下的状态烧掉多少 fuel，就决定了快照上的改写会不会因为 fuel 耗尽被丢掉，改写结果不再只是本次 body 的函数。分开之后 `before_commit` 能用多少只取决于同一轮里之前的 `before_commit`，它们同样只看得到各自的上屏文本。被这一份截断时和其余钩子同一条规则：这一次用掉不少于 50 000（单次上限的一半）算 fuel 耗尽的失败，计入熔断，持久实例照用；不到一半是 `TurnExhausted`。

### Linux 宿主怎么画插件注释（IBus、Fcitx5）

两个宿主经同一个头文件 `platforms/linux/src/candidates/CandidateScriptAnnotation.h` 读 `script_annotation`，不碰引擎 `annotation` 的那条路：引擎注释照旧按辅助码开关、五笔编码提示和本地模式决定画不画，韩文汉字列表照旧把它当训音画；插件注释只要有就画，排在整行最后（候选、纠错标记、云和 AI 标记、固定位置、引擎注释、训音、译文之后），写成两个空格加六角括号 `  〔注释〕`。括号让它在不认颜色和字形属性的面板上也能把它和引擎注释（两个空格后直接跟文本）分开。插件注释不做繁体转换，那是插件自己写的文字。AI 座位的占位行（来源 12）不画。

- **IBus**：追加在 `render` 拼好的行文本末尾，皮肤给了序号颜色时用这一行序号的颜色（高亮行用高亮序号色）给它自己的区间加一条前景属性，追加在行颜色之后所以盖过它；没给时不加颜色，只靠括号。
- **Fcitx5**：`candidateRowText` 在译文之后 `append` 一段，只标 `DontCommit`（和训音一样只供显示，斜体留给训音和 AI 占位行）。选中用的 `FcitxCandidate::text()` 仍是引擎文本。

宿主在画之前再查一遍：合法 UTF-8、不超过 32 个 UTF-16 码元、不含 Cc、行和段分隔符、双向控制符（U+061C、U+200E、U+200F、U+202A–U+202E、U+2066–U+2069）、零宽字符和 BOM，不合规就整条不画、不截断。runtime 已经按完整的 Cf 表拦过（`abi::is_forbidden_plugin_char`），这一道只防契约走样时插件文本把候选行折断或把整行的方向翻过来；它只拦影响排版的那几类，不复制 Cf 表，也不跟着 runtime 追加上面那批填充符和变体选择符：宿主这一道的职责是契约走样时防断行和方向翻转，那批字符既不断行也不改方向，runtime 是插件文本的唯一权威，两边各维护一份只会漂移。macOS 的 `CandidateChrome.h` 同理。

### dev 构建里 wasmi 关掉 debug-assertions

根 `Cargo.toml` 的 `[profile.dev.package.wasmi]` 在 `opt-level = 3` 之外加 `debug-assertions = false`。wasmi 的构建脚本在 opt-level 2、3 时选尾调用分派：每条指令的处理函数尾调用下一条，靠 LLVM 优化成跳转。打开 debug-assertions 时这一步失效，每执行一条指令原生栈就深一层。实测（Mac Studio，`(loop (br 0))` 跑满单个钩子的配额，线程栈逐档缩小）：

| 构建 | `input`（25 000 fuel） | `before_commit`（100 000）、`candidates`（125 000） |
|---|---|---|
| dev，wasmi 带 debug-assertions | 1 MiB 能跑完，512 KiB 溢出 | 2 MiB 溢出，4 MiB 能跑完 |
| dev，wasmi 关掉 debug-assertions | 256 KiB 能跑完 | 256 KiB 能跑完 |
| release | 256 KiB 能跑完 | 256 KiB 能跑完 |

脚本在三个桌面平台的 release 构建里也运行（Windows 见 [Windows 授权页](2026-10-12-windows-script-grants-page.md)），而 Windows Server 的工作线程默认只有 1 MiB 栈。`tests::a_hook_running_out_of_fuel_fits_a_small_native_stack` 在 256 KiB 的线程上跑满三类钩子的配额，守住这一条。

## Testing

- `crates/input-runtime/src/tests/candidate_transform.rs`（桌面 cfg）用记下引擎下标的测试引擎和按预设交回操作的变换：重排、隐藏后 7 个平行数组和 `engine_order` 一起动，视图、分页和 `all_candidates` 只有可见行；用 `Select` 和数字选第 k 行，上屏和引擎收到的下标都是显示的第 k 行，被隐藏的座位的 ID 报 `StaleCandidate`；只剩引擎下标 0 可见时，结束组字、空格、两条标点路由、数字、移动高亮或翻页之后再选、以词定字都只交给引擎下标 0；引擎下标 0 隐藏不了但能挪动；固定位置的词、云和 AI 行不动；五笔唯一码的 0 号座位锁住、第四键自动上屏的是引擎首位；五笔顶字取显示的首行；多插件按授权顺序合成、第二次派发看到的仍是原始窗口和引擎注释；整句改字、打开的候选列表、平行数组不等长时不调用变换；`rerank_settled`（带一个偏向某字的测试模型）、`refresh_online_candidates`（所有 `apply_online_*` 的刷新）、`all_candidates` 的展开、`set_caret`、`apply_translations` 都不调用变换，只重放；派发里的展开会再调用一次，展开时插件多藏了几行、翻页又让高亮原地不动时，空格、结束组字和标点仍只交给可见的行；只改注释也推进 epoch；组字结束后记录清空；暂停时派发不调用变换、旁路调用不重放，恢复后下一次派发重新调用。
- `crates/script-runtime/src/tests.rs`：改写只在可改写的 `before_commit` 里、授予了能力时可用；换行、U+202E、U+200B、U+2028、U+2029、空串、非 UTF-8 被拒，256 个码元的上限和 4096 字节的上限，前一个插件缩短或放大了 body 时上限仍按原文算；持久实例把这一轮的配额烧到连一次 `input` 都放不下之后，`before_commit` 仍能用掉比剩余更多的 fuel、不扣其余钩子的配额，新一轮两份都补满；返回负数、trap、越界之后不交出动作，下一次观察类投递清掉上一次的动作；三种候选修改的暂存结果、坏行号、重复、超长和带 Cf 或 U+2028 的注释、错的钩子、没授权、本地模式屏蔽；Cc、Cf 和行、段分隔符的判定；不是 Cf 的填充符和变体选择符逐个经 `rewrite_commit` 和 `candidates_annotate` 得到 `INVALID`，跟在 emoji 后面的 VS16 两条路都放行，VS15、VS16 和各个范围两端之外的码位不算禁用；同一轮里 `input` 先花过 fuel 之后 `candidates` 死循环被轮配额截断，算 fuel 耗尽的失败；原生栈。
- `crates/host-api/src/tests/script_plugins.rs`：`input` 之后 `candidates` 死循环的插件每次聚焦失败一次，第 3 次被熔断停用（`out_of_fuel`）；两个插件依次改写、tail 原样接回、`commit` 观察者读到改写后的文本；第一个插件把 2 个码元的原文放大到 256 个，第二个插件的 600 个字母拿到 `LIMIT`，上屏停在第一个插件的结果；trap、返回负数、fuel 耗尽、改写成 `\n` 或 U+202E 都上屏原文；Windows 式在派发前后切换 `COMMIT_OBSERVE_ONLY` 时空格选词和回车原样上屏都只能看（`REWRITABLE == 0`，改写 `DENIED`），清掉后同样两条路径能改写，以词定字永远不能；设着 `HOST_HOLDS_PIECES` 时部分选词的那一段和补齐它的那一段都只能看，组字结束后的下一个整词照样能改写，不设时部分选词能改写；`tests/lifecycle_traces.rs` 的 `a_piece_the_host_holds_sounds_only_when_the_whole_phrase_goes_out` 经 C ABI（`FEEDBACK | HOST_HOLDS_PIECES`）选部分词：这一段没有上屏音、`feedback.typing_effect` 为 0，补齐整个词的那次出一次上屏音；Windows 式每次派发前后切换 `COMMIT_OBSERVE_ONLY` 时插件一次都不运行、数字选的是引擎顺序的首行，设着它时云候选到达也不重放之前的对调；SECURE、PRIVATE、ELEVATED 和未知字段不改写也不投递；经 C ABI 重排之后空格上屏显示的首行，`script_annotation` 跟着行走，读窗口的插件读不到别的插件的注释和顺序；隐藏的行不出现在视图里、数字选不到、引擎下标 0 隐藏不了；`apply_online_candidates` 和 `rerank_settled` 不运行脚本、重排按身份重放；本地模式不在白名单里时一个插件都不运行。
- Linux 宿主：`platforms/linux/tests/candidate/candidate_script_annotation.cpp`（`linux-candidate-script-annotation`）测共用的读取和检查：只读 `script_annotation`、占位行不画、32 个 UTF-16 码元的上限（含辅助平面字符）、折行和改方向的字符、坏 UTF-8 都整条不画、行尾那一段的格式；`platforms/linux/fcitx5/tests/native.cpp` 的 `scriptAnnotationRow` 用真实的 `FcitxCandidate` 查 Fcitx5 的行：辅助码设置关着也画、是单独一段 `DontCommit`、引擎注释和译文都在、韩文汉字行训音之后再画、选中的文本不变，它和纠错标记、训音两个行用例一起以 `fcitx5-native-test --rows`（`fcitx5-candidate-rows`）注册在构建门禁里，之前它们只在有已验证词库的验收运行里跑；IBus 的 `render` 离不开 IBus 对象，由 `tests/core/fcitx5_contract.py` 按源码检查它经同一个头文件读、追加在引擎注释之后、不经 `traditional_display`、用序号颜色只画自己的区间。

## Alternatives considered

- **过滤直接删行。** 最强的理由是视图、分页和选择 ID 不用区分可见前缀，代码更少。不用：`engine_index` 对超出列表的座位原样透传，删掉一行后后面每个座位都映射到别的引擎候选；引擎自己按首位取词的路径也看不到 runtime 删了什么。降到末尾、只认可见前缀让映射始终覆盖引擎的全部候选。
- **插件注释并进引擎的 `annotation`。** 最强的理由是宿主已经在画 `annotation`，插件注释立刻就能看到。不用：Linux 宿主只在本地模式或打开辅助码显示时才画它，韩文汉字列表把它当训音读；而且 `read_candidates` 读到的注释就会带着别的插件写的文本，插件之间多一条文本通道。代价是宿主要另画 `script_annotation`，在那之前插件的注释看不见。
- **`rerank_settled` 和在线候选到达时也运行 `candidates`。** 最强的理由是插件能看到云和 AI 候选、在停顿后按新列表重排。不用：这两类调用的时机就是打字停顿和网络往返，运行脚本就把它们交给了脚本（设计 §4.7）；按身份重放让插件已经表达过的意图照样作用在新列表上。
- **候选插件依次处理、后面的看到前面的结果。** 最强的理由是组合直观，和改写上屏一致。不用：结果取决于授权顺序里谁先看到谁，插件之间也多了一条读改动的通道；按原始窗口加身份合成，每个插件的输入都只是内置排序的结果。
- **多个改写插件都针对原文、最后一个生效。** 最强的理由是和候选变换对称。不用：两个都改写的插件（例如中英间距加标点替换）会悄悄丢掉前一个的结果；依次改写、长度上限按原文算，既能组合又不会层层放大。
- **不可改写时不投递 `before_commit`。** 最强的理由是每次上屏省掉一次重新实例化（几十到几百微秒）。不用：`REWRITABLE` 标量就没有意义，插件也无从得知一次上屏为什么没被改写；只声明了 `commit` 的插件不受这个开销影响。
- **`before_commit` 和其余钩子共用每轮 125 000 的配额。** 最强的理由是每个插件每轮的总量守在设计 §4.4 的 125 000，8 个插件合计不超过 1 000 000。不用：共用时快照上的改写能用多少取决于持久实例这一轮用掉了多少，持久状态借此决定改写留不留得下，大约每次上屏 1 bit；给 `before_commit` 固定 100 000、事后再从共用配额里扣，同一轮里有几次上屏就能拿几份，没有上限。
- **`candidates_shown` 也给插件变换之前的行。** 最强的理由是"读不到别的插件的改动"连顺序和隐藏都守住。不用：`page` 和 `highlighted` 指的是显示的列表，给变换前的行就对不上；顺序和隐藏是用户看得到的结果，插件注释这类文本仍然读不到。
- **wasmi 开 `portable-dispatch` 特性，不靠 LLVM 的尾调用优化。** 最强的理由是任何构建配置下原生栈都不随指令增长。不用：它改的是批准过的特性组合，release 也换成更慢的循环分派；关掉 dev 构建里 wasmi 的 debug-assertions 只影响开发构建，release 本来就没有这些断言。
- **Linux 宿主把插件注释接在引擎注释那一段里画。** 最强的理由是不用另算一段区间，代码最少。不用：那一段受辅助码开关和本地模式控制，韩文汉字列表还跳过它改画训音，插件注释接进去就会跟着这些开关消失或被当成训音；两段也就分不出谁是谁。
- **Fcitx5 用 `CandidateWord::setComment`。** 最强的理由是经典界面把 comment 画成次要样式，语义上就是注释。不用：插件声明依赖 `core:5.0.20`，Debian 12 构建镜像里的 5.0.21 没有这个 API（Ubuntu 22.04 的 5.0.14 也没有）。
- **IBus 放进辅助行（auxiliary text）。** 最强的理由是不占候选行的宽度。不用：辅助行只有一行，属于整页而不是某一行，现在还放着页码、拼音、本地模式和连击标签，看不出注释属于哪个候选。
- **宿主用 FSI…PDI 把插件文本包起来做双向隔离。** 最强的理由是从排版上彻底隔开阿拉伯文、希伯来文这类从右到左的文字。不用：面板对隔离符的支持不一（Qt 面板和缺字形的字体可能画成方框）；runtime 已经拦掉全部 Cf，宿主再拦一遍双向控制符，注释又在行尾，从右到左的字母最多影响它自己那一段。
- **超长或含禁用字符时截断后再画。** 最强的理由是用户至少还能看到一部分。不用：runtime 从不交出这样的文本，出现了就说明契约走样，截出来的前缀可能意思相反；整条不画更容易发现。

## Consequences

- 收益：脚本能改写上屏和重排、注释、过滤候选，选中的永远是显示的那一行，隐藏的行任何路径都选不到；旁路调用不运行脚本；插件之间不能经候选窗口读到彼此的改动；没有插件时 transition JSON 不变；dev 构建里一个钩子跑满配额只要几百 KiB 的原生栈。
- 代价：声明了 `before_commit` 的插件每次派发类上屏都要重新实例化一次，不可改写时也一样；候选变换每次派发多一次窗口构造和按身份的线性查找（最多 20 行）；dev 构建里 wasmi 自己的 debug 断言不再运行。
- `script_annotation` 由各宿主自己画，三个桌面平台都已接上：macOS 把它画在释义行的最后一行（斜体，见 [macOS 候选窗把脚本插件的注释画在释义行的最后一行](../feature/2026-10-12-macos-script-annotations.md)）；Linux 的 IBus 和 Fcitx5 在行尾画（见上面的 Linux 一节）；Windows 的 Server 候选窗接在引擎注释后面、换成次要色，不经 TIP 的管道（见 [Windows 候选窗](../feature/2026-10-10-windows-candidate-window-mac-parity.md)）。Linux 上插件注释让候选行变宽，横排时一页更容易超出屏幕；从右到左的注释没有做双向隔离；IBus 面板是否认区间前景属性由面板决定，没在真实的 GNOME Shell、KDE 和 ibus-ui-gtk3 面板上看过。
- `candidates_shown` 能看到别的插件造成的顺序和隐藏，构成插件之间约 1 bit 量级的顺序通道；文本（注释）不经这里传递。
- Cf 表按 Unicode 15.1 写死，之后版本新增的格式字符不在表里；ZWJ（U+200D）和标签字符是 Cf，含它们的 emoji 组合序列和旗帜不能出现在改写结果和注释里。表意文字变体选择符也拦下之后，改写类插件不能再输出带异体选择符的汉字（例如 葛 加 U+E0100）；仓库里的插件都不输出它。不用工作区已有的 `unicode-general-category` 按类别判断，是因为它对 script-runtime 是一条新的依赖边，要另外批准；批准之后 Cf 表和 Mn 类的变体选择符都可以改成按类别查。
- 改写不回灌引擎学习和 AI 上文，这一点由调用顺序保证（runtime 在 `dispatch` 里按原文记下，host-api 之后才改写），没有单独的测试；宿主之后照常做繁体转换和统计，作者文档要写明。
- Windows Server 在派发前后切换 `COMMIT_OBSERVE_ONLY` 是第 2b 阶段的宿主改动，这里只验证了核心按位行事；整句改字时的空格和回车走同一个位，没有单独构造整句改字的会话。
- 一个插件一轮最多用 225 000 fuel（其余钩子 125 000 加 `before_commit` 100 000），8 个插件合计 1 800 000，比设计 §4.4 的每轮合计 1 000 000 多；只有同时声明了 `before_commit` 并且每个钩子都烧满的插件才到这个数，按 wall-clock 的熔断照样管它。
- 时延目标（`before_commit` p99 ≤ 1 ms、候选变换 p99 ≤ 2 ms）还没在真机上量；dev 构建里 64 页全脏的快照复位已经量到 0.68 ms。

相关：[桌面端生命周期核心](2026-10-11-desktop-lifecycle-core.md) 推导出 `before_commit` 和可改写条件；[AI 座位的占位行](../../archived/feature/2026-10-11-ai-seat-placeholder.md) 定下了插件挪不动的 AI 座位，现在 AI 行坐第一页最后一格（[AI 候选坐第一页最后一格](../feature/2026-10-16-ai-candidate-last-slot.md)），插件同样挪不动、藏不了它。

快捷短语的光标后缀在脚本实际改写正文后失效，见[光标定位笔记](../feature/2026-10-31-quick-phrase-cursor.md)。
