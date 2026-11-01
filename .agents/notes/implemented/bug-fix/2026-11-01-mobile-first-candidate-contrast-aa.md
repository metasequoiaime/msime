# Agent Note: 触屏候选条浅色首选字色达到 WCAG AA

Status: implemented

## Problem

[默认皮肤的选中行达到 WCAG AA](2026-11-01-default-selection-contrast-aa.md)（#10527、#10536）把 iOS、Android 和 HarmonyOS 触屏候选条的首选记为后续：跟随系统键盘的首选是画成季节强调色的字，浅色四季和 HarmonyOS 的回退配色都低于 4.5:1。那篇的数字是按「首选画在键盘底色上」量的。照代码重新量过，三端的首选其实画在字母键色的底片上（iOS `updateCandidateButton` 的 `number == 1 ? skin.keyBackground`，Android `ImeCandidates.chip(selectedBackground)` 取 `candidateSelectedBackground()` 即 kb.key，HarmonyOS 高亮的胶囊 `backgroundColor(this.skin.keyBackground)`），底片浅色是 `mix(bg 25%, #FFF)`、深色是 `mix(accent 10%, #3A3C3A)`；iOS 设置页的候选预览卡（`CandidatePreviewCard`）则把首选直接画在键盘底色上，不画底片。两种底都量了（键盘底色浅色 `mix(accent 12%, bg)`、深色 `mix(accent 10%, #161716)`，三端的 `AppThemePalette` 逐位一致）：

| 强调色画在 | 浅色键盘底色 | 浅色底片 | 深色键盘底色 | 深色底片 |
| --- | --- | --- | --- | --- |
| 春芽 `#4E9A3A` / `#9AD983` | 2.73 | 3.38 | 8.86 | 5.36 |
| 夏荫 `#17704A` / `#5FD39A` | 4.33 | 5.82 | 8.01 | 4.91 |
| 秋杉 `#B5562B` / `#F0975F` | 3.51 | 4.63 | 6.75 | 4.17 |
| 冬雪 `#3A6A8A` / `#93C7E6` | 4.24 | 5.61 | 8.14 | 4.98 |
| iOS 经典兜底 `#2C7A4B`（`#D1D4DB`、底片 `#FFFFFF`） | 3.54 | 5.26 | | |
| Android classic `#2C7A4B`（`#DFECDF`、底片 `#FDFEFC`） | 4.31 | 5.20 | | |
| HarmonyOS 原生回退 `#2C7A4B`（`#D9E2D6`、底片 `#FCFDFA`） | 3.96 | 5.15 | | |

上一篇的浅色数字对键盘底色是准的，对候选条上真正的底片只有春芽不到 4.5:1；深色那一列它没量底片，秋杉深色在底片上只有 4.17:1。

## Decision

浅色四季和三端回退配色的首选另用一个字色，强调色本身不变。维护者批准的范围是浅色变体和 HarmonyOS 回退，深色不动。

- **字色怎么来**：保持色相，在 OKLCH 里只降明度（色度不变），直到对键盘底色和字母键底片都不低于约 4.6:1。键盘底色更深，是起作用的那一边；两种底都要满足，是因为候选条画在底片上、设置页预览画在键盘底色上，同一个字色在两处都要可读。

| | 原强调色 | 首选字色 | 键盘底色上 | 底片上 |
| --- | --- | --- | --- | --- |
| 春芽 | `#4E9A3A` | `#267308` | 4.64 | 5.75 |
| 夏荫 | `#17704A` | `#0F6B46` | 4.66 | 6.26 |
| 秋杉 | `#B5562B` | `#9F4214` | 4.65 | 6.15 |
| 冬雪 | `#3A6A8A` | `#346484` | 4.64 | 6.13 |
| iOS 经典兜底 | `#2C7A4B` | `#15683B` | 4.60 | 6.83 |
| Android classic | `#2C7A4B` | `#267547` | 4.63 | 5.58 |
| HarmonyOS 原生回退 | `#2C7A4B` | `#1F6F41` | 4.64 | 6.04 |

