# Agent Note: 三个移动端的手写区铺满整行、竖屏加高一行

Status: implemented

## Problem

iOS、Android、HarmonyOS 的触屏手写键盘原来是一行三列：左侧竖排 ，。？！，中间书写区，右侧竖排 ⌫ 和「重写」。#9707 把三端统一成 .6 / 4 / .7 的行宽份额、把虚线辅助框的内缩从设计稿的 14 收窄到 8，iPhone XR（414 pt）上的虚线框从约 236×150 pt 扩到 282.5×162 pt，维护者仍嫌小：两侧列占掉一行的四分之一，书写区的高度又被锁在三行键高。

## Decision

三端用同一套手写键盘：

- **书写区铺满键区宽度**，只留键盘本身的边距，两侧不再有标点列和工具列。iOS 把 `HandwritingInputView` 直接放进键区（`keyColumn`），Android 的 `ImeLayoutRows.rebuildHandwritingRows` 一行里只有书写区，HarmonyOS 的 `handwritingPad` 只剩画布那一层。
- **竖屏手写比其他布局高一行**：书写区有四行键高，键盘总高多一行键高加一道行距；横屏（屏幕宽大于高，手机和平板都算）保留原来的三行。规则写在 iOS `KeyboardFormFactor.handwritingAddsRow`、Android `KeyboardGeometry.handwritingRows`、HarmonyOS `KeyboardMetrics.handwritingExtraRows`。
- **底行**：`123 |（地球键）| ， | 空格 | 。 | 中 | 重写 | ⌫ | 回车`，份额 1.25 /（1）/ 1 / 2.4 / 1 / 1.05 / 1.25 / 1.25 / 1.6。iOS 是 `KeyboardViewController.handwriting*Weight` 与手写专用的那组宽度约束，Android 是 `KeyboardActionRow.designEntries(HANDWRITING_LAYOUT)` 与 `HANDWRITING_*_WEIGHT`，HarmonyOS 是 `handwritingActionRow` 与 `HANDWRITING_*_SHARE`。地球键只在宿主要求时出现（iOS `needsInputModeSwitchKey`、Android `offersGlobeKey`），HarmonyOS 没有这个键。
- **？ 和 ！**：， 长按输入 ？，。 长按输入 ！，两键右上角画角标（iOS 是 `KeyboardKeyButton.cornerHint` 加一个角标标签，Android 是 `KeyHintButton`，HarmonyOS 是 `KeyOptions.cornerHint`），与 26 键「逗号长按输入句号」是同一种手势。iOS 用户自定义了常用标点时 ， 仍按那份菜单，不画 ？ 角标；Android 和 HarmonyOS 没有这个设置。
- ⌫ 有笔迹时撤回一笔，没有时删除光标前的字符；「重写」清空书写区。两个键的行为与原来工具列上的相同。
- 虚线辅助框距卡片边缘 8（iOS `HandwritingCanvasCard.guideInset`、HarmonyOS `HANDWRITING_GUIDE_INSET_VP`），沿用 #9707：设计稿 dc.html 是 14，收窄到 8 让可见书写区更大。Android 的卡片只画十字线、没有虚线框，卡片距书写区边缘 3 dp，正好是默认键距的一半，卡片左右边缘与上下各行的键对齐。

`scripts/test-mobile-handwriting-pad-parity.py` 比对三端的底行键序与份额、书写区一行只有画布、竖屏多一行与横屏不加的规则、长按 ？ ！ 的入口，以及 iOS 与 HarmonyOS 的虚线框内缩，并算出 320 pt 带地球键时 ， 的宽度不得窄于 22 pt。

## 底行的取舍

一行要放下 123、， 。、空格、中/英、重写、⌫、回车，320 pt 宽时还可能多一个地球键（iPad 浮动键盘是手机形态、带地球键）。空格从 4 份让到 2.4 份：手写时空格主要用来上屏首选和按住说话，候选条本身也能点；2.4 是空格还画得下麦克风和「手写」两个字的下限，先试的 2 份在 Android 360 dp 上只剩约 57 dp，`SpaceKeyFace` 把「手写」截成了「手」。iOS 的空格键原来沿用系统按钮两侧各 12 pt 的内边距，在 320 pt 的 60 pt 空格上同样截字，现在两侧和图标间距都取 Android `SpaceKeyFace` 的 4（内容居中，其他布局的宽空格看不出差别）。回车从 1.9 让到 1.6，与九键底行的回车同宽。腾出的宽度给「重写」和 ⌫ 各 1.25 份，两个字的「重写」在 320 pt 上约 31 pt 宽，放得下 15 pt 的标签。， 。各 1 份：320 pt 不带地球键时约 25 pt，带地球键时约 22.5 pt，与 26 键字母键（约 26 pt）相当。

