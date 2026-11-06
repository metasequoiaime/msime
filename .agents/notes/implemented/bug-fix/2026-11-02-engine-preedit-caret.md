# Agent Note: 组字光标在 preedit 里的位置由引擎给出

Status: implemented

## Problem

#10632：Windows 双拼把「双拼预编辑」设成「拼音分词」后，行内预编辑里的光标位置不对。这时引擎的 `preedit` 是展开的全拼（小鹤 `uiui` 是 `shi'shi`），而 `caret_position` 仍是按键原文 `editing_text` 里的字节偏移。TSF 的 `MapPreeditCaret` 按「原文光标前有几个字母，就在 preedit 里走几个字母」换算，这只对全拼分词这种只在字母之间加了隔音符的拼写成立：`uiui` 打完光标在原文 4，走 4 个字母落在 `shi's|hi`，而不是末尾。只打一个音节 `ui` 也落在 `sh|i`。所以用户每打一个键，光标都停在音节中间。「原始按键」行内样式也会碰到：`HostRawStyleText` 只对全拼换画 `editing_text`，双拼照旧画 `preedit`。

其他宿主各自有一份同样的换算，但在字母对不上时不猜：macOS／iOS 的 `MSIMEPreeditCaretPosition` 放在末尾，Windows 候选窗的 `preedit_display_caret` 不画光标，IBus 在「拼音分词」样式下一律放在末尾，Fcitx5 不设光标。这些宿主打字时不会画错位置，只是左右移光标时光标不跟着动。`MSIMEPreeditCaretPosition` 的注释早就写明，展开的双拼要靠「引擎提供的偏移映射」，不能在宿主里猜。

## Decision

引擎门面（`crates/engine/src/host/session.rs`）在快照里多给一项 `preedit_caret`：组字光标在 `preedit` 里的位置，按 Unicode 标量计。它经 input-runtime 的 `View.preedit_caret` 交给宿主，`msime_client.h` 里写了它的含义。换算在 `crates/engine/src/host/session/preedit_caret.rs`，按顺序判断：

- 光标在末尾（或越界）：`preedit` 的长度。
- `preedit` 与 `editing_text` 相同：照搬。
- `preedit` 只是在相同的字母之间加了分隔符（全拼分词、双拼原始按键的 `ni'hc`）：按字母数换，落在音节边界上的光标停在插入的隔音符前面，紧跟用户自己打的隔音符时落在它后面。和 macOS、Windows 原来的映射一致。
- 字母对不上、但 `preedit` 是 ASCII，并且显示音节的个数等于原文单位的个数（`segment_raw_boundaries` 划出的单位，只由 `'` 组成的段不算）：光标前有几个完整单位，就落在第几个显示音节后面。光标在一个单位中间时按单位起点算，和光标前缀解码（`prefix_end`）取的是同一段，候选也只是这几个完整单位的。
- 其余情况（韩文音节、假名、笔画字形，以及音节数对不上，例如末尾辅助码单独算一个单位却不单独显示）：`preedit` 的长度。光标画错位置比画在末尾更糟。

Windows TSF 读 `preedit_caret`（`EngineView::preedit_caret`，`KeyCompositionStateFromView` 换算成 UTF-16）。`MapPreeditCaret` 多了一个重载：画出来的文字去掉已选词段后正好是引擎的 `preedit` 时，用引擎给的位置再加上词段的长度；画的是别的文字（全拼的原始按键、日文假名）或视图里没有这一项时，照旧按字母数换。绘制、纯光标移动（`_MoveCompositionCaretToHost`）和左右键三条路径都走这个重载。

macOS、iOS、Linux 和 Windows 候选窗这次不改。它们打字时光标在末尾，位置不错；改成读 `preedit_caret` 能让左右移光标时光标跟着动，但这属于另一类行为改动，需要在各自平台上验收。Linux 两个宿主的候选拼音行后来改读它，见 [Linux 候选拼音行的组字光标](2026-11-03-linux-candidate-preedit-caret.md)。

