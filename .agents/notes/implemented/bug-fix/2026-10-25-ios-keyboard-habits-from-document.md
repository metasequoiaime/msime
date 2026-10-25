# Agent Note: iOS 键盘出现时按共享文档读输入习惯，不按 App Group 镜像

Status: implemented

## Problem

`EighteenKeyKeyboardTests.testShuangpinLayoutReloadSwitchesBetweenFourteenAndEighteenKeys` 单独跑（或在新模拟器上只跑这一类）时失败：双拼 14 键时键盘高 295pt，设置页换成 18 键、热加载之后变成 291pt。整套 `MSIMEClientTests` 一起跑时它是绿的，所以 CI 一直没拦下来。

差的 4pt 正是带释义的候选行（42pt）和不带释义的候选行（38pt）之差，和 14 键还是 18 键无关：两者都是读音单独一行（`separateReadingRow` 由 `usesLetterGrid` 决定），顶栏都是 14 + 候选行。真正变的是释义开关：

- 共享 `PreferencesStore` 文档里 `candidate_english_gloss` 默认关（client-core `Preferences::default()`，见 `candidate_english_gloss_is_opt_in_and_round_trips`），文档总会写出这一项。Android 和 HarmonyOS 都按文档默认关。
- iOS 的 App Group 镜像 `CandidateGlossPreference.enabled` 没写过时读作开（`?? true`）。
- 键盘在 `viewDidLoad` 先把会话创建时读到的文档里的方案、双拼布局和简繁抄进镜像（#5172），但输入习惯（学习、调频、释义、候选翻译与语言）没有抄，于是第一次布局按镜像的「开」多留一行释义。
- 之后 `viewWillAppear` 的后台重载只在文档比会话手上的新时才调用 `synchronizeSharedTouchPreferences`，那时才把文档的「关」抄进镜像，顶栏缩回去。测试里换 18 键写了文档，重载因此生效，高度从 295 掉到 291。整套跑时前面的用例早已触发过一次生效的重载，镜像已经是「关」，两次测量都是 291，用例碰巧通过。

用户那边是同一个缺陷：镜像没写过（没走过写入输入习惯的引导或设置页）时，键盘一出现就按「开」显示英文释义、多留 4pt，设置页却按文档显示「关」；等任何一次设置改动触发重载，释义又消失、键盘变矮。文档没有更新时重载不抄，这个不一致会一直留着。

这条用例是 #9643（e4614e6112）加进来的，从加进来起单独跑就是红的；#9707、#9715、#9716 的手写高度改动与它无关（手写加高只在 `handwriting` 可见时计入 `keyRows`）。缺陷本身更早：07f1b6c607 让键盘在重载时把文档的释义开关抄进镜像，#5172 让方案和简繁在出现时以文档为准，输入习惯两头都漏了。

## Decision

`KeyboardViewController.viewDidLoad` 在算顶栏之前，紧跟方案和双拼布局之后，把会话创建时读到的文档里的输入习惯整组抄进镜像：`InputHabitPreference.mirror(InputHabitPreference.settings(in: session.sharedPreferences))`。文档缺的字段沿用镜像原值，与重载时的读法一致。不额外读盘，键盘第一次布局、释义请求和学习开关从一开始就与设置页看到的文档一致。

14 键和 18 键双拼的高度规则不变：竖屏手机都是上边距 2 + 读音行 14 + 候选行（不带释义 38、带释义 42）+ 间隔 4 + 四排 52pt 键和三道行距 + 下边距 4，默认行距 7 时不带释义 291pt、带释义 295pt，换布局时不变。用例的断言（换布局高度不变）是对的，错的是产品。

测试里给释义开关赋值、随后新建键盘的用例改用 `setCandidateGloss(_:)`（`InputSchemeTestSupport.swift`）：文档和镜像一起写，结束时两边放回原样，与 `enableAllInputSchemes` 处理方案的做法相同。只改镜像的赋值会在新键盘出现时被文档盖掉。已建好的键盘中途只改镜像来模拟「刚关掉」（如长按菜单不再提供释义）仍然成立，那些行不改。

## Alternatives considered

- **只改用例：先把释义开关钉住再量高度。** 最省事，用例马上变绿；但它把产品里第一次出现与重载后高度、释义不一致的缺陷藏起来了，用户照样会看到释义出现又消失。
- **把镜像默认改成关（`?? false`）。** 新装时也能对上文档的默认值，但用户在别处（桌面同步、设置页写文档后写镜像失败）改过开关时，镜像和文档照样可能不一致，第一次布局仍会按旧值；而且只修了释义，学习和翻译语言等同组字段仍有同样的窗口。以文档为准才是 `InputHabitPreference` 头注释写明的契约。
- **把文档默认改成开。** 会改 Android、HarmonyOS 和桌面的新装行为，与 client-core 里「释义是 opt-in」的既定决定相反，不在这次的范围。

## Consequences

- iOS 键盘出现时的释义开关、学习、调频和候选翻译语言都按文档；在镜像从没写过的设备上，新装键盘不再默认显示英文释义，与设置页、Android 和 HarmonyOS 一致。读音单独一行的 14 键、18 键竖屏因此从 295pt 变成 291pt；26 键的顶栏本来就被最低高度 42pt 垫着，高度不变。[iOS 读音并入候选行](../feature/2026-10-13-ios-inline-reading-top-row.md)里「默认开释义」说的是当时镜像的默认值，文档默认是关。
- 只给 `CandidateGlossPreference.enabled` 赋值再新建键盘的测试不再起作用，必须走 `setCandidateGloss(_:)`；这次已改 `CandidateTranslationTests`、`KoreanDubeolsikTests`、`KeystrokeLatencyTests`、`NineKeyKeyboardTests` 和两份 `JapaneseNineKeyTests`。
- `EighteenKeyKeyboardTests` 不再依赖前面的用例先触发一次重载，单独跑和整套跑结果一样。
