# Agent Note: Android 英文字母长按变体

Status: implemented

## Problem

#9056 反馈英文键盘长按只有角标预览，没有 #8635 所列的大写、数字与字母上下标、拼音声调和财务数字。截图中的拼音是声调字符，不是切换输入语言。

## Decision

英文 26 键长按提供大写、原角标、可用的字母上下标和对应声调字符；首排按右上角数字提供数字上下标和零、壹至玖。短按已经提供小写，菜单不重复。`v` 提供 ü 与四声。保留 O 原有的切换到拼音键盘选项和选字后留在中文的规则。

选项按键盘实际宽度排成最多五列的多行菜单，限制在输入法窗口内，避免 API 28 子窗口裁剪或窄屏横向溢出。复用原长按弹窗的关闭与文本提交生命周期，不改中文上滑、下滑符号或其他语言路由。

弹窗优先靠近按住的键；位置在屏幕坐标中夹进键盘边界后，换算为锚点偏移交给 `showAsDropDown`。`showAtLocation` 的坐标相对 IME 父窗口，直接传入屏幕坐标会重复叠加窗口原点，导致菜单总被夹到键盘底部。

英文与中文弹窗共用原按键的拖选状态：长按后保持按住，移入选项高亮，松手只提交一次；进入后滑出或收到取消事件就关闭。没有移入选项时保留原有的后续点选。#9473 的设备复现确认英文弹窗虽然已经显示符号，但没有注册原手势，导致按住拖选无效；接入共用跟踪入口，避免两个语言分别维护生命周期。逗号长按保留已有的 ASCII 句号快捷输入。

逗号键复用 `KeyHintButton` 显示句号角标，让不使用读屏的用户也能发现长按入口。角标随当前标点模式显示 `.` 或 `。`，颜色继续由 `ImeStyler` 使用皮肤的提示色；不使用句号次级动作的布局清空角标，点按、长按、键位与计数保持原有路由。

## Related decisions

- [中文长按](../../implemented/feature/2026-10-20-android-letter-hold-variants.md)：部分重叠；中文三选项仍成立，本篇补齐英文。
- [O 键拼音入口](../../implemented/feature/2026-10-18-android-english-pinyin-entry.md)：部分重叠；本篇取代英文其他键仅预览、O 仅有切换项的限制，保留 O 切换与不自动返回规则。
- [叹号上下标](../../implemented/bug-fix/2026-10-16-android-exclamation-superscript-shortcuts.md)：不同入口，保持原有零的快捷手势。
- 其余命中的九键、中文标点、iOS 和 Caps Lock 笔记不涉及英文 26 键的长按变体，归为无关。

## Alternatives considered

- **单行横向滚动**：沿用当前排列且占用高度少，但用户必须滚动才能发现声调与财务数字；多行能在窄屏一次展示完整选项。
- **只保留松手后再次点选**：沿用已有英文菜单且改动最少，但同样的长按弹窗在中文可直接拖选，英文却不能，无法满足按住后滑向符号的输入手势；两者复用现有拖选状态。
- **把所有字母补成完整上下标表**：键位对称，但 Unicode 没有完整拉丁下标表，不能拿普通小写或相似字形假装上下标；仅提供实际存在且常见字体支持的字符。

## Verification

`EnglishLetterHoldPolicySmoke` 检查十个首排数字、字母上下标缺项和声调。`LetterHintPunctuationDeviceSmoke -e english-hold true` 逐个长按三排字母，并以真实触摸坐标选中，检查单次提交、屏幕边界、菜单靠近按键、英文状态、下滑角标和 O 的切换；它由普通 `smoke.sh` 执行。

