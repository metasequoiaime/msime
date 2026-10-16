# Agent Note: 引擎网址模式补全邮箱后缀、网址开头和顶级域名

Status: implemented

## Problem

产品要求：打 `@` 时候选里联想主流邮箱后缀；打 `http` 之类时候选里给出 `http://`、`https://` 等补全。要在引擎里做，让每个平台的候选栏不改宿主就能显示。

此前网址模式只在组字原文恰好是 `www`、`http`、`https`、`ftp` 且按下 `.` 或 `:` 时进入，模式里没有候选，只有一行显示整段网址的兜底行。组字后按 `@` 走标点路线：runtime 先结束组字上屏首选，再写 `@`，引擎没有机会把它当成邮箱。只有 Android 在系统标记的邮箱输入框里由宿主自己给后缀（[Android 邮箱后缀](2026-10-09-android-email-suffixes.md)）。

## Decision

### 进入邮箱

全拼、双拼、五笔（`SchemeType::detects_urls`）里，组字原文非空、只有 ASCII 字母和数字、光标在末尾、不在本地模式或专用英文时，`@` 把组字转成网址模式，预编辑是 `<原文>@`。规则在 `local::url::opens` / `entry_keys`：触发词表 `TRIGGERS` 不变，邮箱的键单独是 `EMAIL_KEY`。组字时 `spelling_symbols` 因此多一个 `@`（`www` 发布 `.@`，`ni` 发布 `@`），runtime 才会把 `@` 当字符送给引擎，而不是先结束组字。带撇号的原文（`ni'hao`）不算，`@` 照旧结束组字。空闲时的 `@` 仍是名单模式的入口，不变。

退格的逆操作扩展到 `@`：删掉末尾的 `@` 退回原来的组字。退回现在要求删掉的是末尾那个字符（`caret == text.len()`），`abc@def` 删掉中间的 `@` 留在网址模式；进入网址模式总是在末尾追加一个键，所以只有删末尾才是逆操作。

### 网址模式里的候选

列表的第一行始终是用户打的整段网址（兜底行），补全排在它后面：

- 邮箱后缀（`url::email_completions`）：预编辑是 `<本地部分>@<已打的域名>` 时，给出以 `@<已打的域名>` 开头（不分大小写）的后缀，按 `EMAIL_SUFFIXES` 的顺序；已打的域名已经是完整后缀时不给。规则与 Android `EmailSuffixPolicy.match` 相同，后缀表是同一份、同一个顺序，`scripts/test-email-suffixes.py` 核对。选中即上屏整个邮箱并结束网址模式。
- 顶级域名（`url::tld_completions`）：预编辑以 `www.` 开头或含 `://`，正在打的主机名（去掉可选的 `www.`）非空、只有字母数字和 `-` 时，给出加上 `.com` `.cn` `.net` `.org` `.com.cn` 的五行。选中不上屏：以它为新预编辑留在网址模式，用户还可以接着打路径。

整段网址留在第一行，是因为空格、失焦和结束网址的标点都上屏高亮的那一行（默认第一行）。补全排在前面的话，`https://localhost` 按空格会变成 `https://localhost.com`，失焦时也会悄悄补上 `.com`。补全只在明确选中（点按、方向键加空格）时才用。

数字在网址模式里是输入，不选候选：`SPELLING_SYMBOLS` 已经列出数字和 Shift+数字行的全部符号，引擎收下数字，runtime 和宿主按 `spelling_symbols` 不把它们当选词键。补全显示着时也一样，选补全靠点按或方向键。

### 组字里的网址开头

组字原文是 `https`、`http`、`www`、`ftp` 的前缀且至少两个小写字母时（`ht`、`htt`、`http`、`https`、`ww`、`www`、`ft`、`ftp`），混排列表里接上 `https://`、`http://`（`https` 只给 `https://`）、`www.`、`ftp://`（`url::scheme_completions`）。第一行是词典、用户词典或英文词典的词时补全排在它后面（五笔 `www` 的“众”仍是首选），否则排在最前面。选中补全不上屏，以它为预编辑进入网址模式（`InputSession::url_continuation` → `enter_url_mode`）。

