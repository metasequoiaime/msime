# Agent Note: Windows 延迟键屏障里保留智能标点的前文与手势

Status: implemented

## Problem

用户在 QQ 里报告智能标点时好时坏：数字后打 `.` `,` `:` 有时候仍是中文标点，「中文标点后按空格转换」也时灵时不灵；同一个输入法在别的程序里稳定。复现不了「一定坏」，只能描述成偶发。

诊断日志（`%LOCALAPPDATA%\metasequoiaime-full\logs\server.log` 里 TSF 经管道回传的 `[msime]` 记录）给出了形状：QQ 里一次典型的 `1` `.` 空格 会得到

```
keydown-classified vk=0x31 fn=4 eaten=0        # 数字，直通
keydown-classified vk=0xBE fn=23 eaten=1       # 句号，走标点路径
keydown-classified vk=0x20 fn=4  eaten=0       # 空格，当普通文本交给宿主
```

空格从来没有拿到 `FUNCTION_SMART_PUNCTUATION_CONVERT`（函数号 30，整份日志里一次都没有）。这条链路上的两个前提都靠「TIP 记得用户刚才打了什么」：

- `_GetPrecedingCharForSmartPunctuation()` 先用本进程的前文影子（`_smartPunctuationShadowChar`），影子无效才去读宿主文本存储。QQ 是 Chromium/IMM32 宿主，`ITfContext` 的文本存储读不回光标前的字，读回 0 个字符。
- 「中文标点后按空格转换」在 `_ClassifyDeferredKeyDown` 里根本没有对应分支，只有正常路径 `_IsKeyEaten` 有。

而 Server 回得慢、或有偏好事务在排队时，TIP 走延迟键屏障（`_HasDeferredKeyBarrier()`）：可打印键被吃掉、排队、稍后由 TIP 自己按顺序插进文档。这条路径上有三处把前文和手势弄丢：

1. 排队等回放的**应用文本**被记成「被吃进组字的键」，`_UpdateSmartPunctuationShadow` 因此让影子失效。可那个字符明明就是用户刚打进文档的字。下一格标点只能去读读不回来的文档，于是出中文标点——取决于那一瞬间有没有屏障，这就是「时好时坏」。
2. 屏障的兜底规则是「可打印键一律排进应用文本队列」，空格也在其中，于是它永远轮不到转换分支。
3. `_NoteKeyForSmartPunctuation` 在任何一个中间按键上发现 `_CanConvertSmartPunctuationSpace()` 为假就立刻清掉武装。而那个条件里含 Server 会话说「就绪」（`_workerCommitReady`），焦点会话协商、偏好事务期间会短暂为假并自己恢复——用它判一次就把已经打出的手势永久丢掉了。

## Decision

1. 排队等回放的应用文本按「写进文档的字」记前文。判定统一成 `Global::DeferredKeyIsApplicationText(hasCategory, hasFunction)`：没有 `Category` 也没有 `Function` 才是被排队的应用文本，队列分拣和智能标点记前文共用这一条（此前 `_QueueDeferredKeyDown` 里那份判断和这里各写一遍）。
2. 空格转换和重复标点转中文在屏障里先认领，排在「可打印键排队」之前（`Global::DeferredSmartPunctuationClaimFor` 直接给出要认领哪一条，不再由调用方重算一遍是不是空格键）。两条命令只看本进程记住的上一次上屏和当前焦点，不需要 Server 的分类结果。条件与 `_IsKeyEaten` 对齐：没有组字投影、没有候选、没有活动组字。
3. 武装的存活判断换成 `Global::SmartPunctuationArmStillBelongsToDocument`：只丢真正回不来的情况——焦点会话令牌换了、前台窗口换了、超过 `SMART_PUNCTUATION_SPACE_WINDOW_MS`（2 秒，与重复标点同长）。会话暂时没就绪不再丢；开关关掉时 `_CanConvertSmartPunctuationSpace()` 自己会拦，不必在这里再判一次。「重复标点转中文」不跟着放宽：它靠盲删一个字符兑现，读不回前文时放宽窗口只会多出误删的机会。为了让放宽后的空格手势同样不盲删，标点之后只要按过别的键就记下 `_smartPunctuationSpaceInterrupted`，读不回文档时那条 SendInput 路径不再兑现（文档读得回来时仍按指纹判）。

