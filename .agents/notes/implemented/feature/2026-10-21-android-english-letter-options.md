# Agent Note: Android 英文字母长按变体

Status: implemented

## Problem

#9056 反馈英文键盘长按只有角标预览，没有 #8635 所列的大写、数字与字母上下标、拼音声调和财务数字。截图中的拼音是声调字符，不是切换输入语言。

## Decision

英文 26 键长按提供大写、原角标、可用的字母上下标和对应声调字符；首排按右上角数字提供数字上下标和零、壹至玖。短按已经提供小写，菜单不重复。`v` 提供 ü 与四声。保留 O 原有的切换到拼音键盘选项和选字后留在中文的规则。

选项按键盘实际宽度排成最多五列的多行菜单，限制在输入法窗口内，避免 API 28 子窗口裁剪或窄屏横向溢出。复用原长按弹窗的关闭与文本提交生命周期，不改中文上滑、下滑符号或其他语言路由。

弹窗优先靠近按住的键；位置在屏幕坐标中夹进键盘边界后，换算为锚点偏移交给 `showAsDropDown`。`showAtLocation` 的坐标相对 IME 父窗口，直接传入屏幕坐标会重复叠加窗口原点，导致菜单总被夹到键盘底部。

## Related decisions

- [中文长按](../../implemented/feature/2026-10-20-android-letter-hold-variants.md)：部分重叠；中文三选项仍成立，本篇补齐英文。
- [O 键拼音入口](../../implemented/feature/2026-10-18-android-english-pinyin-entry.md)：部分重叠；本篇取代英文其他键仅预览、O 仅有切换项的限制，保留 O 切换与不自动返回规则。
- [叹号上下标](../../implemented/bug-fix/2026-10-16-android-exclamation-superscript-shortcuts.md)：不同入口，保持原有零的快捷手势。
- 其余命中的九键、中文标点、iOS 和 Caps Lock 笔记不涉及英文 26 键的长按变体，归为无关。

## Alternatives considered

- **单行横向滚动**：沿用当前排列且占用高度少，但用户必须滚动才能发现声调与财务数字；多行能在窄屏一次展示完整选项。
- **把所有字母补成完整上下标表**：键位对称，但 Unicode 没有完整拉丁下标表，不能拿普通小写或相似字形假装上下标；仅提供实际存在且常见字体支持的字符。

## Verification

`EnglishLetterHoldPolicySmoke` 检查十个首排数字、字母上下标缺项和声调。`LetterHintPunctuationDeviceSmoke -e english-hold true` 逐个长按三排字母，并以真实触摸坐标选中，检查单次提交、屏幕边界、菜单靠近按键、英文状态、下滑角标和 O 的切换；它由普通 `smoke.sh` 执行。

原生 full APK 在 API 28、35 专用模拟器的英文专项均通过：覆盖全部 26 个字母、E 菜单最后一行的 è、下滑 W 的数字 2、O 切回中文后输入并确认「你好」，以及选字后仍留在中文。两台模拟器的 `-e letter-hold true` 中文大小写、符号和上滑回归也通过。旧 APK 在同一英文专项的第一个 Q 长按选项等待失败，证明测试确实覆盖缺失的菜单。

API 35 的 `SuperscriptShortcutDeviceSmoke` 与 `NineKeyEnglishDeviceSmoke` 通过，覆盖共用弹窗的叹号上下标和九键数字选择。两台模拟器的 E 菜单截图确认三行完整可见且靠近按键。

`scripts/verify-local.sh --quick`、Android 宿主检查（251 项 JVM 冒烟及设备套件源码编译）、`pnpm run verify-notes` 通过。构建与模拟器验证不代表报告人 Android 11 真机已经验收。

## Consequences

英文状态可以直接选择这些字符，不必逐次翻符号页。字体对上下标的基线和尺寸不同，按原始 Unicode 字符提交，不使用富文本模拟上下标；缺少常见字符的键不显示假选项。多行弹窗会暂时覆盖部分键盘，点外部仍关闭。

[Unicode 18 下标补齐](../bug-fix/2026-10-23-android-unicode18-subscripts.md)补入 w/y/z 的真实码位，并仅在缺字形时缩小当前字母绘制，长按选项与候选共用局部标签处理。
