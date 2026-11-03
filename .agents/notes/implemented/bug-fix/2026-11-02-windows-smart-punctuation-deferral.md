# Agent Note: Windows 延迟键屏障里的智能标点

Status: implemented

## Problem

用户在 QQ 里报告智能标点时好时坏：数字后打 `.` `,` `:` 有时候仍出中文标点，「中文标点后按空格转换」也时灵时不灵；同一个输入法在别的程序里稳定。

这不是「功能没实现」，而是两处在屏障窗口里才会出现的缺口：

1. 屏障（`_HasDeferredKeyBarrier()`）会把「没有功能」的可打印键排成应用文本，由 TIP 自己按顺序插进文档。这条路径上智能标点的前文影子（`_smartPunctuationShadowChar`）从来不会被记：入队时按「被吃掉的键」清掉，插入文档时又没人补记。QQ 是 Chromium/IMM32 宿主，`ITfContext` 的文本存储读不回光标前的字，于是下一格标点只能拿到「读不到」，数字后直出退回中文标点——有没有屏障决定对错。
2. `_ClassifyDeferredKeyDown` 里没有 `_IsKeyEaten` 那两条本地命令（空格转换、重复标点转中文）的分支，屏障窗口里的空格一律被当成普通文本排队，转换永远轮不到。

另外两处会让手势在屏障里更容易丢：`_NoteKeyForSmartPunctuation` 在任何一个中间按键上发现 `_CanConvertSmartPunctuationSpace()` 为假就清掉武装，而那个条件含 Server 会话说「就绪」，焦点会话协商期间会短暂为假并自己恢复。

**没能证明的部分**：手上只有 `1` `.` 空格 的日志，而 `1` `.` 出 ASCII 时本来就不会武装空格转换，那三行也是非屏障路径（`keydown-classified`）。所以「QQ 里那一次到底是不是屏障」没有证据；#10678 修的焦点令牌反复更换同样会清掉武装，也是候选来源之一，它已合并。这里的改动是把两处**代码上成立**的缺口补齐，QQ 里是否因此不再偶发要等复测。

## Decision

1. 排队等回放的应用文本，在它**真正写进文档**时记前文：`CDeferredApplicationTextEditSession` 的 `SetText` 成功后调 `_NoteSmartPunctuationDocumentText`。入队时那两处 `_NoteKeyForSmartPunctuation` 维持原样（被吃掉的键照旧让影子失效）——影子必须反映文档顺序，入队时就记会把后面排队的字当成前文（屏障里连打 `.` `1` 就会让 `.` 读到 `1`）。
2. 空格转换与重复标点转中文在屏障里先认领，排在「可打印键排队」和影子分类之前（`Global::DeferredSmartPunctuationClaimFor` 给出认领哪一条）。入口条件与 `_IsKeyEaten` 对齐：键盘开着，或在中文标点/全角语义下；自己组字的方案（韩文、注音、越南文、藏文）不认领；宿主会话、引擎里的组码或候选里有组字时不认领。认领对**排空时的重分类**同样生效：先排队的标点和空格在排到空格那一刻武装才出现，只在入队时认领会把这一步漏掉（`_ClassifyDeferredKeyDown` 的 `replayedKey` 分支）。判断「排到它时有没有组字」时，入队看投影（含前面排队的键），回放只能看宿主当前状态——投影已经包含它之后的键，拿它判断会把后面的字算进来。
3. 已经武装的空格转换只在焦点会话令牌变了或前台窗口变了时丢（`Global::SmartPunctuationArmStillBelongsToDocument`），会话暂时没就绪不再丢，是否就绪留到空格那一刻再判。不加时限：其他宿主的空格转换也没有时限，新增时限是用户能感知的行为变化。退格直接作废武装（与 macOS 下一个键就消费武装一致）；只按修饰键不算打断。
4. 读不回文档又被打断时只插入普通空格。盲删会删掉用户后来打的字；原地改写（`ShiftStart(-1)` + `SetText`）在终端代理存储里会报成功却什么都不改，空格就被吞了。
5. 新增 TSF 诊断记录 `[msime][smart-punctuation] before=digit|letter|other|none shadow=yes|no result=ascii|chinese process=…`：只记类别、不记字符、不记按了哪个键，与 macOS 的 `[smart-punctuation]` 同一约定。
6. 认领发生在入队、执行在排空之后，中间可能过期：重复标点的武装只有 2 秒窗口，排空时已被 `_NoteKeyForSmartPunctuation` 清掉的话，`_HandleSmartPunctuationRevert` 不再返回 `E_INVALIDARG` 把那颗键丢掉，而是按用户实际按下的字写进文档。空格那一侧的过期只会退化成普通空格，本身不丢键。

