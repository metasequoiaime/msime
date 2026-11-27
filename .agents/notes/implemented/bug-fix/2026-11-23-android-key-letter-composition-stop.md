# Agent Note: Android 上按键型的组字区不再被当成正文结束

Status: implemented

## Problem

维护者报告：Android 26 键全拼打字时，点候选、或按住候选松手之后，输入框里偶尔出现一段没有下划线的拼音；这时把消息发出去，发出去的就是那段拼音（#11831）。

组字区里写的是按键本身：全拼和双拼写拼音、五笔写码、粤拼写字母、笔画写字形。`EditorBridge.abandon` 用 `finishComposingText()` 结束这样的组字区，Android 的语义是「结束组字区、文字留在文档里」，于是按键变成正文。走到这里的路径有三条：

- 候选点选失败（旧代次的候选被引擎拒绝、输入连接异常等）→ `fail()` → `stop(false)` → `abandon`；
- 会话停下收尾（换输入框、应用改写或清空文字、同编辑器重启）→ `stop(...)` 里的 `abandon`；
- 编辑器自己动了光标 → `updateEditorSelection` 先发取消命令清掉引擎组字，再 `finishCompositionInPlace`；全拼 `drawn == settled`，`EditorBridge.finishInPlace` 走 `abandon`。

此外 `stop(finish=true)` 在收尾前发共享命令 2（`CommitRaw`），把还在的拼音原样写进文档。

共享契约 `SchemeType::commits_on_blur` 只对韩语、注音、越南语、藏文为真：它们的组字区里是用户已经写下的文字，失焦时保留；其余方案的组字区里是按键，失焦即取消。`InputSchemeTraits.locksCaret` 是同一份集合的 Java 副本。Android 的三条路径都没有按这条契约分开处理，这是缺陷而不是设计。

## Decision

新增宿主谓词 `InputSchemeTraits.composesKeyLetters(scheme)`：全拼、双拼、五笔、粤拼、笔画为真。它是宿主专用谓词，没有对应的引擎 `SchemeType` 谓词，`scripts/test-scheme-traits-parity.py` 不受影响。服务层的 `composesKeyLetters()` 在方案之外还要求视图里有组字文字，并且 `dedicatedEnglish` 为假；笔画沿用原有的 `strokeCompositionMarked()`，九键笔画组字区里是数字键，不属于这一类。

结束组字区的方式按谓词分岔：

- `EditorBridge.finishInPlace` 增加 `keyLetters` 参数。组字区仍是本桥最后写入的那一段时（`composingText.equals(marked)`），按键型方案走 `discard`，把组字区删掉、选区按删除长度左移，和笔画一直以来的做法一致；其余情况（编辑器已经改写、读不回标记）仍走 `abandon`，不去动不是自己写的文字。
- `stop(...)` 的收尾同样按谓词分岔：按键型方案 `discard`，其余方案保持 `abandon`。
- `stop(finish=true)` 发给引擎的命令由新的 `app.msime.android.policy.CompositionStopPolicy` 决定：按键型方案发共享命令 9（`Action::Finish`，按首选结束组字，留下的汉字才是用户打出来的字），其余方案仍发命令 2（`CommitRaw`，韩语音节、日文读音本身就是用户写下的文字）；`finish=false` 时两条都不发。

焦点丢失时应该结束成汉字而不是字母，`finishInputViewPresentation` 早就是这么做的（那里用命令 9，注释写明「leaves 你好 behind rather than the letters `nihao`」），这次让 `stop` 的收尾与它对齐。`finishInPlace` 与 `stop` 的「只结束组字区」原本来自[日文末尾 n 的决定](2026-10-11-japanese-display-reading-trailing-n.md)：日文罗马字要 `settle` 成ん，停会话那条路径不知道编辑器里还有没有组字区，所以不写文字。这两条对日文仍然成立——日文罗马字不在 `composesKeyLetters` 里，`settle` 与 `abandon` 的分岔没有变。

## Alternatives considered