结束组字（`finish_composition`：标点、失焦、宿主的结束键）时高亮停在补全上不算选中：上屏补全之外的第一行，没有就上屏原文。五笔四码 `http` 不加补全，因为 runtime 的顶字在第 5 个字母上选第一行，补全领先时 `http` 加 `s` 会变成 `https://s`，而 `http` 加 `s` 本来直接进入网址模式。

### 行的来源和重排

所有补全行的来源是 `CandidateSource::Fallback`，不是 `Generated`：runtime 的整句读法降位（`runner_up_order`）把第一个 `Generated` 行的字数当作词网格整句的宽度，同宽的补全会被当成整句挪位，混排列表里真正的整句读法也会因此认错宽度。不新增来源值：来源是宿主 ABI 的数字，新增一个要改各宿主和 `candidate-sources` 契约，而这些行在宿主看来就是普通候选。补全行的 `candidate_answers_key` 为假（补全不是回答了按键，而是越过它），重排模型不拿它们比较，也不会把它们提到前面。补全行的识别不靠来源单独判断，而是按当前原文、预编辑重算一遍补全再比对（`is_url_completion`、`url_continuation`），不在会话里另存状态。

### Windows TIP

TSF DLL 在 Server 回答前就要给按键分类，`platforms/windows/tsf/Global/LocalModeKeyPolicy.h` 保留了网址进入规则的拷贝。这份拷贝同步加上邮箱的键（`UrlEmailKey`、`IsEmailLocalPartAndKey`），`UrlModeAfterDeletion` 同样只在删末尾时退回；`scripts/test-windows-expression-symbols-parity.py` 多核对一项 `EMAIL_KEY` 与 `UrlEmailKey` 相同。

### Windows 按键分类的状态归属

TIP 的宿主会话处理输入时绕过 `AddVirtualKey`，旧键击缓冲可能一直为空。键盘编号或 TSF 内置候选列表选中网址开头、顶级域名后，宿主预编辑会直接替换。`CCompositionProcessorEngine::GetKeyCompositionState` 因此提供只读快照：有有效宿主时按其 `editing_text`、`caret_position` 和 `local_mode` 读取原文、光标、Unicode 与网址标记；宿主视图读取失败返回空状态，不能复用另一份旧会话。没有宿主会话的旧路径才从键击缓冲构建快照。中文方案和网址、Unicode 等本地模式的 `editing_text` 是 ASCII 原文，光标下标与 Windows UTF-16 一致；越南文、藏文的原文可含非 ASCII 字符，目前光标位于末尾，沿用按 UTF-16 长度钳制的位置。

普通按键分类和排队按键的初始投影共用快照。投影已有效时仍按队列里尚未执行的按键推进，不用实时视图覆盖未来状态；队列完成后失效，下一次从宿主当前视图重新建立。宿主候选选择后不需要额外复制或修改旧缓冲。

候选编号、空格转换、候选完成、回车完成和韩文汉字键可能继续组字，入队后置 `_deferredKeyClassificationPending`。此后普通可打印字符及无 Shift/Ctrl/Alt/Win 的编辑键只捕获原译文、修饰键、Caps Lock/Win 状态、上下文、焦点代次与前台窗口；不推进未知结果后的未来投影。到 `_DrainOneDeferredKeyDown` 真正轮到该键时，`_ResolveDeferredKeyClassification` 忽略全部未来投影，按当前宿主快照分类。IME 操作继续原派发路径，应用文字继续原直接插字路径，Backspace、Escape、Enter、Tab 和方向等应用编辑键复用[应用键确认屏障](../bug-fix/2026-10-16-windows-application-key-replay-order.md)。分类成功后清本键标记，transport 重试不重新分类；恢复检查点生成的强制 INPUT/MOVE 始终保持原语义。待分类状态保持至队列耗尽或 clear；独立应用键槽尚在等待时同样捕获后续键，避免最后一个应用键移交后出现空档。

