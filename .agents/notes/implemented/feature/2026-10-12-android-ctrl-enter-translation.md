# Agent Note: Android 实体键盘 Ctrl+Enter 与屏幕键盘长按「确认」上屏候选译文

Status: implemented

## Problem

#7079（拆自 #6792 第三点）：用户接实体键盘打字时想直接打出译文。Windows、macOS、Linux（IBus、Fcitx5）和鸿蒙在组字时按 Ctrl+Enter 上屏高亮候选的译文，有几个义项时打开义项页，见 [docs/windows-parity.md](../../../../docs/windows-parity.md) 里「Ctrl+Enter 的译义副候选页」那段增量记录。Android 只有长按候选、在菜单里点译文这一条路（[长按按义项上屏](../bug-fix/2026-10-11-android-gloss-insertion-senses.md)）。

实体键盘上，`MSIMEInputService.onKeyDown` 的兜底分支只要按着 Ctrl/Alt/Meta，就先 `command(2)` 把原文上屏，再把键交给应用。所以 Ctrl+Enter 在 Android 上是「拼音上屏，应用再收到回车」，聊天应用会直接把拼音发出去。

## Decision

- 判断放在纯策略类 `policy/HardwareTranslationCommitPolicy`，只依赖 `KeyEvent` 常量，`tests/settings/HardwareTranslationCommitPolicySmoke` 逐条钉住。`ctrlEnter` 只认只按着 Ctrl 的 `KEYCODE_ENTER` / `KEYCODE_NUMPAD_ENTER`，结果有四种：`NONE`（交给原来的分支）、`CONSUME`、`COMMIT`、`OPEN_PAGE`。
- 路由排在维护组合键之后、修饰键兜底分支之前（`handleTranslationCommitKey`），`check-host.sh` 用 rg 守住 `ctrlEnter` 和 `pageKey` 都在服务里被调用。早于它的分支都不会截走 Ctrl+Enter：单按 Ctrl 在最前面被吃掉，`HardwareShortcutPolicy.chord` 不认回车，注音、`engineSpells`、数字选词和英文直输都要求没按 Ctrl。
- 什么时候接：正在组字、候选非空、`candidateGlossInsertionEnabled()` 为真。最后这个条件也是长按菜单的条件，本地模式、日语（方案 3）、韩语，以及 `schemeShowsGlosses` 排除的粤拼、注音、越南语、藏文、笔画都不接，保持原来的行为。没有组字时同样不接，Ctrl+Enter 照常交给应用。
- 释义显示开关（`candidate_english_gloss` 或账号译文）不算进「接不接」，只决定义项是不是空的。接住以后：0 个义项吃掉按键、什么也不做；1 个义项直接上屏；2 个及以上打开义项页。按住不放的自动重复只吃掉。
- 义项取高亮候选的 `translation`，经 `CandidateTranslationPolicy.insertionGlosses` 拆分：先按换行分语言行，再按 `;` / `；` 拆义项，两种目标语言都拆进来，最多 9 项。和同一平台的长按菜单是同一份列表。
- 单义项走现成的 `insertCandidateGloss(slot, id, chineseOutput(text), gloss)`，它带 `candidateIsCurrent` 的身份核对。`text` 必须是 `chineseOutput` 之后的文字，因为核对拿的就是它。上屏前先 `closeCandidatePanel()`，展开面板不会和义项页同时出现。
- 义项页是 `core/ImeTranslationSenses`，形状照 `ImeEmailSuffixes`。它记下打开时的 session 和 generation，`active()` 要求两者都没变、view 不为空、仍在组字。所以任何推进 generation 的引擎回复（继续打字、点选、换输入框）和把 view 置空的 `onUpdateSelection` 都会让它自动失效，不用每条路径记得关页。异步释义经 `applyCandidateGlosses` 换 view 时 generation 不变，页面保留。云候选和 AI 候选（包括 AI 落空时把占位换成「—」）不一样：runtime 采用它们时会像一次输入那样推进 generation（`input-runtime` 的 `apply_online_candidate_snapshot` / `resolve_online_candidates_owned`），用户却什么也没做。如果义项页因此关掉，用户接着按的数字或空格就落到候选条上，选中的是中文候选而不是正在选的译文。所以 `applyOnlineResult` 换 view 之前先调 `followOnlineRefresh`：开页时记下的组字（`editing_text`）和高亮候选原文都没变，页面就改认新的 generation；任一变了（比如 AI 候选顶到了高亮位置）页面随旧 generation 失效。这个判断在策略类的 `pageSurvivesRefresh` 里，冒烟钉住。`render()` 在邮箱后缀之前画它：候选行只放义项，按钮前标 `1 `…`9 `，高亮项画成选中，读屏是「译文 n：xxx」。横排、竖排都画，展开键和页码不显示，点按上屏，不能长按。
- 页内按键由 `pageKey` 分派，排在 `onKeyDown` 最前面，紧跟单按修饰键的处理，早于韩语 F9、数字选词和以词定字。数字 1–9（含小键盘）选第几项，超出义项数时留在页内。空格和回车上屏高亮项。上下左右移动高亮，Home/End 跳到两头。Esc 和返回只关页，组字保留，和鸿蒙的 `exitTranslationPage` 一样。单按的修饰键留在页内。其他键和任何带修饰键的组合都先关页再照常处理，同 macOS 的 `handleGlossSenseEvent`，所以 Ctrl+Enter 会关页再重新开页。
- 上屏之后用命令 3（`MSIME_CANCEL`）结束组字，和长按路径 `insertCandidateGloss` / `insertExpandedCandidateGloss` 相同。
- 被这条路接住的键记在 `translationConsumedKeyCode`。它在按住时的自动重复，以及随后的抬起，也都不交给应用，下一次首次按下或 `onFinishInput` 时清掉。义项页里只记上屏、关页和留在页内的键（`holdsKey`），不记方向键和 Home/End：自动重复的过滤排在页内分派之前，记下方向键的话它的重复到不了 `pageKey`，按住只会移动一格。

