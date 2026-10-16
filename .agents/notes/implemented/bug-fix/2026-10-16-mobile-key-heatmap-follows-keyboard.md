# Agent Note: 触屏统计热力图按真实键盘绘制

Status: implemented

## Problem

产品方在 iPhone 上发现统计页「按键」里的热力图和实际键盘不一样。三端的热力图各自手写了一份键位：iOS 的 26 键底行还是 123 / 地球 / 空格 / 中英 / 回车，九键带句号和 0；Android 九键画了 `Comma`、`Period`、`Nine0` 和独立的「符」列，26 键第三行左端是 ⇧；鸿蒙也停在旧底行。键盘这几次改动（[26 键底行加符号键](../feature/2026-10-16-26key-bottom-row-symbol-key.md)、九键去掉 0、全拼第三行改为「分词」、中文字母大写）没有一处同步到热力图，按键次数因此落在不存在的键上，或者真实的键没有画出来。

## Decision

三端的触屏热力图都按各自键盘在手机竖屏、默认设置下的真实排布绘制：同样的键、顺序、宽度份额和键面，每个键带着键盘按下时记录的 id，计数落在按下的那个键帽上。

- 26 键（全拼）：字母大写；第三行是 分词 | Z…M | ⌫；底行是 符 | 123 | 空格 | 中 | 回车。第二行是否缩进照各平台键盘自己（iOS 两端各 5%，鸿蒙各半键，Android 不缩进）。
- 9 键（拼音）：左列 ，。？；3 × 3 字母格，首格 @# 记 `Nine1`；右列 ⌫ / 分词 / ！（Android 是 ⌫ / 重输 / ！）；底行同 26 键，没有 0。
- 布局尽量取自键盘本身的常量：Android 的 `KeyHeatmapLayout` 由 `KeyboardLayout`、`KeyboardActionRow.designEntries`、`NineKeyLayout` 等推出；鸿蒙的测试直接读 `NineKeyLayout`、`KeyIdPolicy` 核对；iOS 的单元测试在 402pt 宽的真实 `KeyboardViewController` 上量出键位与宽度，再和热力图比对。键盘再改，测试会失败。

三端统一的计数规则：

- **共用一个 id 的键全部着同一档颜色，都不印占比。** 九键的 ，。？！ 都记 `SoftPunctuation`，存储分不出按的是哪一个；只在一个键上印数字会被读成它单独的占比。
- **「分词」格只算 `Quote`。** 英文模式下同一键位是 ⇧、记 `ShiftLeft`，热力图画的是中文键盘，⇧ 列进「其他键」。
- **按过但没画出来的键列在热力图下的「其他键」里**，按次数从多到少。Android 原先没有这一栏，这次补上。
- 空格键印「空格」，不跟键盘印方案简称。

顺带：iOS 热力图标题旁的「26 键 / 9 键」切换（`StatisticsLayoutSwitch`）在 iOS 26 起画成液态玻璃胶囊，与 `DesignSegmentedControl` 一致。

### 旧笔记审计

- [26 键底行加符号键](../feature/2026-10-16-26key-bottom-row-symbol-key.md)：其中「Android 热力图画成同样的底行」一条由本篇接管，已改为指向这里。
- [底行符号与回车让位](../feature/2026-10-09-mobile-bottom-row-symbols-and-return-yield.md)：只提到误触计数不进热力图，仍成立。

## Alternatives considered

- **三端共用一份热力图布局描述。** 改一处三端都对；但三端键盘本身的第二行缩进、九键右列（分词 / 重输）和宽度份额并不相同，热力图要对的是用户手上那块键盘，统一描述只会和其中两端不一致。
- **「分词」格把 `Quote` 和 `ShiftLeft` 合计。** 反映这个键位的总使用量，Android 与鸿蒙最初这样做；但热力图画的是中文键盘，这一格叫「分词」，把英文 ⇧ 的次数算进来会让分词看起来比实际常用，iOS 也没有这样做。
- **共用 id 的键只在第一个上印占比。** 少一处空白；但数字会被当成那一个键的占比。

## Consequences

- 收益：热力图和用户手上的键盘一一对应；键盘再改时由测试拦下，不再悄悄跑偏。
- 代价：九键四个标点键看不到各自的占比；`Quote` 也收符号层和实体键盘的撇号，会少量落进「分词」格。设置里的键盘预览（iOS `KeyboardSkinPreview`、鸿蒙「尺寸」页的 `harmonyTouchKeyboardRows`）仍是旧底行，不在本次范围。

## Verification

- iOS：Mac Studio 上 `MSIMEClientTests`、`MSIMEClientUITests` 的 `build-for-testing` 通过；本机 iPhone 17 / iOS 27.0 模拟器上 `TypingStatisticsTests`（两处）通过，`testStatisticsChartsAndDaySelection`、`testStatisticsOffShowsOnlyTheRecordingNotice` 通过，截图核对 26 键与 9 键热力图。同批运行的 `NineKeyKeyboardTests.testQuanpinSeparatorKeepsChineseAndSplitsCurrentComposition` 失败在打 `xi'an` 后候选里找不到「西安」，与本次统计改动无关（本次没有改键盘与引擎），未在 develop 上复核。
- Android：`check-host.sh` 通过（含 `KeyHeatmapLayoutSmoke`、`KeyPressCountingSmoke`）；Studio 上 `compileFullReleaseJavaWithJavac` 通过。未在设备上查看。
- 鸿蒙与共享界面：`tsc --noEmit` 通过；`vitest run tests/settings tests/core` 2566 个用例全部通过；`platforms/harmony/tests/run.sh` 0 failure。未改 `.ets`，不需要重新打 HAP。