重分类与异步写字在 COM 调用前保留原上下文，并在查询后复查准确槽身份、代次和原前台窗口。`GetSelection` 返回到 `SetText` 之前再查一次。离线没有 worker Ready 令牌也必须通过线程焦点、顶层 context 和原窗口核验，不能仅凭 fallback 放行；在线另外保留 Ready 令牌检查。AltGr 先归一化再判断是否普通字符，分类使用捕获的 Caps/Win 而非执行时按键状态。


### Windows 断线恢复检查点

`_RefreshDeferredRecoveryPrefix` 同样从 `GetKeyCompositionState` 读取原文和光标。宿主按键路径绕过旧键击缓冲，队列耗尽时和 transport 重置前若仍从旧缓冲压缩历史，会把非空组字变成空检查点；旧缓冲残留文字时还可能恢复已经结束的组字。检查点读取当前快照后仍按原有规则生成字符和左移事件，保持容量检查、焦点代次、上下文引用与重试顺序。`CommitsOnBlur` 方案（韩文、注音、越南文、藏文）不生成检查点：`_RequestLocalSessionReset` 已经把现有组字上屏，重放会重复输入；每次刷新仍先释放旧检查点。

选择完成后即使队列里仍有后续键，`_CompleteDeferredKeyReplay` 也对不能直接保留为恢复事件的操作刷新检查点。数字选词和空格选词可能只补全网址或钉住一段，并不一定清空组字；一律删除已执行前缀，会让下一键失败重试或 transport 重置时丢失选择后仍在组字的原文。完成回调读取此时的宿主快照，替换旧检查点；完整上屏后快照为空，自然清掉历史。原有输入、移动事件及 finalize-and-input 的压缩规则不变。刷新先释放旧引用，新事件保留同一上下文与焦点代次；重连时检查点排在未处理尾键之前，重复 transport 通知不会再次插入。

检查点仍不是引擎会话的序列化。经过选词、整句改字或进入本地模式后又删除触发前缀的状态，未必能从原文重放得到；越南文、藏文的非 ASCII 原文也不等于可重放的 ASCII 按键。需要完整恢复这些状态时，应单独设计会话恢复契约，不能据此宣称重连保留了全部组字语义。

## Alternatives considered

- **补全排在整段网址前面（第一行就是 `zhangsan@qq.com`）** — 最贴近“打 @ 就联想后缀”的直觉，按空格直接得到最常用的后缀。但空格、失焦和结束网址的标点都上屏第一行，顶级域名补全领先时 `https://localhost` 会被改成 `.com`，打一半的域名也会被替换，这是悄悄改用户打的字。网址模式里数字不能选词，键盘上的选择本来就要靠方向键，让默认行保持用户打的内容更稳。
- **新增 `CandidateSource::UrlCompletion`** — 语义最明确，选择、重排和宿主都能按来源识别。代价是来源数字是宿主 ABI：macOS、Windows、Android、iOS、鸿蒙、Linux 都按数字做事，`scripts/test-candidate-sources.py` 和 Windows 的右键菜单策略也要跟着改。这些行在宿主看来与普通候选没有区别，引擎按原文重算就能识别，不值得为此动 ABI。
- **用 `Generated` 作来源（与组字里插入的日期时间行一样）** — 最省事。但 `Generated` 是 runtime 整句读法降位的依据（`LATTICE_SOURCE`），补全行会被当成整句读法挪位，同一列表里真正的整句读法也会认错宽度。
- **选中顶级域名直接上屏** — 实现最简单，与邮箱后缀一致。但网址后面常常还要接路径（`github.com/...`），上屏后就接不上了；留在网址模式再按空格上屏只多一步。
- **`@` 只在原文不是完整拼音时才进入邮箱** — 能保住 `xiexie@` 这类打完拼音直接接 `@` 的旧行为。但邮箱的本地部分本来就常是拼音名（`zhangsan`），分不出来；误进入后退格一次就回到原来的组字，可以选回汉字。
- **在候选选择回调里同步旧缓冲** — 可以让旧分类代码不变，改动集中在候选选择路径。但键盘输入本来就绕过旧缓冲，删除、取消、模式切换也需要维护同样的同步；两份可变组字容易再次漂移。分类时读取拥有组字的会话，队列只维护尚未执行按键的投影。
- **选择成功后直接保留旧检查点** — 可以省掉一次快照读取，也保留选择前的拼写。但网址补全会整段替换原文，部分选词会缩短原文，旧检查点已经不代表当前组字；按旧原文恢复会撤销已成功的选择。读取完成后的快照，并让空快照处理完整上屏。
- **选择入队后立即作废投影并读实时状态** — 实现最短，也不用增加每键分类标记。但更早的排队字母可能还没执行，实时状态落后于这次选择；直接读取仍会漏键。保留已知投影，未知结果后的普通键只捕获，轮到执行再分类。
- **选择投影永远当作继续组字** — 能保住网址补全后的回车和日语连续空格。但完整上屏后的编辑键仍会被当作 IME 操作而吞掉，必须根据实际结果区分。
- **保留全部已执行按键作断线恢复历史** — 能保留仅靠原文无法表达的部分操作顺序。但历史会持续增长，重放已经上屏或只适用于旧候选代次的动作也不能保证还原会话；现有有界检查点继续压缩为当前原文与光标，修正其读取来源与选择完成后的刷新时机，完整会话恢复另行设计。