- **四季的值放在 Rust 里**：`AppThemeSeed::candidate_accent`（浅色是上表的值，深色 `None`），`resolve_app_theme` 和 `catalog()` 多出 `candidate_accent` 字段，深色等于 `accent`。三端都从 `msime_client_resolve_app_theme` 读它，季节种子不在宿主里留副本，与其他种子色一样；`platforms/android/tests/settings/app-theme-catalog.json` 随之重写，`host-contracts.ts` 的 `ResolvedAppTheme` 加上同名字段（共享设置页不画它）。回退配色本来就是各宿主自己的 token，三端各写一个常量：iOS `NativeKeyboardTokens.candidateAccent`，Android `KeyboardSkin.CLASSIC` 的浅色 `Mode`，HarmonyOS `GlobalTheme.candidateAccent`。Android 解析失败时用的 `Seed.AUTUMN` 也带秋杉的首选字色。
- **只在首选上用**：iOS `ThemeKeyboardPalette.candidateAccent` 经 `KeyboardTheme.candidateAccent` 给候选条的第一个词块（字、坐了首选的纠错注释）和 `CandidatePreviewCard` 的第一个候选；其余位置的固定词仍是强调色。Android 是 `KeyboardSkin.candidateSelectedForeground()`，`CandidateAppearance.chipText` 本来就是它，所以候选条的选中 chip、展开网格的选中格和 chip 上的纠错标记一起换。HarmonyOS 是 `KeyboardSkin.candidateAccent`，触屏条 `candidateColors()` 的 `selectedText` 取它；2in1 候选窗走自己的 `NATIVE_CANDIDATE_*`，不受影响。
- **谁不变**：强调色本身（回车键、开启态、工具栏激活、拼写、品牌键）、深色变体、具名主题（水杉、纸白、夜青等用自己的强调色）、用户的键盘设计、键盘底色和底片。

### 对比度测试

- Rust `skin::app_theme::the_first_candidate_meets_aa_on_light_keyboards`：按宿主的 `color-mix` 取整算出四季浅色的键盘底色和底片，`candidate_accent` 对两者都不低于 4.5:1，强调色本身对键盘底色低于 4.5:1（不需要单独字色时测试会提醒去掉），深色 `candidate_accent` 等于 `accent`。`host-api` 的契约测试要求目录和解析结果都带这个字段。
- iOS `KeyboardSkinTests.testFirstCandidateMeetsAAOnLightSeasonKeyboards`：四个固定季节下跟随系统皮肤的 `candidateAccent` 是上表的值，对 `background` 和 `keyBackground` 都不低于 4.5:1，回车键仍是强调色，深色等于强调色；经典兜底 token 同样检查；纸白的首选仍是自己的强调色。候选条第一个词块的字色测试改成 `candidateAccent`。
- Android `AppThemePaletteSmoke.firstCandidateContrast`：四季和代码里的秋杉回退，浅色 `candidateAccent` 对 `keyboardBackground`、`keyboardKey` 不低于 4.5:1；`KeyboardSkinSmoke` 对 classic 回退按真实颜色算对比度（`candidateSelectedForeground()` 对 `candidateSelectedBackground()` 和 `background()` 都不低于 4.5:1），并和 `CandidateAppearanceSmoke`、`CandidateCorrectionMarkPolicySmoke` 一起钉住 classic、秋杉和具名皮肤的 chip 字色。
- HarmonyOS `keyboard-logic.test.ts`「浅色跟随系统键盘的首选字色……」：四季和无种子回退，`candidateAccent` 对 `keyBackground`、`background` 不低于 4.5:1，强调色和回车键不变，深色等于强调色，具名主题用自己的强调色，首选字色计入皮肤身份。

## Alternatives considered

