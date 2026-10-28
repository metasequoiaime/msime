# Agent Note: HarmonyOS 标出坐上首选的双拼纠错

Status: implemented

## Problem

[双拼触点纠错](2026-10-27-shuangpin-touch-correction.md)的 PR3 让落点明显支持的纠错整句坐上首选，空格会直接上屏它。产品决定首选被纠正时三端都要显式标出来：这一行画强调色，读屏念「已纠正」，原来画在旁边的原始按键注释（corrected_from）照旧。引擎与宿主的契约是 View JSON 的候选对象多一个可选布尔键 `confident_correction`，只在按 PR3 规则坐上首选的纠错行出现且为真，其余行没有这个键，默认视图逐字节不变。

HarmonyOS 宿主以前完全不读纠错标记：`corrected` 和 corrected_from 只经 `annotation` 画成灰色注释。

## Decision

- **读键**：`KeyboardSession` 的 `Candidate` 声明可选的 `confident_correction`，候选栏的两处（`render`、`publishCurrentCandidates`）和展开面板的 `allCandidates` 都经 `CorrectionMarkPolicy.confident` 读，只认字面的 `true`，缺键、false、不是布尔值都不标；`CandidateEntry`、`ExpandedCandidate` 多一个 `confidentCorrection`。翻译页的行一律不标。
- **画法**：与 Android（`CandidateAppearance.Palette.correctionFor`）、iOS 同一条规则，三端一致：标记的行把候选字和它的注释、释义、编码提示都画成标记色，标记色在没高亮的行上是强调色，在高亮的行上是这一处的高亮字色。候选窗（2in1 的 `candidatePrimaryLine` 和它下面的释义行 `candidateCell`）、触屏胶囊（`candidateChip`）上，候选字经 `candidateWordColor`、其余字经 `candidateDetailTint`，令牌来自 `candidateSurfaceColors`；展开面板（`expandedGrid`，全拼九键三栏的中栏共用）格子只有字，经 `CorrectionMarkPolicy.cellTextColor`，令牌由 `CorrectionMarkPolicy.expandedSurface` 按有没有候选包画布在候选区配色和键盘配色（高亮格是 `toggleForeground`）之间选，与格子底色同源。高亮行不用强调色，是因为皮肤可能把高亮底填成强调色；内置配色里高亮字色本来就是强调色，所以高亮的首选字色不变，标记靠换了色的原始按键注释看出来，这一点三端相同。都是已有的配色令牌，没有新颜色常量。展开面板的 `ForEach` 键带上这个标记，同一个词标记变了也会重画。
- **读屏**：`candidateLabel` 和 `expandedLabel` 在整行朗读末尾接 `CorrectionMarkPolicy.accessibilitySuffix`，即「，已纠正」，排在「提示：原始按键」之后。
- **设置文案**：共享设置页（`packages/ui/src/settings/pages/input-page.tsx`，HarmonyOS 手机用它）的「全拼邻键纠错」「全拼乱序纠错」改成「拼音邻键纠错」「拼音乱序纠错」，双拼纠错同样受这两个字段控制，见 [共享设置页拆分纠错开关](2026-10-27-shared-autocorrect-switches.md)。

## Alternatives considered

- **强调色下划线（本分支最初的做法）**：下划线在高亮的首选上也看得见，但 Android 和 iOS 都改字色和注释色、不画下划线，三端标记会不一样，违反移动端界面一致的约定；产品决定写的也是「这一行画强调色」。审查后改成与另外两端相同的字色加注释色。
- **只改候选字色、不动注释**：内置配色里高亮的首选字本来就是强调色，只改字色在最常见的情形下看不出区别。注释能被「候选注释」偏好关掉，关掉后高亮首选上只剩读屏的「已纠正」，Android、iOS 也是这样，接受。
- **高亮行也一律用强调色**：皮肤的高亮底是强调色时字会看不见，所以高亮行用高亮字色，与 Android 的 `selected ? chipText : accent` 相同。

## Consequences

- 引擎不发这个键时（桌面、没有触点、门槛未达到时 `shuangpin_touch_lead_margin` 为无穷），候选栏、展开面板和读屏与以前完全相同。
- 只换颜色不改字重和字号，`CandidateWidthPolicy`、胶囊宽度和 2in1 候选窗宽度都不变。
- Android、iOS 的同一标记由各自宿主实现，规则与这里相同；以后改标记的画法要三端一起改。

## Verification

- `bash platforms/harmony/tests/run.sh` 退出 0：keyboard-logic.test.ts 新增一组钉住只认 `true`、读屏后缀与它在朗读里的位置、标记色在高亮与否、内置配色、固定词、展开格以及有无画布时的取值；run.sh 新增接线检查（会话三处经 `CorrectionMarkPolicy.confident` 读键、翻译页不标；视图去掉空白后逐字比较三处候选面的整段调用和两套令牌的每一项，候选窗与胶囊里不再留没经过 `candidateDetailTint` 的次要色；两处朗读接后缀）。把两套令牌里的高亮字色与强调色对调、展开格传 `false` 代替高亮、画布判断换成常量、胶囊注释退回 `skin.secondary`、注释着色忽略高亮，这六种改动都会让 run.sh 失败。
- `apps/desktop` 的 `vitest run tests/settings/language-card.test.tsx` 12 项通过，按新标题找到两个开关并分别保存。
- `stage-settings.sh`（含 `tsc --noEmit`）、`ohpm install`、`hvigorw assembleHap --no-daemon` 成功，ArkTS 编译没有报这次改动的文件；本机没有跑 `build-native.sh`，HAP 里没有引擎原生库，只是编译检查。`scripts/test-harmony-arkts-subset.py` 通过。
- 没有在模拟器或真机上看过：引擎侧发出 `confident_correction` 的改动在另一条分支上，本分支单独装机也不会出现这个键。