## Alternatives considered

- **只在 TSF 里改成和 macOS 一样，字母对不上时放在末尾。** 这样改动最小（一个头文件），也能消掉报告的现象。但左右键移光标时光标会一直停在末尾，看不出改的是哪一段，下一个问题已经能预见到。而且每个宿主都要知道「哪些 preedit 能按字母换」，这正是这次出错的地方。
- **在 TSF 里按 `segment_raw_boundaries` 自己数音节。** 不用改 Rust 和视图契约。但它需要知道 `preedit` 怎么由单位拼出来（辅助码装饰、用户打的隔音符、末尾隔音符），这些都是引擎的规则；放在 C++ 里会有一份会漂移的拷贝，其他宿主以后还要再抄一遍。
- **核心快照 `SessionSnapshot` 里加字段。** 核心快照受金样例约束，而这个位置只给宿主画光标用，所以放在门面里，和 `segment_raw_boundaries`、`display_reading` 放在同一层。

## Verification

- `cargo test -p msime-engine --lib preedit`：`preedit_caret.rs` 的单测覆盖照搬、全拼分隔、双拼展开（单位边界、单位中间、末尾半个单位）、用户打的隔音符、末尾隔音符和对不上时放在末尾。`host::tests::expanded_shuangpin_preedit_carries_the_caret_by_syllable` 用真实会话打 `uiui`：末尾是 7，左移到 3、2 都是 3，Home 是 0；原始按键时与 `caret_position` 相同。
- `scripts/test-windows-preedit-caret.py` 提取生产的 `_HandleCompositionInputWorker` 和 `_MoveCompositionCaretToHost`，在原始按键和拼音分词两种行内样式下画 `uiui` → `shi'shi`：光标在末尾，左移跟着引擎给的位置，已选词段在前时整体后移。把重载里使用引擎位置的分支关掉后，这条用例失败（`expanded shuangpin keeps the caret at the end of the displayed pinyin`）。
- TSF 的 `preedit_caret.cpp` 和 `engine_response.cpp` 补了重载和解析的用例，在本机用 clang 编译运行通过。Windows 交叉编译与其余验证见 PR，没有在真实 Windows 编辑器里验收。

## Consequences

宿主不用再猜展开拼写里的光标，以后哪个宿主想让光标在 `preedit` 里跟着移动，读这一项就行。代价是视图多一个字段，每次快照多一次换算（只扫一遍 `preedit` 和边界），门面的 `annotation_move_tests` 里那份参照快照也要跟着同步。光标在一个双拼单位中间时只能画在单位起点，和候选覆盖的范围一致，但看起来和按键的移动不是一格一格对应的：从末尾左移一格，光标就跳到上一个音节后面，再左移一格位置不变。显示音节和原文单位对不上的情况（例如末尾辅助码）仍放在末尾，不会跟着移动。

Windows 候选窗拼音行后来也改读 `preedit_caret`，见 [Windows 候选窗拼音行按引擎给的位置画插入点](2026-11-04-windows-candidate-preedit-caret.md)。

## Existing notes audit

检索 `MapPreeditCaret`、`preedit_display_caret`、`MSIMEPreeditCaretPosition`、`caret_position`、拼音分词。[Windows 预编辑文字与光标共用宿主快照](2026-10-22-windows-host-preedit-caret.md)负责 TSF 绘制从同一次宿主视图取文字和光标，以及 `SetCompositionCaret` 定位。本篇沿用这两点，只换掉其中「分词符映射保持现有规则」这一项在展开拼写上的做法，两篇是部分重叠。[macOS 行内组字的标记样式](2026-10-19-macos-inline-preedit-caret.md)只涉及标记属性，[字母按键光标画在显示分隔前面](2026-10-29-letter-caret-before-display-separator.md)是 Android 读音行在分隔两侧的取舍，本篇在全拼分隔上的规则（停在插入的隔音符前面）和它一致，与这两篇都没有共享决定。
