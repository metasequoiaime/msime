# Agent Note: IBus 成对标点补全后光标没有退回两半之间

Status: implemented

## Problem

设置项「成对标点自动补全」的文案承诺「输入左侧符号时自动补全右侧符号，并将光标置于中间」，但 Linux IBus 宿主在任意应用、所有成对标点上，光标都停在右半之后。Fcitx5、macOS 和 Windows 没有这个现象。

## Decision

IBus 没有直接设置文档光标的接口，宿主原来在一次性 commit 整对之后，用 `ibus_engine_forward_key_event(engine, IBUS_Left, 0, 0)` 转发一个人工合成的 Left 键来退光标。这里有两处缺陷：一是 `keycode` 传 0，而 IBus 的 `keycode` 是硬件扫描码，0 会让客户端构造的按键事件无效；二是打 `"` `(` `{` 等成对键都按着 Shift，Wayland 下虚拟键盘沿用物理修饰状态，此时转发方向键会被应用当成 Shift+方向键选中文字。

改动：新增纯策略 `PairedCaretForward::defer()`，Wayland 且 Shift 仍按着时把方向键排队，等 Shift 松开再补发，与 Fcitx5 `forwardCaret()` 一致；转发的方向键从 `us` 键位表查出有效扫描码（Left 105、Right 106），不再是 0；跨过已有右半的 `try_skip_paired_closing()` 同样走这条路。策略判定单独成头文件，用行为断言覆盖四种会话/Shift 组合。

首轮只把扫描码从 0 改成有效值后，报告者实测光标不再停在右半之后，却变成「连续移动很多下直到行首」。根因是每次转发只发了一个方向键按下事件、没有配对的松开事件：客户端把方向键当成一直按着，触发键盘自动重复，光标一路冲到行首。这是照抄 Fcitx5 时漏掉的一处——它的 `forwardCaret()` 每次都 `forwardKey(Key(sym), false)` 加 `forwardKey(Key(sym), true)` 成对发送。补上：抽出 `emit_paired_caret()`，同一扫描码先发 `state=0` 的按下、再发带 `IBUS_RELEASE_MASK` 的松开；即时转发和 Shift 松开后的补发都走它。

## Historical audit

- [Windows 组字细节对齐 macOS](../../implemented/feature/2026-10-10-windows-input-compose-mac-parity.md)：成对标点覆盖面与宿主侧光标定位的旧决定；本次不改覆盖面，只修 IBus 转发按键的扫描码与时机。
- [macOS 成对键文档读取](../../implemented/bug-fix/2026-11-17-macos-paired-key-document-reads.md)：同一功能的 macOS 侧跨过分支；不涉及 IBus。

## Alternatives considered

- 只把 `keycode` 换成 `process_key` 收到的标点键扫描码：扫描码对应的是标点键而不是方向键，客户端按键事件里的键值与扫描码不一致，不可靠。
- 改成前缀先上屏、右半边做 preedit 并把光标放在其前（macOS 的做法）：能绕开转发按键，但要给宿主引入待定右半边的状态机，改动大且会改变可见的预编辑状态；本次先用最小改动对齐 Fcitx5，若转发键在某些应用里仍不生效再考虑。

## Verification

- Linux `ctest` 115/115，含新增的 `PairedCaretForward::defer()` 四组合断言；CMake 全量构建通过；重打并覆盖安装 `.deb`（0.17.0）。
- 用独立 IBus 客户端连到本机 daemon 发 `"` 并监听信号：commit `“”` 后收到**一对** `forward-key-event`——keyval=0xff51(Left)、keycode=105，一条 state=FORWARD_MASK（按下）、一条 state=FORWARD_MASK|IBUS_RELEASE_MASK（松开），不再是单个 keycode=0 的按下事件。Wayland 会话下先按 Shift 再打 `"` 时，Shift 按住期间只有 commit、没有转发；松开 Shift 后才收到成对的 Left 按下/松开。
- 限制：本会话的 Wayland/XTEST 环境里 xdotool 按键到不了 gedit，没能自动抓真实编辑器里的光标画面；光标最终是否落在两半之间仍需报告者在真实应用里确认。不改默认值、不加设置入口，版本号保持 0.17.0。

## Consequences

修的是 IBus 宿主侧的转发按键，不改引擎标点契约、不改其他平台、不碰设置默认值。代价是 Wayland 下退光标延后到 Shift 松开；极端连打且始终不松 Shift 时，下一字符会先于退光标落入文档，与 Fcitx5 现有行为一致。