另外补一条 TSF 诊断记录 `[msime][smart-punctuation] before=digit|letter|other|none shadow=yes|no result=ascii|chinese process=…`：只记类别、不记字符、不记按了哪个键，和 macOS 的 `[smart-punctuation]` 同一约定。这条问题只能在读不回前文的宿主里靠它确认，否则「修好了没有」永远说不清。

## Alternatives considered

- **让 TSF 用 UI Automation 读宿主前文。** QQ 的输入框在 UIA 里是有文本的，能绕开浅文本存储；但那是跨进程、跨 COM 的同步查询，要在按键分类这条热路径上做，延迟和重入风险都比一个进程内的影子大得多，而且 UIA 客户端在 Chromium 里通常要先唤醒无障碍树才拿得到文本。参考实现（msime-windows）也是靠影子。
- **空格转换不再要求 Server 会话说就绪。** 它写的是文档里的上一个字符，确实不需要 Server；但浅文本存储那一侧会退化成 SendInput 盲删一个字符，读不回前文时唯一的安全依据就是焦点令牌和前台窗口。这一步会把「不转换」的方向从偶发失败改成偶发误删，风险比收益大，只做上面第 3 条（不丢武装、但仍在转换那一刻判一次）。
- **只要屏障就把按键直接交给宿主。** 屏障存在的意义就是保持顺序；把它拆掉会让慢宿主上的选词和组字错位，代价远大于这两条标点命令。

## Consequences

- 屏障窗口里数字/字母后的 `,` `.` `:` 继续按前文出 ASCII；QQ 这类宿主的偶发回退消失。
- 屏障窗口里中文标点后的空格仍能转换成 ASCII，重复按同一标点也仍能换回中文。
- 手势在会话协商期间不再被丢掉，但最多只活 2 秒：读不回前文的宿主没有指纹可以反悔，过期的手势不能再兑现；标点之后按过别的键时，读不回文档的一侧也不兑现。
- 诊断日志只在 `diagnostic_log.tsf` 打开时写，一次标点一行，只含类别，不含任何用户文本。

## Verification

- `platforms/windows/tsf/tests/input/deferred_smart_punctuation.cpp`（新增，已挂进 `platforms/windows/tsf/CMakeLists.txt` 的 ctest）：三条策略的判定表逐个断言，并按源码检查它执行不到的部分——应用文本的判定只写一份、影子与盲删的前置条件。同一份用例对 `origin/develop` 的 `KeyEventSink.cpp`/`Composition.cpp` 运行会红（8 条断言失败），对本次改动全绿。
- `scripts/test-windows-deferred-classification.py`（既有用例，直接编译 `_ClassifyDeferredKeyDown` 本体）：新增四组断言——屏障里已武装的空格得到 `FUNCTION_SMART_PUNCTUATION_CONVERT`、同一按键得到 `FUNCTION_SMART_PUNCTUATION_REVERT`，以及组字中（当前的或投影里的）空格不会被认领。把认领块摘掉后这两条会红（反向验证过），288 项检查全过。
- `python scripts/test-windows-native-run.py`：新增用例编译并执行通过；唯一失败是 `caps_lock_tap.cpp`，在未改动的 `develop` 主工作区上同样失败，属于本机既有差异，与本次改动无关。
- **没有在 QQ 里验收。** 本机 QQ 未登录、且桌面同时被另一个会话占用，自动化打字会互相干扰，所以「偶发失效消失了没有」必须由报告者在 QQ 里按原步骤复现确认；确认手段是打开 `diagnostic_log.tsf` 后看 `[msime][smart-punctuation] shadow=yes` 是否稳定出现在数字后的句号上。
- 没有在 Windows 原生构建里编译过（本机无 MinGW、无 MSVC），TSF 的编译由 CI 的 Windows 作业覆盖。
