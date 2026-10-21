# Agent Note: Android 九键双拼收简键面提示

Status: implemented

## Problem

九键双拼把每格的声母和韵母表同时铺在四行网格里，数字与小字挤在同一行。用户反馈满屏字母、难看。[九键双拼](../../implemented/feature/2026-10-10-nine-key-shuangpin.md)定义的键位和输入行为仍然成立，这里只收简触摸键面的提示。

历史检索中，九键双拼笔记与此部分重叠，双方互链；[九键字号适配](../../implemented/bug-fix/2026-10-13-android-nine-key-label-fit.md)主要约束拼音九键，其双拼键面引用同步到本篇。

## Decision

每键主标签采用最大 22sp 的数字，底部独立绘制最大 9sp 的完整声母提示，零声母编码从这个键开始时前面加「零」；只有韵母的键显示「韵母」。`NineKeyDigitButton` 复用 `KeyHintButton` 的底部提示，不把提示拼成 `TextView` 的第二行。底部按字体实际度量预留高度，主标签由 Android 自动字号适配剩余空间，提示超宽时按实际宽度缩小，保留全部声母。完整声韵母表仍在设置编辑页与无障碍说明中提供。输入、分词、0 组码、方案切换继续使用引擎映射。

同一个提示控件也承载 [26 键双拼提示](2026-10-09-touch-shuangpin-key-hints.md)：开启底部提示时同时为右上角符号预留高度，避免字母被下边距向上推到角标上；关闭时恢复原字号和上下内边距。缩短键高和系统大字体下以文字边界互不相交为先，主标签可能小于默认字号。

## Alternatives considered

- **把声母提示限制为三项并加省略号**：可约束每格的小字宽度，但默认 9 键的 `z` 会被藏成省略号，用户无法从键面判断完整声母映射，因此不采用。
- **保留完整声韵母表，仅缩小文字**：初学者可在键盘上查全部映射。但四行布局空间不足，更小的文字会继续降低可读性。
- **键帽只显示数字**：视觉最简洁，但会失去寻找声母键的提示。保留一行简短提示，兼顾辨认与留白。
- **继续用带相对字号的两行 `TextView`**：不需要独立绘制提示，但四行压进三行高度时无法分别约束主标签与提示，窄键还可能再折行；复用已有底部提示绘制，按真实字体度量留出各自的空间。

## Consequences

数字主标签更易辨认，四行网格不再铺满声韵母表。韵母映射不再直接全铺在键面上，学习时需要查看设置中的表；输入行为和完整无障碍说明保持原有契约。

## Verification

原收简改动的验证：`NineKeyShuangpinKeypadSmoke` 检查简短声母提示、韵母类别、四项声母不被省略，9 键完整显示 `w x y z`，以及完整映射说明。`build-apk.sh`、`scripts/verify-local.sh --quick` 和 `pnpm run verify-notes` 通过。API 35 专用模拟器的截图确认数字独占主行、9 键完整显示 `w x y z`。`NineKeyShuangpinDeviceSmoke` 验证两位组码、0 组码、分词和方案切换通过。Android 12 真机覆盖安装返回 `Success`，未在真机执行输入验收。

2026-10-21 的重叠修复在 API 28 专用模拟器验证：新增 `ShuangpinKeyFaceChecks` 用安装产品的真实按钮、Android 排版器和合成彩色标签扫描像素边界，覆盖系统字体 1 倍和 2 倍、26 键 28dp 窄键和 40/52/84dp 键高，以及九键 29/40/52/84dp 键高；同时检查主标签单行、提示关闭恢复字号。旧包在 26 键 28×40dp、普通字体下报主标签与角标重叠，修复包全部通过。九键和开启提示的 26 键实际截图确认主标签与提示分离；`NineKeyShuangpinDeviceSmoke`、`MicrosoftShuangpinDeviceSmoke` 和普通全拼的 `NineKeyPanelDeviceSmoke` 实际输入验收通过。微软双拼回归明确开启 `touch_shuangpin_key_hints` 并等待提示出现，避免模拟器原配置关闭提示时漏验。

本轮 `check-host.sh` 通过 252 项 JVM 冒烟并编译 199 个设备源文件，Gradle `assembleFullRelease lintFullRelease` 成功。模拟器验证包重新编译当前 Java 和资源，复用原安装包的原生库与词库；没有重新构建 Rust/原生库，没有在真机重复本轮验收。
