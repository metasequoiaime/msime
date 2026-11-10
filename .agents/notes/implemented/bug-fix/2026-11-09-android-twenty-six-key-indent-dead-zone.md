# Agent Note: Android 26 键第二排缩进归 A、L

Status: implemented

## Problem

标准 26 键第二排比首排少一个键，A 左边、L 右边各有半个键宽的缩进（`ImeLetterRows#indent()`）。缩进是不可点击的普通 `View`，`KeyboardKeyArea` 只把落在键的外边距（键距、行距）里的按下交给那个键，落在缩进里的按下没有键接：不出字，没有按键音和振动，看起来像漏键。手指斜着够屏幕边缘时最容易落在这里，用户反馈经常点到 A 左边的空白。iOS 的缩进是行的 `layoutMargins`，`KeyGapRouting` 把整段边距算给最外侧的键，没有这个问题。

## Decision

缩进改成 `KeyboardSpacer`，建完第二排后左边的指定给这一排第一个字母键（A），右边的指定给最后一个字母键（L），和 [14/18 键「宽窄」第二排两端的占位格](../feature/2026-11-06-android-fourteen-eighteen-key-letter-widths.md)同一个机制：`KeyboardKeyArea` 把落在占位格里的按下交给所属的键，按下点挪进它靠这一侧的边缘，按下即反馈和双拼的按下点记录都按那个键算。

不变的部分：
- 缩进的份额（0.5）和外边距（没有）都不变，键帽位置、键盘外观和读屏范围不变；`ImeStyler` 套用键距时给它的外边距是 0。
- 第二排带 `;` 键时（`MicrosoftShuangpinKeyPolicy`）和日语罗马字有长音键时缩进收起（`updateSecondRowIndent`），隐藏的占位格不接收触摸，不用改归属。韩文、注音键面没有缩进。
- 分离式键盘中间的空隙（`splitGap()`）仍是普通 `View`，点它什么也不发生。缩进还在行的两端，分离计算照旧按份额算它。
- 按下只在同一排里归属，不跨排。

## Historical audit

- [14/18 键字母键宽度](../feature/2026-11-06-android-fourteen-eighteen-key-letter-widths.md)引入 `KeyboardSpacer` 时写明 26 键缩进这次不变、机制可以直接用；本篇落实，那篇的这句事实已就地更新。
- [Android 十八键键面](2026-10-31-android-eighteen-key-surface.md)去掉了 18 键的第二排缩进，与本篇无关。
- [日语罗马字长音键](2026-11-08-android-japanese-romaji-long-vowel.md)让日语第二排十格、不带缩进，与本篇无关。

## Alternatives considered

- **像 iOS 一样把缩进做成行的内边距，由 `KeyboardKeyArea` 把行边距里的按下交给最外侧的键。** 不用多一个视图，和 iOS 的写法对齐。不用：分离式键盘按子视图份额切分第二排（`SplitKeyboardPolicy`），缩进是其中两格，改成内边距要改分离计算；而且 `KeyboardKeyArea` 要另加一条「行边距归最外侧的键」的规则，和已有的占位格归属重复。
- **把 A、L 加宽到包住缩进，键帽只画在内侧。** 按下天然落在键上。不用：键的布局范围和画出的键帽不一致，读屏、按键预览、按下点记录和键位几何导出都按键的范围算，都要改。

## Consequences

收益：A 左边、L 右边的缩进不再丢按键，和键距、行距的规则一致。改动只在建第二排的地方指定归属，没有新的机制。

代价：无新增的运行开销，`KeyboardKeyArea` 本来就逐个子视图判断占位格。

验证：`ShuangpinTouchDeviceChecks`（经 `MicrosoftShuangpinDeviceSmoke -e shuangpin-touch true`）在小鹤 26 键上注入真实触摸，按在 A 左边、L 右边离键帽 0.3 个键宽处（已经出了键自己的外边距），分别组出 `a`、`l`；API 35 模拟器（1080×2400）上通过。`check-host.sh` 通过（编译设备套件源码与 JVM 冒烟），未在 API 28 上跑。

问题跟踪：用户反馈，未建 issue。
