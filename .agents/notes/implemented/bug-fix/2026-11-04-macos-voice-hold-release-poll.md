# Agent Note: macOS 按住说话不再只靠松开事件收尾，Esc 与锁定空格不再被一次丢失的 keyUp 吞死

Status: implemented

## Problem

2026-11-03 的设备验收（PR #10710 构建，合成 CGEvent 驱动右 Option）里，按住右 Option 说话松开后录音五次没有停：麦克风一直开着，把房间里的说话录成预编辑；之后的 Esc、再按一次右 Option 也常常无效，最后一次只能杀掉输入法进程。这是隐私缺陷。

统一日志与验收脚本的时间线给出两处状态机缺陷，都出在 `platforms/macos/src/voice/VoiceHoldShortcut.h`：

- 按住只靠松开的 flagsChanged 结束。松开通知没送到输入法（或送到的仍是按下时的快照）时 `hold_` 永远不清，录音一直开着；`flags_` 里还留着右侧位，下一次按下不构成按下沿，被 `if (hold_ != Hold::None) return {key == suppressed_};` 当成重复通知吞掉。验收里紧跟每次失败的那一次按下确实都没有开始录音。
- `escape_`、`space_` 只在收到对应 keyUp 时清零。Esc 按下被输入法吞掉之后它的 keyUp 没有送到，`escape_` 就一直是 true，之后每次 Esc 按下都走「标记在场就吞掉」的分支：不取消录音，应用也收不到 Esc。验收里 Esc 的成败和 `Activate Server`（`activateServer:` 里会 reset）一一对应：一次焦点里只有第一次 Esc 能取消。锁定用的空格同理，keyUp 一丢，这次焦点里之后的空格都被吞。

修饰键和普通键的 keyUp 会丢，仓库里早有记录（`platforms/macos/src/core/ModifierTap.h` 的 `anyKeyHeld()` 注释）；[Option 侧别回退](2026-10-23-macos-voice-option-modifier-flags.md) 也写明它「无法补救完全没有送达的事件」。Esc 与空格这一半与按键来源无关；右 Option 松开丢失那一半，现有日志分不出合成事件与实体按键，但状态机对任何来源的丢失都没有恢复手段。

## Decision

- 按住期间宿主每 0.1 秒用 `CGEventSourceFlagsState(kCGEventSourceStateCombinedSessionState)` 调一次 `MSIMEVoiceHoldShortcut::poll`（`InputController.mm` 的 `syncVoiceHoldPoll` / `pollVoiceHold`）。状态确定按住的键已经抬起时，按住和收到松开一样结束：这次按住开始、未锁定的录音停下（与事件路径同一个代次判断），`flags_` 换成会话状态，下一次按下照常构成新的按下沿。迟到的松开事件仍被吞，应用从没收到过这次按下。按住结束计时器就停，`dealloc` 里也停。
- 「确定抬起」是保守判定：通用修饰位没了，或者侧别位在场而按住的那一侧不在。只有通用位、看不出哪一侧时当作仍按着，宁可晚停，不在用户还按着时提前停。Control+Command 只看通用位。
- Esc 与空格的标记只拦自动重复（`isARepeat`）。新的一次按下说明上一次早已抬起：先清掉标记再照常处理，录音中的 Esc 照样取消，不在录音时的 Esc、不在按住时的空格交给应用。
- `activateServer:` 和没有客户端的按键会 reset 按住状态，之后不会再有松开来结束这次按住开始、未锁定的录音，所以 reset 前先按松开把它停下（`resetVoiceHoldFinishingRecording`）。其他 reset 点原本就会取消或结束录音，不变。

## Alternatives considered

- 用 HID 状态（`kCGEventSourceStateHIDSystemState`）或 `CGEventSourceKeyState(…, 61)` 判断松开：侧别精确，`ModifierTap.h` 也这么用。但 HID 状态不一定包含别的进程合成的按键（验收脚本、辅助功能和改键工具都走这条路），按住期间会被误判成已经松开、提前停录音。会话合并状态是两者的并集，只会晚停不会早停。
- 给按住录音加一个固定时长上限：实现最简单，但上限之内录音仍然比按住活得久，长按口述又会被截断，治标不治本。
- 只修 Esc，让用户自己兜底：Esc 本身也依赖会丢的 keyUp，而且用户以为已经松开、根本不知道还在录。
- 安装全局键盘事件监听拿到松开：需要输入监控授权，多一条快捷键路径；[Option 侧别回退](2026-10-23-macos-voice-option-modifier-flags.md) 当时同样没有走这条路。

## Consequences

收益：松开通知丢失时录音最迟在约 0.1 秒后停下，下一次按下和 Esc 不再被陈旧状态吞掉；一次焦点里 Esc 可以反复取消，锁定之后的空格照常打字。按住之外不新增任何查询或计时器，不需要新的授权，出厂默认不变。

代价：按住期间每 0.1 秒一次状态查询。按住时左右 Option 同时按着且状态里没有侧别位，松开右侧后要等左侧也松开才停（与事件路径一致的保守取舍）。同一客户端在按住中途被重新激活时，录音按松开结束，而不是继续跟着按住。

验证缺口：`tests/voice/VoiceHoldShortcutTest.mm` 覆盖状态机（轮询判定、锁定、按住开始时停的是旧录音、三种组合、Esc 与空格的丢失 keyUp），新增的 Esc/空格断言在旧头文件上先失败；控制器里的计时器接线没有自动化测试。合成 CGEvent 是否反映在会话合并状态、实体按键与合成事件下松开都能停，需要设备轮次确认，清单在 PR 里。

## Note audit

- [macOS 语音按住快捷键的 Option 侧别回退](2026-10-23-macos-voice-option-modifier-flags.md)：部分重叠，互链。那篇恢复事件里缺失的侧别位，并写明补救不了完全没送达的事件；本篇补的正是这一块，那里的决定不变。
- [macOS 与 Linux 的按住组合键](2026-11-02-macos-linux-voice-hold-chord.md)：无冲突。组合键取消仍在事件路径里，轮询只处理松开。
- [Windows 右 Alt 组合键](2026-11-02-windows-voice-ralt-chord.md)：无关。Windows 用低级钩子并以 `GetAsyncKeyState` 核对，不是这套状态机。