`LetterHintPunctuationDeviceSmoke -e english-symbol-drag true` 保持一次真实触摸，覆盖 A→`@`、V→`?`、取消不提交、连续两次符号与逗号长按的 ASCII 句号。修复前 API 28 在 A→`@` 的选中状态断言失败，弹窗选项已经可见，排除菜单本身缺失。本次修复的原生 full APK 与重建夹具在 API 28、35 均通过此专项，原始 instrumentation 返回通过标记和 `-1`。完整英文与中文长按专项在 API 28 通过。API 35 后续完整验收遭遇测试期间包被替换与 System UI 无响应，相关轮次作废，改用本任务独立 AVD 继续核对；不能沿用下述早期菜单验收。此前中文滑动专项在切换为上滑直接符号后仍打开选项，失败记录保留；最终 APK 的重跑结果另列如下。

句号角标增量使用重编 Java 与资源的原生 full APK，在 API 28、35 通过英文拖选、取消、连续提交及逗号长按句号专项；两轮前后核对产品包与测试包哈希未变化。六项原生库与此前 APK 的打包字节相同，本轮没有重新构建 Rust 核心。API 28 在合成编辑器实看中文 `。` 和英文 `.` 角标，截图分别保留为 `period-hint-api28-chinese.png`、`period-hint-api28-english.png`，随后恢复中文。相同 APK 和最终夹具在 API 35 完整 `english-hold` 通过，覆盖逐键菜单、Unicode 18 下标、声调、O 切回中文和随后追加的拖选、取消及句号断言（`period-hint-api35-english-hold.log`）；前后包哈希一致。本轮完整 `scripts/verify-local.sh --quick` 退出 0（`period-hint-quick.log`），覆盖契约、笔记、Rust workspace、Android target、Android Java/JNI 与共享 Apple bridge；其余平台按改动范围跳过。

最终 APK 与相同夹具在 API 28 的完整 `letter-swipe` 重跑通过，覆盖大小写拖选、引号交替、取消与滑出不提交、后续点选、ASCII 标点、上下方向、禁用后保留字母、成对标点及候选态普通点按（`period-hint-api28-letter-swipe.log`）。前后产品和夹具包哈希一致，未修改这轮的测试或产品。早先同一配置阶段失败的根因仍未定位，本次通过不证明竞态已被消除。

本轮夹具只在未成功发送终止事件时补发取消，避免松手后重复 `ACTION_CANCEL` 被系统拒绝；这不改变选中、取消或精确正文断言。中文点选夹具先测 Q 长按与 W 上滑，最后测 A→`@`，因为邮箱候选出现时产品按设计禁用字母手势。完整 quick 与测试 APK 构建通过。

早期原生 full APK 在 API 28、35 专用模拟器的英文专项均通过：覆盖全部 26 个字母、E 菜单最后一行的 è、下滑 W 的数字 2、O 切回中文后输入并确认「你好」，以及选字后仍留在中文。两台模拟器的 `-e letter-hold true` 中文大小写、符号和上滑回归也通过。旧 APK 在同一英文专项的第一个 Q 长按选项等待失败，证明测试确实覆盖缺失的菜单。

API 35 的 `SuperscriptShortcutDeviceSmoke` 与 `NineKeyEnglishDeviceSmoke` 通过，覆盖共用弹窗的叹号上下标和九键数字选择。两台模拟器的 E 菜单截图确认三行完整可见且靠近按键。

`scripts/verify-local.sh --quick`、Android 宿主检查（251 项 JVM 冒烟及设备套件源码编译）、`pnpm run verify-notes` 通过。构建与模拟器验证不代表报告人 Android 11 真机已经验收。

## Consequences

英文状态可以直接选择这些字符，不必逐次翻符号页。字体对上下标的基线和尺寸不同，按原始 Unicode 字符提交，不使用富文本模拟上下标；缺少常见字符的键不显示假选项。多行弹窗会暂时覆盖部分键盘，点外部仍关闭。

[Unicode 18 下标补齐](../bug-fix/2026-10-23-android-unicode18-subscripts.md)补入 w/y/z 的真实码位，并仅在缺字形时缩小当前字母绘制，长按选项与候选共用局部标签处理。
