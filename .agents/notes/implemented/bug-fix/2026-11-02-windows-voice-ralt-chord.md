# Agent Note: Windows 按住右 Alt 说话时再按别的键，改为把组合键交给应用

Status: implemented

## Problem

Issue #10518：在 PowerPoint 里用五笔，按 Alt+F10 的效果和只按 F10 一样，选择窗格开关不了。用户附的 server.log 里，PowerPoint 进程只记到两次 F10（`vk=0x79`，`eaten=0`），一次 Alt（`vk=0x12`）都没有，说明 Alt 在 TIP 之前就被拦掉了。TIP 的按键接收器、preserved key、线程钩子和维护快捷键钩子都不吞 Alt。

全仓唯一会吞 Alt 的是 Server 的语音低级键盘钩子 `platforms/windows/src/voice/VoiceHotkey.cpp`。「按住右 Alt 说话」默认开着（`voice_input.enabled`、`hotkey_ralt` 出厂都是 true），右 Alt 按下时 `activate()` 设上 `suppress_ralt_until_up_`，这次按下和之后的自动重复、松开全部吞掉，同时开始录音。按住期间再按别的键，只有空格（锁定）和 Esc（取消）有特殊处理，其余键原样放行，可应用从没见过 Alt，于是右 Alt 加任何键的应用快捷键都退化成裸键。

用户按的是左 Alt 还是右 Alt 还没确认。左 Alt 在 Server 和 TIP 里都找不到被吞的路径，本篇只处理右 Alt。

## Decision

右 Alt 的按住（`RAlt`，以及同样吞掉右 Alt 的 `RCtrlRAlt`）期间，用户新按下一个非修饰键，就当作组合键：

- 结束这次按住，只丢弃这次按住自己开始的那段录音，不上屏。取消走新消息 `kChordCancelMessage`。窗口线程处理 `kStartMessage` 时，如果这次按住真的开始了录音，就记下 `VoiceInputSession::session()` 返回的会话代次（`hold_session_`），否则记 0。`kChordCancelMessage` 只在 `voice_chord_cancels_recording` 成立时调用 `voice_.cancel()`：记下的代次不是 0、等于当前代次、仍在录音、没有锁定。下面几种都不取消：
  - 按住之前已经在录音（Ctrl+F9、自定义切换键、工具栏麦克风、设置页试录开始的免手持听写）：用户按的是快捷键，不是要结束这段听写。修复前这段听写会在右 Alt 松开时被 stop 并上屏，`cancel()` 会把录音直接丢掉，所以不能取消。组合键之后右 Alt 的松开也不再停止它，听写继续。德语、法语布局在听写中按 AltGr+Q 打 @ 就走这条路。
  - 这次按住停掉的是一段已锁定的听写，或者开始录音失败：此时 `cancel()` 会连带丢掉上一段还在识别的结果。
  - 按住开始的录音已经结束，另一处又开始了新的一段（代次变了）。

  代次要在窗口线程上、真正调用 `toggle()` 之后取，不在钩子的 `activate()` 里判断：钩子运行时前面投递的消息可能还没处理，比如 Ctrl+F9 的 `kToggleMessage` 还在队列里，这时看到的「没在录音」是过时的。`start()` 和 `cancel()` 都会推进代次，`stop()` 不会，所以代次相等加上仍在录音就能确认还是同一段。
- 吞掉这次按键，用 `SendInput` 依次补发右 Alt 按下（`KEYEVENTF_EXTENDEDKEY`）和这个键的按下（同 vk、scanCode、扩展键位）。不能只补右 Alt 再放行当前键：注入事件排在当前事件之后，应用会先看到键、再看到 Alt。
- 之后的松开不再拦截，应用自然收到这个键和右 Alt 的松开，与补发的按下配对。
- 补发的两条事件 `dwExtraInfo` 带 `kChordReplayExtraInfo`（`0x4D535641`），钩子见到它直接放行，不更新任何状态。这个值故意不进 TIP 的 `IsSelfGeneratedSendInputExtraInfo`：补发的键代替的是被吞掉的物理按键，TIP 要像对物理键一样分类和计入统计。
- 组合键之后用户仍按着右 Alt，它的自动重复会一直以按下事件到达。`ralt_chord_until_up_` 在右 Alt 松开前挡住新的按住，否则第二次重复就会重新开始录音、再次吞掉应用已经拿到的 Alt。

