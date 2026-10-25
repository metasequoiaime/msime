# Agent Note: 三端移动键盘部分选词后保留展开候选面板

Status: implemented

## Problem

#9193：Android 上打一段词组或长句，展开候选面板逐字选择，选了第一个音节的字以后面板收起，回到候选条，要再点展开才能接着选下一个字。报告人给的三款参照输入法录屏都是面板留着、直接换成剩下音节的候选（选完「同」直接选 tong 后面那个字）。

收起不是偶然的。#9107 修了「选词后展开页盖着键盘、必须按返回」，此后选中面板里的候选一律先把面板标成收起再选词；#9202 的九键四步回归把「每选一个字面板自动收起、再点展开」写成了验收条件。[九宫格展开面板](../feature/2026-10-08-nine-key-expanded-panel.md)当初的决定也是「只有选中候选、点返回或组字结束才关，关闭时清掉全部筛选」。iOS（`onSelect` 先 `closeCandidatePanel` 再选词、再清筛选）和 HarmonyOS（`chooseAny` 之后 `show(SURFACE_NONE)`）是同一条规则。所以三端都要改，而不是只修 Android。

## Decision

面板里选中一个候选之后，选词被引擎接受、会话没换、组字还在（`editing_text` 非空，鸿蒙用 `KeyboardSession.composing()`）时面板留着，换上剩下组字的完整候选；整句上屏、组字清空、引擎拒绝了过期的一代或会话变了，照旧收起。判断各端一份，规则相同：Android `candidate/CandidatePanelSelectionPolicy.java`，iOS `KeyboardExtension/Sources/keyboard/CandidatePanelSelectionPolicy.swift`，HarmonyOS `keyboard/candidate/CandidatePanelSelectionPolicy.ts`。九键三栏面板和普通候选网格都适用，剩下的候选只够一页也留着：用户正在面板里逐段选词，收起只会让下一段再点一次展开。这与 macOS 的[部分选词保留展开](2026-10-25-macos-expanded-partial-selection.md)同一方向。

九键面板的筛选：「只留单字」保留，它本来就作用到组字结束，逐字选时正好不用每段重开；笔画前缀清空，因为引擎比的是候选的第一个字，而那个字刚被选掉，留着它会让下一个音节几乎没有候选。左栏是拼音还是笔画键不变。面板最终收起（返回、整句上屏）时照旧按原规则清掉剩下的筛选。

各端的接法：

- **Android**：`ImeCandidates` 的格子点击交给 `MSIMEInputService.selectPanelCandidate`。选词（以及清笔画）那次 `apply` 用 `renderingSuppressed` 压住重画：面板此时已标成收起，画一次会先把面板收掉（九键面板还会复位左栏、排队清筛选），紧接着又重开。状态定下来以后，留着就走 `openCandidatePanel(true)` 取新一代快照再画一次，不留就直接画。手指还按着候选时这次 `render` 照旧被 `candidateTreeHeld` 延后，松手补的那一帧按定下来的状态画；面板内容跟的是 `candidatePanelSnapshot` 的代次，不靠 `View.isPressed()`，所以不会重现 #9107 的「补帧前状态已过去」，也不会留下旧一代的格子。普通网格在 `renderExpandedCandidates` 里遇到代次不符仍然收起，部分选词路径在画之前已换上新快照，走不到那里。
- **iOS**：`selectPanelCandidate` 先选再决定收不收（先收起再清筛选会换一代，这次选择就过期了）。画这次快照时 `candidatePanelFollowsSelection` 让 `refreshCandidatePanelAnnotations` 对普通网格也按新的一代原地 `reload`，与九键面板换代的处理相同；画完按策略决定是否 `closeKeyboardPicker()`。
- **HarmonyOS**：`chooseAny` 返回引擎是否接受。选词的回报经 `onChanged` 同步走到 `refreshExpanded`，面板留着就已经是新的一代；不留时 `show(SURFACE_NONE)`，`leaveNineKeyPanel` 照旧清筛选。

## Alternatives considered

- **只改 Android**：issue 只报了 Android，改动最小。但移动键盘的交互三端保持一致是产品规则，iOS 与鸿蒙用同一条「选中即收起」的规则，只改一端会让同一个用户在两台设备上得到相反的手感。
- **只让九键三栏面板保留，普通网格照旧收起**：九键面板本来就在换代时重建，改动最少，录屏里也是九键。但 26 键全拼、双拼长句逐词选择是同一个需求，普通网格收起后要再点展开，问题原样留着；macOS 的决定同样覆盖了普通网格。
- **任何组字变化都保留展开**（包括打字、退格、异步候选换代）：规则统一。但会改变普通网格在打字和异步回包时收起的既有生命周期，范围超出报告；这里只在面板里主动选词的那次回报上保留。
- **部分选词后连单字筛选一起清掉**：与原来「收起时清掉全部筛选」一致。但用户开着「单字」正是为了逐字选，每段都要重开一次，抵消了这次改动的意义；引擎给单字筛选的语义本来就是作用到组字结束。
- **Android 不压重画，选完让 `render` 先收起再重开**：少一个标志位。但两次整树重画里第一次会触发九键面板 `dismiss()` 的复位和清筛选排队，第二次再靠排队里的判断撤销它，顺序依赖更脆。

## Consequences

- **收益**：三端在面板里逐字、逐词选完一整句只需展开一次；「单字」筛选跨段保留。
- **代价**：部分选词后要多取一次完整候选（Android 的 `allCandidates`、iOS 的 `allCandidates`、鸿蒙的 `allCandidates`），与面板里选拼音、筛选时的代价相同。普通网格在剩一页时也会留着，候选条上看得到的候选在面板里重复一遍，用户可以点返回或展开键收起。
- **改写的回归**：Android `NineKeyPanelDeviceSmoke -e partial-selection true` 改为只展开一次，每选一个字检查面板仍在、编辑器保留已选前缀与组字标记、单字筛选仍开着，下一个字要从换代后的面板里点到（你、好交替，旧一代里没有下一个字），最后一个字整句上屏后面板消失；[候选条按下期间不重画](2026-10-18-android-candidate-strip-rebuild-drops-tap.md)里对这条回归的描述随之更新。
- **验证**：Android `check-host.sh` 通过（274 项 JVM 冒烟，含 `CandidatePanelSelectionPolicySmoke`；243 个设备套件源文件编译通过）。iOS 在 iOS 27.0 模拟器上跑 `CandidatePanelSelectionTests` 与 `NineKeyKeyboardTests` 全部 123 项通过（暂存了笔画词库，笔画相关用例不跳过），其中新增的两条驱动整个视图控制器：九键逐字（单字加笔画 `p`，选「你」后面板仍是同一个、出现「好」、笔画前缀清空，选「好」后整句上屏收起）和全拼普通网格（`nihao` 选「你」再选「好」）。HarmonyOS `tests/run.sh` 通过（含新增策略用例），`scripts/test-harmony-arkts-subset.py`、`expanded-candidates`、`unwired-policies`、`unwired-symbols` 文本门禁通过。没有覆盖的：Android 改写后的设备回归这次没有在模拟器上跑（本机没有空闲的专用 AVD，磁盘也不够新建一台）；鸿蒙 `.ets` 改动没有经过 `hvigorw assembleHap` 编译，也没在设备上看过；三端都没有在报告人的 vivo/Android 11 真机上验收。
