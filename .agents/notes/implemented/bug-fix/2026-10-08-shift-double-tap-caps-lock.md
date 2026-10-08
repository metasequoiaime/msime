# Agent Note: 双击 Shift 锁定大写只看两次点按的间隔

Status: implemented

## Problem

Android、iOS 和 HarmonyOS 的触屏键盘都有「350 ms 内双击 Shift 进入大写锁定」，但用户反馈「大写似乎不能锁定」。三端的状态机是同一套逻辑移植的，双击判定写成了「第一下之后处于单次大写，且记得第一下的时间」，于是有两种常见情况锁不上：

- 句首或空输入框里键盘已被自动大写置成单次大写，第一下把它关掉、第二下又打开，永远到不了锁定。英文状态下这是最常见的开始打字的位置。
- 自动大写每次重新计算都会清掉点按时间（Android、HarmonyOS 的 `EnglishLetterCaseState.applyAutomatic`，iOS 的 `updateAutomaticCapitalization`）。Android 的 `onUpdateSelection` 每次都会重新计算，WebView 一类编辑器在两次点按之间回报一次光标位置，双击就失效。

Android 和 iOS 的自动大写重新计算还会把用户手动按下、尚未被字母用掉的单次大写冲掉；HarmonyOS 的调用方早已在这种情况下跳过重新计算。

## Decision

- 双击只看两次手动点按的间隔是否不超过 350 ms，不看第一下之后是什么状态。处于大写锁定时点一下回到小写并清掉计时，紧接着的一下只是单次大写，不会又锁上。单击的行为不变：小写与单次大写之间切换，单次大写被下一个字母用掉。
- 自动大写的重新计算不再清掉点按计时。点按计时仍在输入字母（`consumeLetter`）、切换中英、换输入框（`reset`）时清掉，所以「Shift、字母、Shift」不算双击。
- 手动按下的单次大写留到下一个字母用掉为止：Android 的 `updateAutomaticCapitalization` 在 `EnglishLetterCaseState.isPressedShift()` 为真时不调用 `applyAutomatic`，iOS 的 `updateAutomaticCapitalization` 在 `letterCaseState == .shifted && !isAutomaticShift` 时直接返回，与 HarmonyOS 的 `applyAutomaticCase` 一致。

## Alternatives considered

- 长按 Shift 锁定大写。Gboard 这样做，也不依赖时间窗口，最不容易误触；但反馈里明确说双击比长按快，而且三端本来就承诺双击锁定，问题出在判定条件，不在交互方式。
- 只去掉 `applyAutomatic` 里清计时的那一行。改动最小，能修好两次点按之间编辑器回报光标位置的情况；但句首自动大写时第一下仍把状态变成小写，双击判定仍要求「当前是单次大写」，最常见的那种情况照样锁不上。
- 让句首自动大写时的第一下直接进入锁定。能锁上，但单击就锁定与「单击是单次大写」的期望冲突，用户只想关掉自动大写时会被锁住。

## Consequences

- 句首、自动大写开启时双击也能锁定，与 iOS 系统键盘和 Gboard 的习惯一致；编辑器在两次点按之间回报光标位置不再影响双击。
- 从自动大写开始的单击仍是关掉大写，没有变化。350 ms 内快速点两下想「关掉再打开」单次大写的操作现在会锁定；这与双击锁定的承诺一致。
- Android 的 `EnglishLetterCaseStateSmoke` 和 HarmonyOS 的 `keyboard-logic.test.ts` 覆盖句首双击、两次点按之间的自动大写重新计算、锁定后的单击与随后的单击。iOS 的状态写在 `KeyboardViewController` 里，没有单元测试，靠 CI 的 iOS Simulator 构建确认能编译；三端都没有在真机上点按验证。
- `platforms/android/java/app/msime/android/keyboard/ShiftTapPolicy.java` 是另一份同样规则的状态机，目前没有调用方，这次没有改它。