中/英留在底行：手写里夹一个英文词要一键切过去再一键切回来，放进方案选择面板就成了两三步。三个平台的方案选择面板都有「英文 26 键」，但那是换方案，不是临时切换。⌫ 紧挨回车，所以手写底行上回车不把键帽让给左边的键（`KeyboardActionRow.languageBesideReturn` 为假，iOS 的 `KeyGapRouting` 只对紧挨的中/英生效，HarmonyOS `designReturnKey(…, false)`）。

## 高度

- iOS：竖屏手写的键盘高度按五排算（`keyRows(numberRow:handwriting:landscape:)`），底行固定 44 pt，余下全给书写区；用户的高度调整照旧整份加在总高上，打开表情等面板时键区盖住同样的高度，不跳。iPad 竖屏同样多一排 54 pt，横屏的手机、iPad 都不加。
- Android：书写区是一块四行高的键块（`adjustRowBlockHeight(row, 4)`），分到底行以外那部分高度调整，底行照旧分四分之一，总高 = 五行键高加整份调整。横屏是三行。
- HarmonyOS：`KeyboardMetrics.totalHeightVp` 的 `extraRows` 多算一个不带调整的 `ROW_HEIGHT_VP` 和一道行距，`KeyboardView.surfaceHeight` 画同样的高度。面板高度由视图在方案、中英文、123 层、本地模式和横竖屏变化时重定（`syncStripHeight`），扩展能力建面板时按没有多出的行算，视图出现后补上。
- 123 / #+= 层不是手写区，三端都回到标准高度：从手写切到数字页时键盘矮一行，切回来再高一行。

## 坐标

- iOS 与 Android 的 ML Kit 收到画布坐标，`WritingArea` 取画布实际宽高，书写区变大不需要换算；iOS 的笔画按画布 `bounds` 夹住，画布 `clipsToBounds`，铺满后贴着键盘边缘的笔画同样记录。
- HarmonyOS 原来把画布横竖分别缩放到引擎的 420×420（`x × 420 / 宽`、`y × 420 / 高`）。书写区宽高比约 1.8，这样会把字压扁近一半，而引擎会再把整个字的外框按原比例缩进自己的画布，压扁的字形就带进了识别。现在横竖按同一比例缩放，较长的一边对齐 420（`HandwritingStrokePolicy.recognitionPoint`）。`redrawHandwriting` 按画布实际大小清屏：平板和折叠屏上书写区比 420 vp 宽，只清 420×420 会在右侧留下上一笔。

## 测量

| 平台与宽度 | 改前（#9707）卡片 / 虚线框 | 改后卡片 / 虚线框 |
| --- | --- | --- |
| iOS 414 pt | 298.5×178 / 282.5×162 pt | 408×237 / 392×221 pt（虚线框面积 ×1.89） |
| iOS 320 pt | 228×178 / 212×162 pt | 314×237 / 298×221 pt（×1.92） |
| Android 411 dp | 300×171 dp（无虚线框） | 399×229 dp（×1.78） |
| Android 360 dp | 261×172 dp（无虚线框） | 348×231 dp（×1.79） |
| HarmonyOS 360 vp（按公式） | 约 257×146 / 241×130 vp | 352×197 / 336×181 vp（×1.94） |

iOS 数据来自模拟器 XCTest 中真实 `KeyboardViewController` 的布局；Android 来自 API 35 模拟器上 uiautomator 读到的画布边界（卡片为画布减去四周各 3 dp）；HarmonyOS 没有设备，按 `surfaceHeight` 与默认 7 vp 行距、6 vp 键距、两侧 4 vp 内边距推算。

## 验证