## Consequences

- **收益**：全拼、双拼、五笔的所有平台在组字后打 `@` 都有邮箱后缀候选，打 `ht`、`www` 之类有网址开头的补全，打主机名时有顶级域名补全，共享引擎统一生成补全，Windows 同步按键分类规则并从宿主快照读取分类状态。后缀表与 Android 宿主是同一份，由脚本核对。
- **代价**：拼音后直接按 `@`（`xiexie@`）不再上屏“谢谢@”，而是进入网址模式；退格一次回到组字。组字时 `spelling_symbols` 多一个 `@`，快照多一次分配（两处分配预算测试各加一）。`ww`、`ft` 这类短组字的候选里多一两行补全。Windows 普通分类、队列初次投影和恢复检查点各增加一次宿主视图读取及解析；选择完成且仍有尾键时也读取一次快照，检查点长度仍受原有容量上限约束。
- **Windows 验证边界**：TIP 从宿主快照读取当前原文和模式，宿主选择补全后的数字、句点不再受旧缓冲影响。便携 C++ 回归覆盖状态读取及网址键分类；真实 Windows 编辑器中的选择与后续按键仍需设备验收。外部 Server 候选窗的无上屏选择通过[继续组字协议](../bug-fix/2026-10-16-windows-candidate-continuation.md)交给 TIP 自己的宿主会话执行：双方协商能力后，核对前后组字状态并按候选文本解析本地身份；旧 DLL 组合仍保留旧路径。真实编辑器中的鼠标点选和后续按键仍需设备验收。

- **队列边界**：Ctrl/Alt/Win 快捷键与 Shift 导航仍按原有规则处理，未纳入本次原始键捕获；这些快捷键仍可能越过未完成操作。`std::iswprint` 仍受本机 locale 限制，不能据此宣称覆盖全部 Unicode。应用编辑键注入沿用当前物理修饰键，没有恢复捕获时的修饰键状态；“分类使用捕获状态”不等于“导航注入恢复修饰键”。检查点仍有原文重放无法表达完整会话状态的边界。

## Verification

