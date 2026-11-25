# Agent Note: iOS 手机键帽统一 44pt、第三排等宽、数字九宫格与 ⌫ 横滑删除

Status: implemented

## Problem

iOS 手机竖屏 26 键的键帽是 44pt，键盘外框却按 52pt 键帽算（`KeyboardFormFactor.keyboardHeight` 给 26 键加 `letterHeightCompensation` 32pt），多出的高度垫在键区上下；切到数字九宫格、符号层时按键铺满那一截，键区一高一低。26 键第三排七个字母平分 ⇧、⌫ 之外的整段，比第一排字母宽约 7%，与系统键盘不同。数字九宫格左列运算符要滚动才看得全，删一长段文字只能连点或长按 ⌫。

## Decision

- 手机竖屏所有键面统一 44pt 键帽：`KeyboardHeightPercent.portraitKeyHeight(tablet:)` 返回 `tablet ? 54 : 44`，`keyboardHeight` 去掉 26 键的补偿，外框按实际按键累加。默认 7pt 行距下手机竖屏键块从 229pt 变为 197pt。`touch_keyboard_height_adjustment` 仍是加在键块上的 pt 数，不迁移已存的值。iPad 不变（54pt）。
- 高度上限随键帽放宽 32pt：iOS 的可调范围 `KeyboardHeightPercent.adjustmentRange` 从 -12...48 改为 -12...80，手机竖屏最高仍是 197 + 80 = 277pt，与改前 229 + 48 相同，调高过的用户升级后能调回原来的绝对高度；下限不变。共享文档的 `touch_keyboard_height_adjustment` 是跨平台、随设置同步的字段，client-core 只接受 -12..=48，所以不改它（`KeyboardHeightPercent.sharedAdjustmentRange`）：App Group 的 `keyboard.height.adjustment` 存完整值，写文档时截到 48；文档镜像回 App Group 时，本机值截到 -12...48 后与文档相同就保留本机值，否则说明文档在别处改过，跟文档（`KeyboardLayoutPreference.reconciledHeightAdjustment`）。设置页滑块、步进、键盘调整的上沿柄和百分比换算都改用这个范围，键盘扩展同步文档时取对齐后的 App Group 值。
- 第三排字母与第一排等宽（宽度约束优先级 999），⇧、⌫ 保持 `phoneLetterEdgeWidth`。多出的宽度变成 ⇧ 与 Z、M 与 ⌫ 之间的空隙，由四格 `KeyRoutingSpacer` 撑开，靠外一格把触摸交给 ⇧ / ⌫，靠里一格交给 Z / M，空隙里没有死区。⇧ 之后、⌫ 之前照常留一个键距，所以键盘窄到占位格缩成 0 时（单手模式、iPad 浮动键盘）空隙也不小于普通键距。主体在 `KeyboardViewController+PhoneThirdRow.swift`。
- 数字九宫格：左列五项等高排满；底行 中/英 键显示时改为句点键，右列中间键改为 =；隐藏 中/英 键时中间键仍是句点。符号面板底栏固定 40pt。
- ⌫ 横滑删除：按住 ⌫ 往左滑，每滑一格删一个单位，不松手往右滑按原样恢复，松手定下来。每格距离是 `24 - 2 × 档位` pt，档位 1–10，默认 7（10pt），只存本机 App Group（`KeyboardLayoutPreference.backspaceSwipeSpeed`），不进共享文档。没有组字时按 `Character` 删光标前的文字，恢复时把删掉的字插回；组字时每格调一次 `session.segmentBackspace()`，恢复时把删掉的组码按原顺序经 `handleCharacter` 重打。纯状态在 `BackspaceSwipeDeletion.swift`，控制器逻辑在 `KeyboardViewController+BackspaceSwipe.swift`。同一个 pan 的竖直向上分支是上滑清空，与横滑按起手方向分派，见 [iOS 26 键上滑、下滑各一个可配动作](2026-11-23-ios-letter-two-way-swipe-actions.md)。

## 横滑删除的边界

