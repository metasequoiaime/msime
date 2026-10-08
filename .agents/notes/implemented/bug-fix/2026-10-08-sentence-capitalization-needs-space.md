# Agent Note: 句末标点后隔着空白才自动大写

Status: implemented

## Problem

issue #5806 反馈 Android 0.2.2 英文输入时「点了 `.` 之后大概率切换成大写」，「从中文切到英文、或在英文下删字之后，也有一定概率切换大小写」。

Android、iOS 和 HarmonyOS 的 `EnglishCapitalizationPolicy` 是同一套规则移植的。句子模式（编辑器带 `TYPE_TEXT_FLAG_CAP_SENTENCES`，绝大多数输入框都是）从光标往前跳过空白和右引号、右括号，碰到 `.`、`!`、`?`、`。`、`！`、`？` 就打开自动大写，不要求标点和光标之间有空白。于是：

- 刚点下 `.`，`onUpdateSelection` 重新计算，键盘立刻变成大写。`3.14`、`e.g`、`example.com` 这类写法每次都要手动把大写关掉。
- 删字删到 `abc.` 后面，键盘又变成大写。
- 中文下打完 `你好。` 再切到英文，`toggleInputLanguage` 重新计算，看到 `。` 就变成大写。

这三条是同一个原因，「有一定概率」取决于光标前恰好是不是句末标点。HarmonyOS 的 `keyboard-logic.test.ts` 曾把 `"Hi."` 和 `"你好。"` 写成应当大写的用例。

## Decision

- 句子模式下，句末标点只有在它（连同其后的右引号、右括号）和光标之间至少隔着一个空白字符时，才算开始新句。换行、空输入框、整段只有空白仍然直接大写。
- 半角和全角句末标点用同一条规则：`你好。 ` 大写，`你好。` 不大写。
- 三端一起改，规则逐字一致：Android `platforms/android/java/app/msime/android/policy/EnglishCapitalizationPolicy.java`、iOS `platforms/ios/KeyboardExtension/Sources/input/EnglishCapitalizationPolicy.swift`、HarmonyOS `platforms/harmony/entry/src/main/ets/keyboard/input/EnglishCapitalizationPolicy.ts`。iOS 的 Swift `Character` 会把 `\r\n` 合成一个字符，之前被当成普通空白跳过；现在和另外两端一样算换行。
- 单词模式（`TYPE_TEXT_FLAG_CAP_WORDS`）不变：人名、标题类输入框里 `J.R.` 之后大写是想要的结果，issue 也没有涉及它。

## Alternatives considered

- 只对半角 `.`、`!`、`?` 要求空白，全角 `。！？` 仍然紧贴就大写。最强的理由是中文不写空格，中英混排时 `你好。Hello` 的 `H` 确实是新句开头。不用它，是因为 issue 里「从中文切到英文会变大写」正是这种情况：用户在 `。` 后切英文多半是要接一个英文词，而不是另起一个英文句子；Gboard（`TextUtils.getCapsMode`）也不把全角标点当句末。需要大写时按一下 Shift 即可，误开的大写却要每次手动关。
- 照搬 `TextUtils.getCapsMode` 的缩写判断：`e.g. ` 这种单词里已经有 `.` 的情况不大写。它确实更贴近 Gboard，但这是另一条规则，会把这次修复扩到 issue 之外，而且三端都要再移植一遍；这次只修「没空格也大写」。
- 只改 Android。issue 只报了 Android，但 iOS 和 HarmonyOS 是同一份规则、同样的问题，只改一端会让三端的大写行为开始分叉。

## Consequences

- 打完 `.` 键盘保持小写，打空格后才变成大写，和 Gboard、iOS 系统键盘一致；`3.14`、网址、缩写不再被自动大写打断。删字删到 `abc.` 后面、在 `你好。` 后切英文都不会再变大写。
- 空输入框、换行之后、句末标点加空格之后的自动大写不变。删光整段文字后键盘变成大写属于这一条，是有意保留的行为。
- 中英混排时 `你好。` 之后紧接的英文句子不再自动大写，要手动按 Shift。
- Android 的 `EnglishCapitalizationPolicySmoke`（`check-host.sh` 自动发现）、HarmonyOS 的 `keyboard-logic.test.ts`（`tests/run.sh`）覆盖紧贴标点、右引号后无空白、小数点、全角句号有无空白、整段空白。iOS 的 `platforms/ios/tests/input/EnglishCapitalizationPolicyTests.swift` 被 `project.yml` 排除在测试 target 之外，没有门禁运行；这次用 `swiftc -parse-as-library` 把它和策略文件编在一起手动跑过。三端都没有在真机上验证。
- Shift 状态机本身（手动单次大写不被自动大写冲掉、双击锁定）见 [双击 Shift 锁定大写只看两次点按的间隔](2026-10-08-shift-double-tap-caps-lock.md)，这次没有改动。
