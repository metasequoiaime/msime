# Agent Note: Windows TIP 把 Engine 不收的空闲字符当上屏写入

Status: implemented

## Problem

修 #9696 时发现的另一层（见[五笔打开不经拼音拼写的 Shift 模式](2026-10-25-wubi-scheme-free-shift-modes.md)的「遗留」）：没有组字时，Engine 不收的 Shift+字母在 Windows 上被吞掉。五笔下的 Shift+A，以及 #9719 之后仍只在全拼、双拼下进入的 E、M、J、V；拼音方案在「Shift+字母进入快捷模式」总开关关掉时的任何大写字母也一样（`windows-wubi-auto-commit` 的新场景确认了这一点，不只是推断）。

链路：TIP 在按下时分类，五笔被记成全拼（`mode_scheme`），没有组字时的大写字母由 `IsVirtualKeyNeed` 当 `FUNCTION_INPUT` 吃下，入延迟按键队列；Server 和 TIP 两份会话都把这个键交给 Engine，Engine 在 `dispatch_character` 里答 `handled=false`（五笔没有模式可进；拼音总开关关掉时 `starts_pinyin_with_shift` 为假）。TIP 的 `_ApplyHostCompositionResult` 经 `ApplyHostCharacterResult` 得到 `Unhandled`，返回 `S_FALSE`，编辑会话按「没应用」处理：

- 拼音：`_RetryDeferredKeyReplay` 每次标脏会话并重发同一个键，Engine 每次都答同样的 `handled=false`，第 8 次后 `deferred-replay-abandoned`、`deferred-queue-cleared`、重连。#9696 报告者 0.3.0 的 server.log 里每次 Shift+Y、Shift+T 都是这一串（`deferred-replay-retry` × 7）。
- 五笔：`_wubiCommitOwner.observe` 已把这个 token 记成「本地已推进」，`_FinishWubiLocalEdit(token, false)` 直接走 `_FinishWubiWorkerEdit(token, false)`，清空队列并重连，不重放。

两条路最后都把字母丢掉，应用什么也收不到。macOS（`InputController.mm` 在 `handled=false` 时返回 `NO`）和 Linux（`ClientEngine.cpp` 把 `apply` 的结果交回 IBus）把同一个答复交还应用，应用打出大写字母，不需要改。

## Decision

`handled=false` 对空闲字符是 Engine 的最终答复，不是暂时失败。`HostCharacterResult.h` 的 `DeclinedIdleCharacterCommit` 判断：答复未处理、没有提交、视图没有组字（预编辑、原文、已选词段、候选都为空，`local_mode == "none"`），且键是可打印 ASCII（0x20–0x7E）时，把答复改写成上屏这个字符。`_ApplyHostCompositionResult` 多一个 `typedKey` 参数，只有 `_HandleCompositionInput` 的宿主分支传入产生答复的字符；改写排在五笔提交归属的 `observe` 之后（改写后的提交是 TIP 自己写的，不能让 `WubiCommitOwner` 去等一个不会来的 Server worker），再照常经 `ApplyHostCharacterResult` 写进组字并以 `_HandleCompleteCommitFirst` 结束组字，编辑会话返回 `S_OK`，回放 token 正常完成。

不在此列、行为不变的：

- 组字中 Engine 不收的键：它们要先结束组字，仍答 `Unhandled`，走原来的路。
- 组字中的大写字母：拼音的原文词段（#8815）、五笔的大写顶字都是 `handled=true` 或带提交，不经过这里。
- 开着总开关的拼音空闲 Shift+字母：Engine 开始组字（`handled=true`）；五笔 Shift+T/U/Y/S/K/C 进入各自模式。
- 连续大写里的第二枚起（#7616 的 `uppercase_passthrough`）：Engine 不进模式、答 `handled=false`，同样上屏。
- 真正的暂时失败（焦点或代次过期、IPC 失败、`GetSelection` 失败、编辑会话请求失败）走的是各自的 `S_FALSE`/`E_FAIL` 分支，不经过宿主答复，仍按原样重试。

## Alternatives considered

- 在分类时问同一个谓词、不吃这个键：最强的理由是应用直接收到原始按键，和 macOS、Linux 完全一样。没有采用：分类发生在按下时，排在前面、还没应用的键决定这个键遇到的会话状态（`_ClassifyDeferredKeyDown` 用的是队列投影，不是会话），要预测 Engine 收不收，得在 TIP 里重写一份 `local_mode_for_entry`、总开关、各模式开关、`uppercase_passthrough` 和方案判断，正是 #9719 已经出过错的那种镜像；只有编辑会话里宿主会话的真实答复是准的。键吃下以后再用 `SendInput` 交还应用会排到队列里后面的键之后，打乱顺序。
- 让回放队列把 `Unhandled` 当完成、什么也不写：没有重放和重连了，但字母还是丢。
- 只对字母特判：问题不在字母，在「吃下的空闲键被 Engine 拒绝」；数字、符号如果走到这里也应该照打。

## Consequences

- 收益：空闲时 Engine 不收的大写字母在 Windows 上照常打出，不再触发 8 次重放、清队列和重连；判定只看 Engine 的答复，以后 Engine 改变哪些 Shift+字母进模式时 TIP 不用跟着改。
- 代价：这个字符经 TIP 的组字写入（开始组字、写入、结束），不是应用自己处理的原始按键；只在乎原始 `WM_KEYDOWN`/`WM_CHAR` 的应用（游戏快捷键）看到的是提交文本。全角模式下写入的是半角字符，和直接交还应用的按键一致，不经 TIP 的全角转换。这个字符也不计入打字统计的直通字符（直通统计只在按键放行给应用时记）。
- [Windows 五笔提交归属](2026-10-23-windows-wubi-commit-owner.md)不变：`observe` 仍先看宿主会话的原始答复，未处理的答复不会登记 Server worker。

## Verification

- `platforms/windows/tests/input/wubi_auto_commit.cpp` 新增 `declined_capital_scenario`，Server（`ReplyComposer::basic_key`）和 TIP（`EngineSessionAdapter`）两份真实会话按 `_ApplyHostCompositionResult` 的顺序处理：五笔 `A`、`Aw`、`E`、`M`、`J`、`V`、`AT` 上屏大写字母、Server 没有提交、后面的 `w` 照常组字；五笔 `T` 仍进入 `date_time`；拼音总开关关掉时 `A`、`Tn` 上屏大写字母，`nA` 照常组字；总开关开着时 `A`、`nA` 组字不变。修改前（辅助函数只返回 false）五笔 `A` 和拼音总开关关掉的 `A` 都失败在「the TIP finishes every key it ate instead of replaying it」（TIP 答 `handled=0`，状态 `Unhandled`），五笔 `T` 与拼音总开关开着的场景修改前就通过。
- `platforms/windows/tsf/tests/input/character_result.cpp` 覆盖改写本身和不改写的答复：已处理、带提交、组字中、已选词段、本地模式、不可打印字符。
- `scripts/test-windows-native-run.py` 在 Mac Studio 上全部通过（187 passed）。Windows 实机未验证（测试机 win11、pve9-win-runner 不可达），`KeyHandler.cpp` 的接线只由 PR CI 的 Windows 构建编译。
