# Agent Note: HarmonyOS 标出坐上首选的双拼纠错

Status: implemented

## Problem

[双拼触点纠错](2026-10-27-shuangpin-touch-correction.md)的 PR3 让落点明显支持的纠错整句坐上首选，空格会直接上屏它。产品决定首选被纠正时三端都要显式标出来：这一行画强调色，读屏念「已纠正」，原来画在旁边的原始按键注释（corrected_from）照旧。引擎与宿主的契约是 View JSON 的候选对象多一个可选布尔键 `confident_correction`，只在按 PR3 规则坐上首选的纠错行出现且为真，其余行没有这个键，默认视图逐字节不变。

HarmonyOS 宿主以前完全不读纠错标记：`corrected` 和 corrected_from 只经 `annotation` 画成灰色注释。

## Decision

- **读键**：`KeyboardSession` 的 `Candidate` 声明可选的 `confident_correction`，候选栏的两处（`render`、`publishCurrentCandidates`）和展开面板的 `allCandidates` 都经 `CorrectionMarkPolicy.confident` 读，只认字面的 `true`，缺键、false、不是布尔值都不标；`CandidateEntry`、`ExpandedCandidate` 多一个 `confidentCorrection`。翻译页的行一律不标。
- **画法**：候选窗（2in1 的 `candidatePrimaryLine`）、触屏胶囊（`candidateChip`）、展开面板（`expandedGrid`，全拼九键三栏的中栏共用）三处的候选文字上画下划线（`correctionDecoration`）。颜色由 `CorrectionMarkPolicy.underlineColor` 定：没高亮的行用强调色，高亮的行用这一处的高亮字色。内置配色里高亮字色就是强调色；皮肤把高亮底填成强调色时，强调色的线会消失在底色里，高亮字色在那种底上一定看得清。都是已有的配色令牌，没有新颜色常量。展开面板的 `ForEach` 键带上这个标记，同一个词标记变了也会重画。
- **读屏**：`candidateLabel` 和 `expandedLabel` 在整行朗读末尾接 `CorrectionMarkPolicy.accessibilitySuffix`，即「，已纠正」，排在「提示：原始按键」之后。
- **设置文案**：共享设置页（`packages/ui/src/settings/pages/input-page.tsx`，HarmonyOS 手机用它）的「全拼邻键纠错」「全拼乱序纠错」改成「拼音邻键纠错」「拼音乱序纠错」，双拼纠错同样受这两个字段控制，见 [共享设置页拆分纠错开关](2026-10-27-shared-autocorrect-switches.md)。

## Alternatives considered

- **把候选字改成强调色**：字面上最贴近「画强调色」，但触屏候选栏高亮那一行的字本来就是强调色（`candidateColors` 在非皮肤触屏下 `selectedText` 取 `skin.accent`），首选通常就是高亮的那一行，改了看不出区别；2in1 内置配色也一样。不采用。
- **把原始按键注释改成强调色**：注释能被「候选注释」偏好关掉，在胶囊上还可能挪到释义行，标记会跟着消失。不采用。
- **一律用强调色画下划线**：皮肤的高亮底是强调色时线看不见，所以高亮行改用高亮字色。

## Consequences

- 引擎不发这个键时（桌面、没有触点、门槛未达到时 `shuangpin_touch_lead_margin` 为无穷），候选栏、展开面板和读屏与以前完全相同。
- 下划线不占宽度，`CandidateWidthPolicy`、胶囊宽度和 2in1 候选窗宽度都不变。
- Android、iOS 的同一标记由各自宿主实现；如果它们高亮的首选也已经是强调色字，同样不能只改字色。

## Verification

- `bash platforms/harmony/tests/run.sh` 退出 0：keyboard-logic.test.ts 新增一组钉住只认 `true`、读屏后缀与它在朗读里的位置、下划线颜色的四种情形；run.sh 新增接线检查（会话三处经 `CorrectionMarkPolicy.confident` 读键、翻译页不标、视图三处画下划线、两处朗读接后缀）。
- `apps/desktop` 的 `vitest run tests/settings/language-card.test.tsx` 12 项通过，按新标题找到两个开关并分别保存。
- `stage-settings.sh`（含 `tsc --noEmit`）、`ohpm install`、`hvigorw assembleHap --no-daemon` 成功，ArkTS 编译没有报这次改动的文件；本机没有跑 `build-native.sh`，HAP 里没有引擎原生库，只是编译检查。`scripts/test-harmony-arkts-subset.py` 通过。
- 没有在模拟器或真机上看过：引擎侧发出 `confident_correction` 的改动在另一条分支上，本分支单独装机也不会出现这个键。