- `python3 scripts/test-windows-deferred-classification.py`：提取实际分类、投影、入队、重分类和 drain 方法编译执行，171 项断言通过。选择未完成时 Enter 原先直接漏给应用；只补捕获后，真实 drain 又复现原始 CR 插入，两阶段均先观察失败再修复。回归对同一数字、句点、字母、空格和编辑键分别提供补全继续与完整上屏快照，覆盖部分选词、日语 Fallback 后空格、Caps/Win 变化、Shift 原译文、AltGr、快捷键边界、重试不重分类、独立应用键 pending 后的新键、禁用/owner 失效与 COM 清槽重入。Win32/引擎环境使用合成替身，未模拟引擎补全算法和真实 TSF 消息循环。
- `python3 scripts/test-windows-application-key-replay.py`：183 项断言通过，扩展实际本地 owner 方法及应用文字 edit session，验证离线有效 owner、无线程焦点、错误 context/窗口、reset、COM 查询失败与重入、异步写入前失效、SetSelection 失败后不重复插字，以及 Backspace/Escape 的非扩展键标记。

- `python3 scripts/test-windows-deferred-recovery.py`：取实际 `_RefreshDeferredRecoveryPrefix` 方法编译执行，仅替换 Win32/COM 环境，覆盖空旧缓冲、旧缓冲残留、检查点替换、空宿主、光标、容量边界、字符修饰键和上下文引用释放；旧实现 11 项断言失败，快照实现全部通过。补充的 `CommitsOnBlur` 回归在缺少排除判断时四项失败，加判断后通过，避免已由 reset 上屏的组字再次入队。它由 `scripts/run-checks.sh` 自动发现，不依赖 Windows SDK；没有 C++ 编译器时显式跳过。扩展回归同时编译实际 `_CompleteDeferredKeyReplay`、`_ArmDeferredRecoveryForTransport`、`_RetryDeferredKeyReplay` 和前缀分类函数，覆盖数字/空格选择网址补全、部分选词和完整上屏，检查旧 token 不生效、完成槽释放、原文与光标、尾键顺序、重复 transport 通知和上下文引用计数；覆盖待执行尾键和下一键已在执行中失败两条重连入口；未修复完成回调时有 24 项断言失败，修复后全部通过。该测试验证检查点生成与队列移交，没有在真实 Windows 编辑器里模拟 transport 断线。
- `msime-tsf-key-composition-state`：复现空旧缓冲和点选后的宿主视图，验证网址开头、顶级域名、后续数字与符号、键盘触发进入、光标、模式退出、空视图、读失败和旧路径回退；`scripts/test-windows-native-run.py` 与 Windows CMake 均登记。旧缓冲实现有 20 项断言失败，快照实现全部通过。
- `cargo test -p msime-engine --locked`：`local::url::tests` 覆盖进入键、网址开头、邮箱和顶级域名补全的规则；`session::tests` 的 `url_at_after_a_local_part_offers_email_suffixes`（进入、20 个后缀、按已打域名缩小、完整后缀不给、数字是输入、选中上屏、空格交还 runtime）、`url_email_backspace_over_the_at_returns_to_the_composition`、`url_at_needs_a_plain_local_part`、`url_scheme_completions_enter_url_mode_when_picked`、`url_scheme_completion_is_not_taken_by_finishing`、`url_wubi_keeps_its_word_ahead_of_the_scheme_completion`，原有网址测试只改了 `spelling_symbols` 和补全行带来的期望。golden 基准不涉及网址，没有变化。
- `cargo test -p msime-input-runtime --locked`：`url_email_suffixes_leave_digits_and_space_to_the_address`、`url_completions_picked_through_the_runtime_keep_composing` 走真实引擎验证数字不选词、`SelectHighlighted` 上屏打的网址、选补全不上屏。
- `scripts/test-email-suffixes.py`、`scripts/test-windows-expression-symbols-parity.py`；`platforms/windows/tsf/tests/input/local_mode_key_policy.cpp` 用 clang 在本机编译运行通过。没有在任何平台的真实宿主里装机验证。

## Related notes

- [Android 邮箱后缀](2026-10-09-android-email-suffixes.md)：同一份后缀表和匹配规则，Android 在邮箱输入框里由宿主补全；邮箱输入框进框时切英文，引擎的网址模式不参与，两者不会同时出现。
- [网址退回组字复用剩余文本](../testing/2026-10-08-url-reversion-buffer.md)：退回组字的路径（`restore_composition_from_url`）不变，退回条件多了“删的是末尾”。