## 屏幕键盘：长按「确认」

Ctrl+Enter 只照顾到接实体键盘的人，而 #6792 第三点的原话是「希望可以推出快捷键直接可以打出」，绝大多数 Android 用户手里没有实体键盘，对他们来说还是只有长按候选、在菜单里点译文这一条路。所以屏幕键盘上组字时的「确认」（回车键）加一个长按，去向与 Ctrl+Enter 相同：

- `HardwareTranslationCommitPolicy.returnHold(eligible, senseCount)`：`eligible` 与 Ctrl+Enter 同一个条件。1 个义项 `COMMIT`，2 个及以上 `OPEN_PAGE`（义项页本来就能点选）。**没有译文时 `NONE`，长按当作点按，照常确认组字**。这一点与 Ctrl+Enter 的「吃掉」不同：屏幕上的「确认」只结束组字，不会把回车交给应用，没有把拼音发出去的风险，吃掉反而像键坏了。
- 服务里 Ctrl+Enter 和长按共用 `translationTarget()`（找高亮候选、拆义项）和 `performTranslationCommit()`（直接上屏或开页），`bindReturnTranslationHold` 由 `ImeBottomRow.ensureFaces` 在建回车键面时挂上。所有布局（26 键、九键、14 键、注音、笔画等）共用同一个 `enterButton`，日语九键的「改行」是另一个键，日语本来就不接。
- 看得见：长按能上屏译文时（`marksReturnKey`，与 `returnHold` 不为 `NONE` 同义），回车键右上角标「译」，字号和位置与字母键的角标相同（`KeyboardIconKey.setCornerHint`），读屏描述是「确认；长按上屏译文」，长按动作也叫「上屏译文」。译文是异步到的，标记随下一次 `render()` 里的 `updateReturnKey` 出现。
- 长按播放按键反馈、计一次 `Enter`。

## 和其他宿主的取舍

- **没有译文也吃键**，跟 Windows 和鸿蒙，不跟 macOS 和 Linux。macOS 和两套 Linux 前端只在「候选窗开着、释义开关打开、高亮候选确实有译文」时拦截，否则 Ctrl+Enter 照旧结束组字。issue 正文定的是 Windows 的做法：Windows TIP 只看候选列表开没开（`KeyEventSink.cpp` 的 `_candidateMode != CANDIDATE_NONE`），不看释义开关。理由是在聊天应用里，Ctrl+Enter 常常就是发送键，「这次碰巧没有译文」时把拼音上屏再发出去，比什么也不做糟得多。用户想要原来的效果（上屏原文），按 Enter 就行。
- **义项按 `insertionGlosses` 拆，两种语言都进来**，不照 Windows 只取第一种目标语言那一行（`primary_translation_line`）。同一台设备上，长按菜单和 Ctrl+Enter 给出的义项一样，用户才不会困惑；鸿蒙也是把全部义项列进页里。
- **发命令 3 而不是 19**。桌面端写入译文后发 `MSIME_CANCEL_AFTER_HOST_COMMIT`（19），让生命周期核心把这次组字记成宿主上屏、不出取消音。Android 没有调 `msime_client_set_lifecycle`，会话里没有生命周期状态，19 在这里只是带着 `InputClass::HostCommit` 去走 Cancel，结果和 3 一样（`crates/host-api/src/ffi/input.rs`）。Android 也不读回复里的 `feedback`。等移动端接入[生命周期核心](../architecture/2026-10-11-desktop-lifecycle-core.md)时，再把宿主写入的几条上屏路径一起改成 19。

## Alternatives considered