- **所有方案在结束组字区时一律删掉。** 做法最统一，一次改完三条路径，也不会再有用按键当正文的情况。代价是韩语、注音、越南语、藏文在失焦或会话停下时丢掉用户已经写下的音节，直接推翻 `commits_on_blur` 与 `locksCaret` 的语义；日文画出的 `かn` 也再没有 `settle` 成ん的机会。这些方案这次没法逐个验收，不改。
- **结束组字区后再发 `deleteSurroundingText` 删掉字母。** 不用改 `EditorBridge` 的分岔，改动更小。但删除位置靠本桥记下的长度推算，编辑器在这期间改写了文字（发送后清空的聊天框、被重置的 Compose/Flutter 输入框）时会删到用户的字；`discard` 走 `setComposingText("")`，删的是编辑器自己报告的组字区，不会误删。
- **`stop(finish=true)` 一律改发命令 9。** 所有方案都留下确定文字，看起来更符合「离开输入框就该留下打出来的字」。但韩语、日文、注音的 `CommitRaw` 与 `Finish` 上屏的不是同一段文字（读音 vs 音节），改它们要另开一轮逐方案验收。
- **在引擎里加一条「取消组字」的命令给宿主用。** 宿主已经有命令 3（取消），`updateEditorSelection` 正在用；问题不在引擎是否取消，而在宿主怎么结束编辑器里的组字区，改引擎解决不了。

## Consequences

拼音、双拼码、五笔码、粤拼字母、笔画字形不再因为点候选失败、换输入框、光标被移走而变成正文；`onFinishInput` 时按首选留下汉字，和收起键盘时一致。

代价：

- 会话停下且不结束（`stop(false)`）时，还没转换的按键被删掉，不再作为字母留在输入框里。用户丢掉的是自己刚敲的几个键，比把拼音发出去好。
- `stop(finish=true)` 走命令 9，组字里一个候选都没有时 `Finish` 仍回退到原样上屏，字母照样会留下；那和改动前一样，是「无候选的原样上屏」这条既定语义，这次不动。
- 编辑器改写过组字区（`marked` 读不回或与本桥写入不符）时仍走 `abandon`，字母留在文档里。这与[繁简变体笔记](../feature/2026-11-12-android-traditional-chinese-variants.md)记的「接受该编辑器保留旧字形的限制」同一条边界：不改不是自己写的文字。
- 九键笔画的组字区里是数字键，`commitNineKeyLiteral` 之类「组字中误上滑仍走 `CommitRaw`」的语义没有变。
- 宿主多了一个 policy 类和一条宿主专用谓词，`EditorBridge.finishInPlace` 多一个参数，三个 JVM 冒烟的调用点跟着改。

## Verification

`platforms/android/check-host.sh` 在本机跑到 373 项 JVM 冒烟全过、213 个设备套件源文件编译通过（Android SDK 只有 `platforms;android-35` 和 `build-tools;35.0.0`，没有装模拟器和系统镜像）。新增与修改的用例：

- `CompositionStopPolicySmoke`：`finish=false` 不发命令、`finish=true` 时按键型方案发 9、其余发 2，并钉住 0/2/9 与共享命令号一致。
- `ChineseOutputCompositionSmoke.keyLettersThenCaretOut`：全拼 `nihao` 在光标移走后文档里是 `前后文`（组字被删掉，不是 `前nihao后文`），选区按删除长度左移；组字区被应用改写（`marked` 不是本桥写的那段）时一个字都不删。
- `InputSchemeTraitsSmoke`：`composesKeyLetters` 对 10 个方案的取值（0、1、2、5、9 为真）。

本机 JDK 是 27，`-Xlint:all -Werror` 会多报 `tests/keyboard/KeyboardActionRowSmoke.java` 里本来就有的 `dangling-doc-comments`，CI 的 JDK 不报这一条；本地跑的时候临时在脚本副本里加了 `-Xlint:all,-dangling-doc-comments`，仓库里的 `check-host.sh` 没有改。

没有在模拟器上跑设备验收（`tests/device/smoke.sh`、`StaleCandidateDeviceSmoke`），也没有在真机上复现报告者的场景：API 28/35 的模拟器和系统镜像这次没有装。点候选失败这条路径上「引擎拒绝 → `fail` → 删掉字母」的行为改动因此只有 JVM 层证据。