- 组码重打不回原样的场景 never 接横滑，⌫ 保持点按和长按连删：手写板、就地组字的方案（韩文、注音、越南文）、14/18 键组码网格、本地模式、已选了一段词的组字、笔画、日文，以及 `BackspaceSwipeDeletion.canRetype` 不成立的组码。
- 有选区时 never 接横滑：第一次 `deleteBackward` 删掉的是整段选区，按光标前文字记账会在恢复时插回从没删过的字。
- 删已上屏文字 must 先清掉上屏后的联想：这条路径直接写宿主、不经过 session，回显又被 `textWillChange` 当成自己的编辑跳过 refocus，不清的话联想候选和数字选词会留在已删掉的文字后面。
- 开始滑动 must 先 `cancelBackspacePress()`，松手不再多删一个字，也停掉长按连删的计时器。

## Related notes

- [Android 退格键同次按压左右滑动删除与恢复](2026-10-23-android-backspace-scrub.md)：部分重叠。同一需求在 Android 的实现，Android 按到键盘左边的距离映射删除比例、按下即删并建快照；iOS 按固定步长逐格删、不建快照，组字时按音节删。两边各自保留。
- [iOS 数字侧栏触控区](../bug-fix/2026-10-18-ios-number-sidebar-touch-targets.md) 与 [iOS 14/18 键字母宽度](2026-11-10-ios-fourteen-eighteen-key-letter-widths.md)：部分重叠，本次没有改它们的决定。

## Alternatives considered

- 保留 52pt 外框、只把多出的高度改成均匀内边距：切层不再跳，但键盘仍比系统键盘高 32pt，空白没有用处，因此统一按 44pt 键帽算外框。
- ⇧ 与 Z 之间完全不留键距、全由占位格撑开：正常宽度下与现方案一样宽，但占位格只约束为不小于 0，窄键盘上会缩成 0、让 ⇧ 贴着 Z，因此保留外侧键距。
- 把共享字段 `touch_keyboard_height_adjustment` 的范围放宽到 -12..=80：iOS 不用分本机值和文档值，但这是跨平台、随设置同步的契约，已发布的其他客户端收到 48 以上的值会校验失败，Windows、HarmonyOS 也按 48 封顶；Android 同样是把更大的范围存在本机设置里（`platform.android.keyboard_height_adjustment`），iOS 照这个做法。
- 升级时把已存的调整量加 32pt 自动迁移：调高过的用户不用手动调，但已授权的决定是不迁移已存的值，而且迁移后 +16 以上的值仍超过共享文档的上限，要走同样的本机存储。
- 横滑删除照搬 Android 的比例映射和按下即建快照：恢复更稳，但 iOS 的 `textDocumentProxy` 只能读到有限的光标前文字、没有绝对光标位置可核对，按步长逐格删和插回更简单，也能在组字时按音节删。

## Consequences

- 默认键盘矮 32pt，切层不跳；第三排与系统键盘一致；数字九宫格不滚动就能看到四个运算符。
- 已调高度的用户升级后同样矮 32pt（存的 pt 数不迁移），需要自己再调高一次；上限放宽后能调回原来的高度。超过 +48 的部分只留在本机，不随设置同步到别的设备，别的设备收到的是 +48。另一台设备恰好把高度设成 +48 时，本机若存着 +48 以上的值会保留本机值。iPad 的上限也随之放宽到 +80，比以前多 32pt。「更多」和输入方案面板在默认高度下格子按 `pickerLayout` 缩到约 50pt。数字九宫格上不能直接切中英文。横滑删除没有开关，只有速度档位。
- 以上出厂值变化（52→44pt、中/英 改句点、横滑删除默认开、选择面板格子变小）已由维护者授权（https://github.com/metasequoiaime/msime/pull/11770#issuecomment-7078315911）。维护者要求以前能调到的最高高度仍能调到，因此放宽了高度上限，见上面的决定。
- 只改了 iOS，Android 和 HarmonyOS 的键盘高度与数字九宫格没有跟进。

## Testing

`BackspaceSwipeDeletionTests` 覆盖步长映射、目标格数、组码可重打判断；`BackspaceSwipeGestureTests` 用内存文档驱动 `handleBackspaceSwipe`，覆盖删除并恢复文字、选区不接横滑、清掉联想、拼音与九键组字按音节删并恢复。`KeyboardFormFactorTests` 检查第三排两段空隙相等且在窄键盘上不小于键距，`MorePanelFoldTests` 钉住默认高度下的选择面板格子高度。`KeyboardGeometrySettingsTests` 钉住新上限下的手机键块等于旧上限（277pt），并覆盖 +80 存进 App Group、文档写 48、镜像时保留本机值、文档在别处改过时跟文档。