- **只按底片量，只改春芽浅色** — 首选真正的底是底片，按维护者「对实际画的底到 4.5:1」的字面要求，只有春芽浅色（3.38）不达标，秋杉浅色 4.63 已经过线，改动最小，其他三季的首选颜色完全不变。不采用：iOS 设置页的候选预览卡把首选直接画在键盘底色上，那里夏荫、秋杉、冬雪都不到 4.5:1；同一个字色在键盘里和预览里画法不同，只够底片就会让预览不达标。维护者批准的就是按键盘底色量出的这四季加 HarmonyOS 回退。
- **直接加深种子里的强调色** — 一个值改完，所有用强调色的地方一起变深。不采用：季节强调色同时是宿主设置页、开屏、统计图和回车键的主色，是设计稿的种子；加深它会改掉整个应用的外观，超出这次「只改首选字色」的授权。
- **各宿主运行时从强调色算** — 在 Swift、Java、ArkTS 里各移植一份 OKLCH 和对比度搜索，以后改种子不用再填值。不采用：三份实现要逐位一致才能「三端同值」，比一张七个值的表难维护得多；值放在 Rust 里由测试兜底，改种子时测试会失败并指出要重算。
- **深色也一起修** — 秋杉深色的首选在底片上只有 4.17:1，iOS 经典兜底深色 `#5FBF84` 在 `#6B6B6E` 底片上只有 2.35:1。不采用：维护者这次只批准了浅色，深色需要调亮字色或调暗底片，是另一个取舍，记在 Consequences 里。

## Consequences

- **收益**：浅色跟随系统键盘的首选在三端、四季和回退配色下对候选条底片和键盘底色都不低于 4.5:1；四季的值只有 Rust 一份，三端读同一个契约字段；每个平台都有对比度断言，以后改种子或底色时退化会在单测里失败。
- **代价**：浅色首选比强调色暗，春芽 `#267308` 尤其明显，和回车键不再是同一个绿；Android 展开网格的选中格跟着换色，iOS 和 HarmonyOS 的展开面板仍用强调色（iOS 选中格画在强调色淡底上，HarmonyOS 画开启态的 `toggleForeground`），三端展开面板的首选颜色不一致。应用主题的 C ABI 多了一个字段。iOS `AppThemePalette.Resolved` 和 HarmonyOS `parseResolved` 每次都从共享层现算，缺这个字段时整次解析失败，退回各自的回退配色。Android 的 `Mode.fromResolved` 还要读 `AppThemeController` 存在 SharedPreferences 里的上一次解析结果，升级前存的没有这个字段；那里缺 `candidate_accent` 时退回 accent，不判为读不懂，否则春芽、夏荫、冬雪的用户在 `follow()` 重写缓存之前，宿主里的键盘预览会画成秋杉。
- **未覆盖**：深色秋杉首选在底片上 4.17:1、iOS 经典兜底深色 2.35:1；iOS 候选条其余位置的固定词仍是强调色画在键盘底色上；iOS 日语转换时当前所在的词块画在 `skin.accent` 22% 盖在键盘底色上的淡底上，转换停在首个词块时它的字用 `candidateAccent`，浅色四季只有约 3.5 到 3.8:1（春 3.81、夏 3.52、秋 3.63、冬 3.54，改前用强调色是 2.24 到 3.27），是改善但没到 AA；具名主题（如纸白 `#2C7A4B` 在 `#E6E1D5` 上约 4.0:1）不在这次范围里，iOS `testSkinTextContrastInBothAppearances` 对所有主题的首选仍只要求 3:1。验证：Rust 和 host-api 的测试经 `rbuild` 在 Studio 上跑过，Android `check-host.sh`（含全部 JVM 冒烟）、HarmonyOS `tests/run.sh`、桌面 `typecheck` 和受影响的 vitest 在本机跑过；iOS 的 Swift 测试只在 PR 的 iOS CI 上跑，没有在本机模拟器上跑；HarmonyOS 没有本机打 HAP，`.ets` 里的两处改动（触屏条 `selectedText`、`sameSeed`）没有经过 ArkTS 编译器，只过了 `scripts/test-harmony-arkts-subset.py`；三端都没有在设备或模拟器上看过画面。

相关：跟随系统键盘按应用主题取色的来历见 [触屏键盘新装默认跟随系统皮肤](../feature/2026-10-08-touch-keyboard-default-follows-app-theme.md)。
