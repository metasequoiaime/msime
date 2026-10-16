# Agent Note: 手机 26 键底行改为 符 | 123 | 空格 | 中 | 回车

Status: implemented

## Problem

#6409 把手机 26 键底行定为 `123 | ， | 空格 | 。 | 中 | 回车`，符号面板要靠长按 123 或依次按 123、#+=、符号才能打开；iPhone 上「符」键从一开始就被常量 `showsFullKeyboardSymbols = false` 关着。九键底行已经改成 `符 | 123 | 空格 | 中 | 回车`（[九键底行与 1 键](2026-10-15-nine-key-symbol-key-and-quick-marks.md)），产品方要求 26 键与它一致：底行最左边加「符」，并去掉 ， 和 。 两个键。

## Decision

- iOS、Android、HarmonyOS 手机的 26 键字母层（全拼、双拼、五笔、英文、韩语等用设计底行的方案，中英文两种模式）底行是 `符 | 123 | (地球) | 空格 | 中/英 | 回车`。符 占 1.05 份（与 中/英 相同，空格 4 份），点按打开符号面板，与九键底行的 符 是同一个键、同一个计数 id；Android 交换语言键与数字键时 符 仍在最左。
- 26 键底行不再有 ， 和 。 键，也就没有 ， 键长按的常用标点菜单；标点从 符 面板或 123 层输入。
- 不变的布局：全拼 14 键（第三排已有 符/分词）、手写和大千注音的底行仍是 `123 | ， | 空格 | 。 | 中 | 回车`；123 / #+= 层的底行不变；iPad、HarmonyOS 2in1 的底行不变。HarmonyOS 平板与手机共用触屏底行，也会出现 符。
- 三端按键统计页的热力图照真实键盘绘制，见 [触屏统计热力图按真实键盘绘制](../bug-fix/2026-10-16-mobile-key-heatmap-follows-keyboard.md)。

这一篇取代 [手机底行](2026-10-09-mobile-bottom-row-symbols-and-return-yield.md) 里 26 键 `123 | ， | 空格 | 。 | 中 | 回车` 的布局；该篇的长按 123 打开符号面板、回车让出给 中/英 的规则仍成立。

## Alternatives considered

- **保留 ，：`符 | 123 | ， | 空格 | 中 | 回车`（搜狗的做法）**：逗号是中文里最常用的标点，留着它打逗号只要一下，长按还能出 。？！：；。对比过搜狗、微信、百度、豆包的 26 键截图，四家都留有逗号键。产品方选了去掉，与九键底行完全一致。
- **只加 符、保留 ， 和 。**：改动最小，但底行七个键，空格最窄，产品方没有选。

## Consequences

- 收益：26 键与九键底行一样，符号面板一键可达；空格比原来宽。
- 代价：打逗号、句号要先开 符 面板或切到 123 层，至少多按一下；， 键长按的常用标点菜单在 26 键上没有入口了。
- Android 设备用例 `InitialKeyboardRowDeviceSmoke` 以「九键首帧出现句号键」判定首帧画错底行，26 键也不再有句号，这个判据不再能区分，需要另找判据。

## Verification

- iOS：`NineKeyKeyboardTests.testPhoneBottomRowFollowsTheDesignsWeights` 改为断言 `符 | 123 | 空格 | 中 | 回车` 的顺序和宽度、中英文两种模式都没有 ， 和 。；`FourteenKeyKeyboardTests` 断言 14 键底行没有第二个 符。测试在 Studio 上 `build-for-testing`、拉回本机 iOS 27 模拟器运行。
- Android：`check-host.sh` 通过，`KeyboardActionRowSmoke` 覆盖 26 键与韩文的四种顺序（有无地球键、交换与否）及 14 键、手写、大千注音仍有逗号句号；未在设备上运行。
- HarmonyOS：`tests/run.sh` 与 51 个 `scripts/test-harmony-*.py` 通过；磁盘不足，未运行 `hvigorw assembleHap`，ArkTS 改动没有经过编译器，交给 CI。

手机 26 键的标点与符号位置由后续[逗号次级句号决定](../../implemented/feature/2026-10-16-comma-key-secondary-period.md)部分取代；九键与其他布局边界仍沿用本文。
