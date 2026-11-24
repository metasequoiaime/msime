# Agent Note: iOS 纠错候选标 `*`，不再画打出的原串

Status: implemented

## Problem

引擎改过一行拼写的候选在 View JSON 里带 `corrected: true`（[双拼触点纠错](../feature/2026-10-27-shuangpin-touch-correction.md)与[全拼乱序纠错](../feature/2026-10-09-shuangpin-typo-sentence.md)都会产生这种行）。Windows、macOS、Linux 在候选字后面接 `*`，Android 自 #11578（[候选来源标记](../feature/2026-11-22-android-candidate-source-marks.md)同一份 `CandidateCorrectionMarkPolicy`）起也这样做；iOS 的三个候选面完全不读这个字段，所以纠错行和普通候选长得一样。

没有辅助码时，引擎把原串（`corrected_from`，如 `zhognguo`）放进 `annotation`。iOS 把它当编码提示画出来：候选条上接在候选字右边，展开网格里在候选字下面另起一行。这一串和输入框、读音行里的拼写完全重复，加上没有 `*`，用户看不出这一行是引擎改过拼写的。读屏还把原串念成「还需输入 zhognguo」，可它并不是还要输入的键（#11844）。

## Decision

- **读键**：`MetasequoiaInputSessionBridge` 的 `MetasequoiaInputSnapshot` 新增 `candidateCorrected`，按 `strictBool(candidate["corrected"], fallback: false)` 与 `candidates` 对齐解码，缺键按假、字符串冒充时整份回应作废（与 `highlighted`、`confident_correction` 同一条严格规则）；`CandidatePanelSnapshot.Entry` 多一个 `corrected`。候选条按 `CandidateMarker.visibleIndices` 过滤后对齐，展开网格直接按 `entries`。
- **画法**：候选字后面画 `*`，在 `CandidateCorrectionPolicy.markedWord` 一处实现，候选条（`updateCandidateButton` 的 `title` 与 `attributedTitle` 两条路径）和展开网格（`KeyboardCandidatePanelView.makeChip`）共用。`*` 只是画出来的，不上屏、不参与选择，读屏标签不经过它。位置、字色与字重沿用候选字本身；坐上首选的双拼纠错仍走原来的强调色和「，已纠正」，两条标记互不替代。
- **注释**：`CandidateCorrectionPolicy.visibleAnnotation` 只去掉注释打头的那一段原串。以「(」开头的是辅助码，照旧画；五笔方案里混输拼音的纠错行是「原串 五笔码」，只去掉原串、留下五笔码；原串没有空格时整段去掉，注释行因此消失，候选格高度随内容收缩。Android 的 `visibleAnnotation` 是同一条规则，iOS 照它实现，不在视图层拼字符串。
- **读屏**：原串照旧念，但说成「纠错前的拼写 zhognguo」（`CandidateCorrectionPolicy.spokenAnnotation`），不再说「还需输入」。带五笔码时接在它后面念「还需输入 wq」，带辅助码时按原来的「编码提示」。候选条的可访问标签改成只要 `hintSpoken` 非空就带上它，否则纠错行不画注释时会把这一句念丢。
- **展开网格同样念**：`candidatePanelAnnotation` 以前在可见注释为空时返回 `.none`，纠错行的原串去掉后就没有读法了；现在只要读法非空就保留一个可见注释为空的 `KeyboardCandidateAnnotation`，格子不加行高，读屏照念。

## Alternatives considered

- **在 `KeyboardViewController` 与 `CandidatePanelView` 各自判 `corrected` 拼标题**：改动行数更少，但同一条规则会在两处、三个渲染路径（`title`、`attributedTitle`、面板格）各写一遍，以后改标记符号或改注释规则要同时改三处，Android 已经用一份策略类收着这条规则。
- **注释整段丢掉，不再区分辅助码和五笔码**：实现最简单，但辅助码和五笔反查编码是用户正当要看的信息，丢掉它们等于让纠错行少了一半提示；issue 明确要求「辅助码、五笔编码照旧显示」。
- **把 `*` 也放进读屏标签**：iOS 的候选读屏是逐个念出来的，`*` 会被读成「星号」，和 Windows 候选窗的视觉标记不对应；其他移动端也不念它。
- **保留注释并把 `*` 画成下划线或角标**：与 Windows、macOS、Linux、Android 已经在用的后缀 `*` 不一致，用户在两端看到的标记会不同。

## Consequences

- 默认视图（引擎不发 `corrected`，或这一页没有纠错行）逐字节不变：所有新字段都有默认值，`markedWord` 只在为真时接标记。
- 纠错行的候选格会宽出一个 `*`，没有注释时矮一行；候选条的宽度按带标记的标题重新计算（`pinCandidateWidth` 走的是 `attributedTitle`），不会截断。
- 读屏说法从「还需输入 <原串>」变成「纠错前的拼写 <原串>」，带五笔码/辅助码的组合也一并保留。
- HarmonyOS 仍是「原串照旧画在旁边」（[HarmonyOS 标出坐上首选的双拼纠错](../feature/2026-10-29-harmony-shuangpin-correction-mark.md)），本 issue 不涉及；要同样去掉原串应另开一条。

## Verification

- `CandidateCorrectionTests`（`MSIMEKeyboardTests`）钉住：标记只加在纠错行；辅助码照旧画、五笔混输只去掉原串、原串无空格时整段去掉；读法说「纠错前的拼写」而不是「还需输入」；展开网格的标题带 `*`、读屏标签不带、缺省的 `corrected` 按未纠错读。
- `InputBridgeResponseTests` 的纠错夹具补断言 `candidateCorrected` 逐行对齐、换上屏文字时保留。
- iOS 模拟器（本机 iPhone 16 Pro，iOS 18.4）整套 `MSIMEKeyboardTests` 1413 通过；`ReadOnlySharedStateTests` 的两条只读桥用例失败，在未改动的 develop HEAD 上以同一套资源、同一个模拟器、同一份工程复跑同样失败，属本地只读词库夹具的环境问题，与本改动无关。
- 没有在 iOS 真机上看过画面：引擎已经在 View JSON 里发 `corrected`，其余宿主早已按它显示，本次只让 iOS 开始读它。