不触发组合键的情况，集中在纯函数 `voice_hold_becomes_chord`（`VoiceHotkeyPolicy.h`）里：松开事件；按住开始前就按着的键的自动重复（用钩子已有的 `toggle_strokes_` 判重复）；带 `LLKHF_INJECTED` 的别的程序注入的输入；修饰键（Shift、Ctrl、Alt、Win 的通用码和左右码），它们仍是正在组合的一部分，照旧放行；空格且「空格锁定」开着；Esc；用空格锁定过的按住（`hold_locked_`），那是用户要保留的听写。`CtrlWin` 不在范围内，它吞的是 Win 不是 Alt。

出厂默认不变：语音和右 Alt 开关照旧默认开着。

## Historical audit

- [macOS 语音设置裁剪](../simplification/2026-10-16-macos-voice-settings-prune.md)：无冲突。那里记的 Server 不再读 `hotkey_hold_space_lock`、空格锁定恒为开，本篇的「空格不触发组合键」就是按这个恒开的值走的。
- [切换热键可重绑](../feature/2026-10-21-rebindable-toggle-hotkeys.md)：无冲突。自定义切换键和 Ctrl+F9 在钩子里先于按住逻辑匹配，命中就返回，不会走到组合键判断。

## Alternatives considered

- **只补右 Alt，当前键放行**：改动最少。不采用：`SendInput` 的事件排在当前事件之后，应用收到的顺序是 F10、Alt，Alt+F10 仍然不成立。
- **不吞右 Alt 的按下，等松开时再决定**：应用一直能看到 Alt，组合键天然成立。不采用：单按右 Alt 会让应用激活菜单栏，按住说话每次都会把焦点抢到菜单上，这正是当初吞右 Alt 的原因（`voice_hold_suppresses_ralt` 的注释）。
- **组合键时 stop 而不是 cancel**：保留已经录到的那一小段。不采用：用户按的是快捷键，不是在说话，几百毫秒的录音上屏只会多出噪声识别的字；维护者也定为取消。
- **AltGr 布局下默认不启用右 Alt 语音**：德语、法语等布局的右 Alt 就是 AltGr，按住 AltGr 打 @、€ 时会被当作语音按住。本篇顺带改善了一部分（按住 AltGr 再按字符键会补发 AltGr 和这个键），但改默认值是产品决定，这次不做，在 PR 里提给维护者。

## Consequences

收益：语音开着时，右 Alt+F10、右 Alt+F4 这类应用快捷键恢复正常，不再留下一段误录的语音。

代价与未验证的部分：`VoiceHotkey.cpp` 依赖 Win32 低级钩子，只有策略函数能在非 Windows 机器上跑。下面这些要 Windows 实机确认：PowerPoint 里右 Alt+F10 能开关选择窗格且不开始录音；单按右 Alt 说话、空格锁定、Esc 取消不回归；德语布局下 AltGr+Q 能打出 @（系统为 AltGr 合成的左 Ctrl 在物理按下时已放行，补发的右 Alt 是否会再合成一次左 Ctrl 没有实测）。macOS 右 Option、Linux IBus 右 Alt 的同类行为另开 issue 跟进。

## Testing

- `platforms/windows/tests/voice/voice_hotkey_policy.cpp` 补了 `voice_hold_becomes_chord` 和 `voice_hold_may_activate` 的用例：RAlt 与 RCtrlRAlt 按住时 F10、Q 触发组合键；CtrlWin、无按住、松开、自动重复、注入、已锁定、空格（锁定开着）、Esc、全部修饰键都不触发；空格锁定关掉时右 Alt+空格触发；组合键之后右 Alt 不能重新开始按住，其他键不受影响；`voice_chord_cancels_recording` 只在代次非 0、相等、仍在录音、未锁定时成立，按住没开始录音（代次 0）、代次变了、已停止、已锁定都不取消。在 macOS 上用 `clang++ -std=c++20 -Wall -Wextra -Werror` 编译运行通过；CI 的 `windows-voice-hotkey-policy` 目标会在 MSVC `/W4 /WX` 下再跑一次。
- `VoiceHotkey.cpp` 用 `x86_64-w64-mingw32-g++ -std=c++20 -fsyntax-only -Wall -Wextra -Werror -DMSIME_EDITION_FULL` 做了语法检查，链接和运行交给 Windows CI 与实机。
