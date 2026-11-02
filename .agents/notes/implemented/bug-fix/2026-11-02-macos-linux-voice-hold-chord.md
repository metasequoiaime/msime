# Agent Note: macOS 与 Linux 按住右 Option / 右 Alt 说话时按组合键，取消这次按住开始的录音

Status: implemented

## Problem

Issue #10580 是 [Windows 那次修复](2026-11-02-windows-voice-ralt-chord.md)（#10518、#10581）的跟进：macOS 的右 Option、Linux IBus 和 Fcitx5 的右 Alt 也是「按住说话」，要按同一个验收标准对齐：按住期间按了别的键，应用收到组合键，按住开始的这段录音取消、不上屏；单按说话、空格锁定、Esc 取消不回归；出厂默认不变。

查下来，这三个宿主的问题和 Windows 不同。Windows 的组合键退化成裸键，是因为 Server 的低级键盘钩子在系统层把右 Alt 吞了，系统按键状态里没有 Alt。macOS 的 IMK、Linux 的 IBus 和 Fcitx5 都在客户端的事件管线里，吞掉的只是右 Option / 右 Alt 自己的事件，后面按键事件自带的修饰状态不受影响：

- macOS：`VoiceHoldShortcut.h` 只吞右 Option 的 flagsChanged，keyDown 原样放过；keyDown 的 `modifierFlags` 仍带 Option，`InputController.mm` 里「带 Command / Control / Option 的键结束组字后返回 NO」那一条把它交给应用。
- IBus：`ClientEngine.cpp` 只把 Alt_R 放进 `voice_consumed_keys`，其他键带着 X 服务器或合成器给的 `IBUS_MOD1_MASK` 往下走，没有剥修饰位的地方。
- Fcitx5：`FcitxEngine.cpp` 只设 `voice_ralt_held_` 并吞掉 Alt_R 自己。

真正缺的是取消。按住时按组合键，录音继续，松开后识别结果照样上屏：IBus 和 Fcitx5 只有 Esc 会取消，松开右 Alt 时 `voice_stop` / `stopVoice` 提交结果；macOS 上 keyDown 已经会取消豆包和 live（本地 helper、socket）录音，HTTP 提供商的录音不在其中，松开右 Option 时走 `toggleVoiceInput` 到 `finishHTTPVoiceInput`，结果上屏。

## Decision

三个宿主采用 Windows 的规则，只是不补发修饰键：按住右 Option / 右 Alt（以及右 Control+右 Option、右 Ctrl+右 Alt）期间新按下一个非修饰键，就当作组合键。按住就此结束，只取消这次按住自己开始、仍在录音、没有锁定的那段录音，这个键照常交给应用。

不算组合键的键和 Windows 一致：修饰键（还在拼组合）、Esc（保持取消）、空格锁定开着时的空格（保持锁定）、已经用空格锁定的按住（用户要留下的听写）。Control+Command（macOS）和 Ctrl+Win（Linux）吞的是 Command / Win，不在范围内。

「这次按住自己开始的录音」各宿主按自己的状态判断：

- macOS：`MSIMEVoiceHoldShortcut` 新增 `Action::ChordCancel`。非自动重复的 keyDown（Esc 除外，空格锁定开着时空格在前面已经处理）在未锁定的 RightOption / RightControlOption 按住期间结束按住；`recording && finishOnRelease_`（按住开始时没在录音、现在在录）才报 ChordCancel，否则报 None。按键不吞（`consumed = false`）。`suppressed_` 保留，右 Option 的松开仍被吞掉，因为应用从没收到过它的按下。`InputController.mm` 收到 ChordCancel 且 `_voiceHoldGeneration == _voiceGeneration` 时调用 `cancelHTTPVoiceInput`、`cancelDoubaoVoiceInput`、`cancelLiveVoiceInput`，然后不 return，按键继续走到「带 Option 的键交给应用」那一条。
- IBus：`State` 新增 `voice_hold_generation`。按住开始时，只有「按之前没在录、按之后在录」才记下 `voice_generation`，接管或停止已有录音时记 0。组合键判定放在 `voice_toggle_hotkey` 之前：`voice_hold_key == IBUS_Alt_R` 且 `voice_hold_becomes_chord` 成立时清掉 `voice_hold_key`、`voice_requires_control`、`voice_hold_generation`；代次非 0、等于当前代次、仍在录音、没在停止时 `voice_cancel`。Alt_R 留在 `voice_consumed_keys` 里，它的自动重复和松开照旧被吞，松开时因为 `voice_hold_key` 已清零，不再 `voice_stop`。`voice_hold_generation` 在 State 重置、`focus_out`、`reset` 三处和 `voice_hold_key` 一起清零。
- Fcitx5：新增 `voice_hold_started_` 和 `voice_ralt_chord_until_up_`。按住开始时 `voice_hold_started_ = !voice_loading_`（开始前没在录），`requestVoice` 开始任何新录音时先把它清成 false，所以别处后来开始的录音不会被当成这次按住的。组合键分支放在 Alt_R 按住分支之前：结束两种按住，`voice_hold_started_ && voice_loading_ && !wave_overlay_.processing()` 时 `cancelVoice()`，置 `voice_ralt_chord_until_up_`，不 return。紧接着的分支在松开前吞掉 Alt_R 的自动重复和松开，既不重新开始按住，也不 `stopVoice`；右 Ctrl 的松开照常交给应用，它的按下应用是收到了的。两个新成员在 `close()` 里和其他按住标志一起清零。

