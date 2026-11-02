# Agent Note: Windows TIP 以已存的起始标点开始中文模式

Status: implemented

## Problem

#10646（Windows 10、五笔、0.4.1）：在设置「标点与翻译 › 标点」里选「英文标点」后，切到别的输入法再切回水杉，标点又是中文标点。

设置确实写进了偏好文件：WinUI 设置页按 `PunctuationSettings.h` 的 `mode_patch` 写 `chinese_punctuation=false`、`punctuation_lock=follow`。问题在 TIP：`InitializeMetasequoiaIMECompartment` 在每次激活时调 `SetPunctuationMode(openChinese)`，`SyncPunctuationWithImeMode` 和 OPENCLOSE 回调在每次中英文切换时也把标点设成「中文模式即中文标点」，经 `Ipc.h` 的 `ResolvePunctuationOpen` 只看锁定、从不读 `chinese_punctuation`。设置页随后写出的标点只在用户当次按 Ctrl+. 或工具栏切过之后才看得到，切走输入法（Deactivate 清空 compartment）或中英文往返一次就回到中文标点。[表达页精简](../../implemented/simplification/2026-10-16-expression-page-prune.md)当时把这件事写进了 Windows 设置页的说明（「中文标点和英文标点只决定同一账号其他设备上的起始标点」），并把改 TIP 列为「Alternatives considered」里暂不做的一项；用户照界面上的选项选了英文标点，并不会去读那句说明。

## Decision

TIP 把「标点回到哪里」分成两条规则，放在不依赖 Windows 头文件的 `tsf/Global/PunctuationModePolicy.h`：

- `PunctuationOpenForImeMode(lock, imeOpen, savedChinesePunctuation)`：激活和中英文切换用。锁定时取锁定值；跟随时英文模式用英文标点，中文模式用已存的 `chinese_punctuation`。
- `PunctuationOpenForRequest(lock, requested)`：Ctrl+.、工具栏、托盘和应用改 conversion mode 这些显式请求用。锁定时取锁定值，跟随时照请求，起始标点不参与，所以英文起始下 Ctrl+. 仍能临时打开中文标点。

`Ipc.h` 的 `ResolveImeModePunctuationOpen` / `ResolvePunctuationOpen` 分别包这两条，读进程内的 `PunctuationLockMode` 和新增的 `SavedChinesePunctuation`；`FanyUtils::RefreshPunctuationLockFromConfig` 从内存快照同时刷新两者（键缺失或类型不对时按出厂的中文标点）。接线：`SyncPunctuationWithImeMode` 先刷新再按模式规则定标点，激活改为调它；OPENCLOSE 回调和 `KeyEventSink` 里中英文切换的延迟投影改用模式规则；Ctrl+. 的投影、`SetPunctuationMode` 本身和 `WM_ApplyPunctuationLock` 保持请求规则。

Windows 设置页的「标点」说明改回与共享页 `PunctuationModeRow` 相同；工具栏预览的标点按钮按同一条模式规则画（`toolbar_chinese_punctuation` 多收已存的起始标点），WinUI 和 Win7 两处调用一起改。

出厂默认值不变：`chinese_punctuation` 仍默认为真，没有改过这个键的用户行为与以前完全相同。

### 其他平台

- macOS 不受影响：`AppearancePreferences` 的 `runtimeChinesePunctuation` 没有运行时覆盖时取 `!englishMode && chinesePunctuation`，回到中文模式时丢掉覆盖、回到保存值（[macOS 默认英文启动标点](2026-10-22-macos-initial-english-punctuation.md)）。
- Linux 的 IBus 与 Fcitx5 在 #9980 已改成同一规则（[Linux 模式往返保留英文标点](2026-10-27-linux-saved-punctuation-after-mode-switch.md)）。本篇与它们是同一条规则在 Windows 上的落地，三端现在一致。

## Alternatives considered

- **保持现状，只靠设置页说明。** 最强的理由是与来源 MSIME-Windows 的行为一致（来源里中文模式总是中文标点），改 TIP 涉及激活、中英文切换和延迟投影几处，只能交叉编译、要真机才能完全验收。没有采用：设置页给了「英文标点」这个选项，用户选了它就期望它生效，#10646 正是这样理解的；macOS 和 Linux 已经按保存值起步，Windows 是唯一让同一个选项失效的桌面平台。说明写得再清楚，也不能替代选项本身生效。
- **在 `SetPunctuationMode` 里统一读起始标点。** 最强的理由是只改一处。没有采用：`SetPunctuationMode` 同时服务 Ctrl+. 和工具栏这些显式请求，在那里套起始标点会让英文起始下 Ctrl+. 再也打不开中文标点。模式规则和请求规则必须分开。
- **把 Ctrl+. 的临时选择写回 `chinese_punctuation`，让下次激活沿用。** 最强的理由是「上次用什么下次还是什么」。没有采用：`chinese_punctuation` 会上云，按键随手一切就改写其他设备的起始标点；macOS、Linux 都只把它当会话内的临时覆盖，Windows 照同样做。

## Consequences

- **收益**：选「英文标点」的 Windows 用户在激活、切走再切回、中英文往返之后都得到英文标点；从别的设备同步来的起始标点在 Windows 上也生效，与 macOS、Linux 一致。设置页说明不再需要单独解释 Windows 的例外。
- **代价**：Ctrl+. 或工具栏的临时选择在下一次激活或中英文切换后回到起始标点，这与以前回到「中文模式即中文标点」是同一类重置，只是落点变成用户选的值。以前在别的设备上选过英文起始、在 Windows 上一直看到中文标点的用户，更新后 Windows 会变成英文标点；这是那个选项本来的意思。
- **验证缺口**：本机只能 MinGW 交叉编译（`build-cross.sh x64`），`msime-tsf-punctuation-mode-policy` 的规则和接线检查在 macOS 本机编译运行通过、对着 `origin/develop` 的源码失败；没有在真实 Windows 的 TSF 里切换输入法验收，合并后需要报告者在 Windows 上复验。