## Alternatives considered

- **在入队时就记影子。** 省掉编辑会话里的一处调用，但屏障里连打 `.` `1` 时 `.` 会读到后面那个 `1`；而且 `OnKeyDown` 的屏障分支还会以 `isEaten=true` 再 Note 一次，影子立刻被清掉。写入点必须跟文档顺序一致。
- **给空格武装加时限（放在策略里）。** 能挡住「很久以后才按的空格兑现旧手势」，但 develop 和 macOS 都没有这个限制，用户能感知；而且只在 Note 里判时限、`_CanConvertSmartPunctuationSpace()` 不看时间，两处判据会分叉。改为：打断时只插普通空格，退格作废，其余交给文档指纹。
- **读不回文档且被打断时走原地改写。** 终端代理存储会接受 `SetText` 并报成功而什么都不改，结果空格被吞而标点没换，比不转换更糟。
- **只用源码字符串断言做回归。** 把认领前的防护改成 `if (true)` 也照样通过。行为判定改由 `scripts/test-windows-deferred-classification.py` 抽出真实分类函数执行；C++ 用例只留策略真值表和它执行不到的接线（编辑会话写入点、读不回的三条退路）。

## Consequences

- 屏障窗口里数字/字母后的 `,` `.` `:` 仍按前文出 ASCII；读不回前文的宿主不再因为「有没有屏障」时好时坏。
- 屏障窗口里中文标点后的空格仍能转换，重复按同一标点仍能换回中文；标点和空格在同一次屏障里先后排队也一样（认领在排空重分类时补上）。
- 重复标点的武装在排队期间过期时，那颗键按用户按下的字写进文档，不再静默消失。
- 英文模式与韩文等自己组字的方案在屏障内外行为一致（都不认领）。
- 读不回前文时，标点后按过别的键就只插入空格，不会再盲删或盲改用户的字。
- 诊断日志只在 `diagnostic_log.tsf` 打开时写，一次标点一行，只含类别、不含任何用户文本。

## Verification

- `scripts/test-windows-deferred-classification.py`（既有抽取式用例，直接编译 `_ClassifyDeferredKeyDown` 本体）：新增十组断言——屏障里已武装的空格得到 `FUNCTION_SMART_PUNCTUATION_CONVERT`、同一按键得到 `FUNCTION_SMART_PUNCTUATION_REVERT`、组字中（当前的或投影里的）空格不被认领、键盘关闭时不认领、自己组字的方案不认领；以及**同一次屏障**里先排队的标点与空格、先排队的重复标点键在排空重分类时仍被认领，排到时已在组字的回放空格仍归组字（选词），投影里只在**后面**排队的字母不算到空格头上。反向验证：这条回放用例在修正前红（空格落进 `applicationText`，`dispatched` 里没有它），修正后绿。313 项检查全过。
- `platforms/windows/tsf/tests/input/deferred_smart_punctuation.cpp`（新增，已挂进 `platforms/windows/tsf/CMakeLists.txt` 的 ctest）：策略真值表（应用文本判定、认领选择、武装存活），加上抽取式用例执行不到的接线——应用文本写入时才记影子、退格作废武装、修饰键不算打断、读不回时只插空格、武装过期时按按下的字写入。用 `cmake --build ... --target msime-tsf-deferred-smart-punctuation-test` 编出并运行通过。
- `cmake --build target/windows-full/x64 --config RelWithDebInfo --target msime-tsf` 与同一条 x86 构建通过：改动在真实 TSF 编译单元里成立（Arm64X 这一轮没有重跑，改动不涉及平台差异）。
- `python scripts/test-windows-application-key-replay.py` 204 项全过。
- `python scripts/test-windows-native-run.py`：146 项通过，2 项按宿主差异排除，0 失败。
- `pnpm run verify-notes`、`git diff --check` 通过。
- **没有在 QQ 里复测。** 本机 QQ 未登录、桌面同时被另一个会话占用。确认手段是打开 `diagnostic_log.tsf`，在 QQ 里打 `1.2`、以及「中文标点 + 空格」，看 `logs\server.log` 里 `[msime][smart-punctuation]` 的次数与 `shadow=yes` 是否稳定；同时观察屏障事件（`keydown-deferred-queued`）附近是否仍有 `result=chinese`。