Linux 两个宿主共用纯函数 `voice_hold_becomes_chord`（`platforms/linux/src/voice/VoiceHoldChord.h`）。键值是 X keysym，修饰键逐个列出：Shift、Control、Meta、Alt、Super、Hyper 的左右键，Caps_Lock、Shift_Lock、Num_Lock、ISO_Level3_Shift、ISO_Level5_Shift、Mode_switch。IBus 本来有个 `modifier()`，但 Fcitx5 那边没有对应的；`fcitx::Key::isModifier()` 不含 ISO_Level3_Shift 和锁定键，两边用不同的判定会在 AltGr 布局上分叉，所以抽成一份共用的。

出厂默认不变。

## 为什么不补发修饰键

Windows 要用 `SendInput` 补发右 Alt 和这个键，是因为系统层已经没有 Alt 了。这里修饰状态就在后续按键事件里：macOS 是 `NSEvent.modifierFlags`，X11 是 X 服务器给的状态位，Wayland 是合成器在 `zwp_input_method_v2` / text-input 那一侧维护的修饰状态。输入法只要不吞这个键，应用收到的就是组合键。再补一次反而会让应用收到两份按键。

这一点是按事件模型推出来的，没有在设备上测过。Wayland 下合成器在输入法吞掉 Alt_R 之后是否照常把修饰状态发给应用，要实机确认。如果某个合成器不发，组合键在那里仍会退化成裸键，到时要另做补发（IBus 的 `forward_key_event`，或 Fcitx5 经 virtual keyboard 转发）。

## Historical audit

- [Windows 按住右 Alt 说话时再按别的键](2026-11-02-windows-voice-ralt-chord.md)：部分重叠，互链。规则相同，那篇管 Windows 的钩子与补发，本篇管 macOS 和 Linux 的取消；那篇「Consequences」里「另开 issue 跟进」指的就是本篇。
- [Fcitx5 语音取消](2026-10-11-fcitx-voice-cancellation.md)、[IBus 语音取消](2026-10-11-linux-ibus-voice-cancellation.md)：无冲突。组合键走的就是那两篇定下的 `cancelVoice` / `voice_cancel`，取消后迟到的结果不上屏由它们保证。

## Alternatives considered

- **IBus 直接复用文件里的 `modifier()`，Fcitx5 用 `fcitx::Key::isModifier()`**：不用新增文件。不采用：两者的键表不一样，`isModifier()` 不含 ISO_Level3_Shift、Caps_Lock、Num_Lock，同一个按法在两个宿主上一个算组合键、一个不算；而且散在两个大文件里的判定没法单独测。
- **macOS 统一成「只取消这次按住开始的录音」，连 keyDown 取消豆包和 live 录音那条也改**：和 Windows 完全一致，Ctrl+F9 开始的 live 听写在按住期间按组合键也不会被取消。不采用：那条「录音中打字即取消」是 macOS 早就有的语义，不只管按住，改它是另一个决定，不在这个 issue 里做。
- **组合键时 stop 而不是 cancel**：保留已经录到的那一小段。不采用，理由同 Windows：用户按的是快捷键，不是在说话，几百毫秒的录音上屏只会多出噪声识别的字。

## Consequences

收益：语音开着时，按住右 Option / 右 Alt 再按 F10、字母等组合键，按住开始的录音被丢掉、不上屏，应用照常收到组合键；HTTP 提供商的 macOS 录音也一样。