- **只在有译文、释义开着时拦截，否则照旧（macOS / Linux 的做法）**：最强的理由是不改变没开释义的用户的 Ctrl+Enter，原文照样上屏。不采用的原因是聊天应用里按键会被直接发送，而且 issue 已经定了跟 Windows。
- **把释义开关算进「接不接」**：评审时的第一版就是这样，它和「释义关着时拼音不上屏」这条设备用例互相矛盾。最后让开关只影响义项数。
- **页内按键交给引擎的候选导航，或复用展开面板**：可以少写一个组件。但释义页不是引擎状态，页里的「候选」是宿主拆出来的字符串，没有 session、generation 和 index 身份，引擎也没法给它们分页。照 `ImeEmailSuffixes` 另画一行，是同一宿主里已经验证过的写法。
- **候选栏上放一个「译」键（屏幕键盘入口）**：一眼能看到，点一下就上屏。不采用的原因是译文异步到达，这个键会在候选出来之后才插进候选栏，把候选整体往右挤，正要点候选的手指会点错；它还要占候选栏本来就紧的宽度。回车键的长按不改变任何布局，角标「译」只是在已有的键上多画一个字。
- **长按「确认」没有译文时也吃掉，和 Ctrl+Enter 一致**：两条路的规则完全相同，好记。不采用的原因见上一节：屏幕上的「确认」不会把回车交给应用，吃掉只会让人以为长按坏了。
- **义项页用 `PopupMenu`，和长按一样**：实体键盘用户的手不在屏幕上，`PopupMenu` 接不到数字键和空格，也和其他平台的义项页对不上。
- **抬起事件交给应用（评审方案原本的打算，和裸回车 `enter(); return true;` 的现状相同）**：按下被吃掉、抬起却放行，一般不会出事，因为 `TextView` 只在自己看到过按下时才响应抬起。但用户先松 Ctrl 再松回车时，这个抬起不带修饰键，个别直接监听 `KEYCODE_ENTER` 抬起的应用会执行发送。吃掉抬起只需要一个字段，按住时的自动重复也一起挡住了：第一下上屏后组字已经清空，之后的重复事件按理落进空闲状态，会被交给应用。

## Consequences

- Android 接实体键盘时，Ctrl+Enter 与桌面端、鸿蒙基本一致：单义项直接上屏，多义项可以用键盘选。没有译文时这个键在组字中不再起作用，这正是 issue 要的。
- 支持插入译文的方案里，组字中的 Ctrl+Enter 从此不再「上屏原文 + 应用收到回车」。之前依赖这个效果的用户要改按 Enter。不支持的方案和本地模式不变。
- 已知遗留：上屏的译文不随繁体输出转换，Windows 和 macOS 会转。目标语言里只有日语可能含汉字，影响很小，长按路径也一样没转。iOS 键盘扩展收不到实体键盘事件，不涉及。裸回车 `enter(); return true;` 的抬起仍交给应用，这不在本次范围内。
- 没有实体键盘的人也有了一步上屏译文的路：组字时回车键角上出现「译」，长按它。代价是回车键在组字时多了一个长按含义，平时习惯按住回车的人会触发译文上屏，但只在角上有「译」时才会。
- 义项页的失效靠 session 和 generation。引擎不回复的操作（例如只打开工具栏面板）不会关页，页面就留在候选行上，下一次组字变化时自然消失。

## Verification

- `bash platforms/android/check-host.sh`：新冒烟 `HardwareTranslationCommitPolicySmoke` 通过（也钉住 `returnHold` 与 `marksReturnKey`；把「没有译文时当作点按」改成吃掉后，冒烟在 "a held return without a gloss is a tap" 处失败）；结构守卫通过；`MSIMEInputService`、`ImeTranslationSenses` 和设备套件源文件在 `javac -Xlint:all -Werror` 下编译通过。
- 屏幕键盘长按由 `tests/device/candidate/CandidateGlossDeviceSmoke.java` 覆盖：「你好」带上 `hello` 释义后，回车键的读屏描述是「确认；长按上屏译文」，对它执行长按动作后输入框是「你好hello」；释义关着时回车键不再提供这个长按。
- 设备套件 `tests/device/keyboard/HardwareTranslationDeviceSmoke.java` 已登记进 `smoke.sh`。它用 `uinput` 接上的 USB 键盘依次覆盖：「你好」单义项 `hello` 直接上屏；「我的」两个义项开页后按 2、Esc 关页后组字还在、空格上屏第一项；释义关着时吃键；空闲时不接。`uinput` 键盘的辅助方法从 `CaretCandidatesDeviceSmoke` 提到 `DeviceSmoke`，加了 `ctrlChord`，所以 `CaretCandidatesDeviceSmoke` 也要一起重跑。这两个套件写下这篇笔记时还没在模拟器上跑过。设备 CI（`android-device.yml`）在 PR 上不跑，合并前要在本机 API 35 上跑 `smoke.sh`。API 28 只能在 `hw.keyboard=yes` 的 AVD 里手工验证。