- iOS：iOS 26.5 模拟器（iPhone 11）上 `MSIMEKeyboardTests` 943 项通过、8 项跳过；`HandwritingTests` 在 320、390、414 pt 下检查书写区铺满、四排高、底行键序与份额、？ ！ 角标和空格不截字。
- Android：`check-host.sh` 的 JVM 冒烟（底行组成与份额、几何、热力图）通过；API 35 模拟器 360 dp 与 411 dp 上量了画布和底行，长按 ，。 分别输入 ？ ！，⌫ 在空画布上删字；横屏书写区仍是三行高；从手写打开表情面板，键盘不变矮。
- HarmonyOS：`platforms/harmony/tests/run.sh` 通过（面板高度、多一行、横屏不加、坐标等比映射），`hvigorw assembleHap` 编出 HAP；没有设备，界面和真机识别未验收。

## Alternatives considered

- **保留两侧列，只把份额收得更窄（如 .5 / 4 / .6）**：改动最小，键盘高度不变；但 320 pt 时标点键会窄到约 29 pt，书写区宽度只多十几 pt，高度一点没动，达不到维护者要的「明显更大」。#9707 已经走过这条路。
- **只铺满宽度、不加高**：键盘总高不变，切换方案时不跳；但书写区高度仍是三行（约 178 pt），宽高比被拉到 2.3 左右，写一个字主要缺的是高度。#9707 当时以「切换方案时键盘总高跳动」为由否掉加高，这次维护者明确要求加高，跳动只发生在进出手写和它的数字页时，用户能预期。
- **去掉底行的 中/英，把 ⌫ 放在它的位置**：底行少一个键，空格能宽回 3 份；但英汉混写要多两步，三端的方案面板入口也不完全一样（iOS、Android 的英文卡片是换方案）。
- **？ ！ 放进 ， 的长按常用标点菜单**：不用新增手势，现成菜单里就有；但要长按再选一次、看不到入口，原来是一点就有的两个键。改成角标长按，与 26 键的逗号长按句号一致。
- **把书写区向上延伸进候选条**：手写的识别结果就显示在候选条里，占用它会让候选无处显示（#9707 已否）。
- **HarmonyOS 继续横竖分别缩放到 420×420**：引擎只看笔画外框，分别缩放在旧的书写区上也存在，看起来没坏；但书写区越宽，字被压得越扁，与 iOS、Android 交给 ML Kit 的原始比例不一致。

## Related notes

- 取代 #9707 的「三列份额 .6 / 4 / .7」决定，那一篇的测量、内缩理由和备选已并入本篇，原文删除。
- [鸿蒙手写识别](../bug-fix/2026-10-25-harmony-handwriting-input.md)：识别链路不变，本篇改了它的坐标映射比例。
- [手机底行中/英挪到回车左边](2026-10-09-mobile-language-key-beside-return.md)：手写底行的回车左边改为 ⌫，中/英在「重写」左边。
- [手机底行逗号长按句号](2026-10-16-comma-key-secondary-period.md)：手写底行用同一种长按，逗号长按改输入问号、句号长按输入叹号。
- [Android 底行与字母行同高](2026-10-10-android-equal-bottom-row.md)：竖屏手写是「各布局总高相同」的例外，多一行。
- 检索「手写」相关的其余笔记（鸿蒙手写板设置预览、Android 手写会话重置、手写候选按钮与面板状态的简化、Windows 7 手写等）只涉及设置动画、识别会话或桌面端，与本篇无关。

## Consequences

收益：书写区（iOS、HarmonyOS 量虚线框，Android 量卡片）面积约为 #9707 的 1.8–1.9 倍，三端同一个样子并由契约检查锁定；HarmonyOS 的识别不再把字压扁。

代价：竖屏手写的键盘高一行（手机 iOS 59 pt、Android 59 dp、HarmonyOS 51 vp），编辑区相应变矮，进出手写和它的数字页时键盘高度跳一行；空格只剩原来的六成，回车也窄了一些；？ ！ 从一点变成长按；iOS 自定义常用标点的用户在手写里拿不到 ？ 的长按。HarmonyOS 只有 node 逻辑测试与 CI 的 HAP 构建，界面与真机识别未在设备上验收。