代价：macOS 上右 Option 加字母打特殊字符（如 Option+e 的重音）现在会取消按住开始的录音，字符照常输出，行为变了，但更符合用户意图。Linux 的 AltGr 布局里右 Alt 是 ISO_Level3_Shift 而不是 Alt_R，不触发按住，不受影响。Ctrl+F9 开始的 live 听写在 macOS 上按住期间按键仍会被取消（既有语义），和 Windows、Linux 不同。

没有在设备上验证的部分：macOS 上 Keynote、终端（Option as Meta）里右 Option+F10 / 字母能收到组合键、录音浮层消失、不上屏，分别用 HTTP 和本地 helper 提供商；右 Control+右 Option 同样。Linux X11 与 GNOME Wayland（IBus）、KDE Wayland（Fcitx5）下右 Alt+F10、LibreOffice / Firefox 里 Alt+F 收到 Alt 组合，重点是 Wayland 下修饰状态在输入法吞掉 Alt_R 之后仍然送达；Ctrl+F9 开始的听写按住右 Alt 再按键不被取消。

## Testing

- `platforms/macos/tests/voice/VoiceHoldShortcutTest.mm`：右 Option 按住、录音中按 F10 报 ChordCancel 且不吞；之后的键不再算组合键；右 Option 松开被吞且不再结束录音；右 Control+右 Option 同样；自动重复不触发，松开照常结束录音；空格锁定后不触发；按住前就在录音、按住开始却没录上时只结束按住、不报取消；Esc 仍是 Cancel；Control+Command 和单按左 Option 不触发。改动前这组用例在 F10 那一步断言失败，改动后通过。
- `platforms/linux/tests/voice/voice_hold_chord.cpp`（`linux-voice-hold-chord`）：头文件里的数值逐个对上 IBus 的 keysym 常量；修饰键和锁定键不触发，功能键、字母、数字、方向键触发；Esc 不触发；空格只在锁定关掉时触发；已锁定时什么键都不触发。
- `platforms/linux/tests/runtime/engine_smoke.cpp`：右 Alt 与右 Ctrl+右 Alt 按住开始录音后按 Alt+F10，按键交给应用，提供商收到取消，右 Alt 的重复不开始新录音，松开被吞且没有停止请求，迟到的结果不上屏；Ctrl+F9 开始的听写再按住右 Alt 按组合键，不取消、不停止，之后 Ctrl+F9 停止并上屏。这段要已校验的词库目录，CI 的 `build-container.sh` 只编译不跑。
- `platforms/linux/fcitx5/tests/native.cpp`：按住开始的录音被组合键取消、按键未被接受、Alt_R 的重复和松开被接受；接管的听写不被取消、松开也不停止；空格锁定后组合键不结束按住。同样要词库目录才跑。用例开头先把浮层复位到「正在录音」，和 `requestVoice` 一样：前面的语音用例会把浮层留在识别中，那时 `wave_overlay_.processing()` 为真，组合键按规则不取消。
- 验证方式：`build-container.sh` 全量编译并跑 ctest（122 项全过，含 `linux-voice-hold-chord`）。另在同一个构建镜像里用 `install_resources` 取回已校验词库，直接运行 `ibus-engine-smoke <词库>` 和 `fcitx5-native-test <词库>`。这两个程序在本次改动之前就有和语音无关的断言失败（候选辅助行光标、临时日语模式、智能标点上下文、Fcitx5 的标点样式和韩文汉字读音等），会在走到语音段之前抛出，所以临时把 `require` 改成只打印、不抛出（不提交）再跑：修复前，新增的组合键用例在 IBus 和 Fcitx5 上都失败；修复后全部通过，其余失败两次一致，与本改动无关。
- CI 里守住宿主层行为的只有纯函数测试 `linux-voice-hold-chord`：IBus 不带参数的默认分支没有注册成任何 ctest 项，`fcitx5-native-context` 只在手动运行的 `check-container.sh --fcitx5` 里注册。修掉那些无关失败、让两段语音用例进 CI 的工作见 #10723。
- macOS 的 `InputController.mm` 那条 ChordCancel 分支（代次判断、三个 cancel 调用、按键继续交给应用）没有单元测试，InputController 没有单测框架。要在设备上确认：HTTP 提供商下按住右 Option 再按键，录音取消、不上屏。
