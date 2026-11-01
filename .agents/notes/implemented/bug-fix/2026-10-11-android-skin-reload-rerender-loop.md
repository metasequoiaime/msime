# Agent Note: Android 跟随系统皮肤下偏好每次重读都整页重画

Status: implemented

## Problem

Android 键盘开着季节应用主题、键盘皮肤是「跟随系统」时，键盘空闲着也每秒整页 `render()` 一次。耗电耗 CPU；之前排查 AI 请求反复发出的问题时，这个每秒一次的整页重画也把那条循环放大了。

`PreferencesReloader` 每秒读一次偏好，回调 `MSIMEInputService.reloadPreferences`。它在重读前记下 `skin.key()`，应用快照后再比一次 `skin.key()`，不等就 `render()`。两次拿到的却不是同一种皮肤：

- 重读前的 `skin` 是上一次渲染末尾 `ImeStyler.applySkin` 写回的 `themed(skin)`，即 `KeyboardSkin.system(dark, 当季种子)`。
- 重读后的 `skin` 是 `applyPreferencesSnapshot` 按偏好重建的 `keyboardSkin(preferences)`，即还没着色的 `KeyboardSkin.system(dark)`，classic 基础色。

`key()` 里带着全部颜色槽位（`designKey`），当季色和 classic 色永远不等，于是每次重读都判为「皮肤变了」，`render()` 又把 `skin` 换回当季色，下一秒循环再来。

`ImeStyler.themed` 的写法本来就是「SVC 按偏好重建 `skin` 时拿 classic 基础色，着色时统一换掉」，所以 `skin` 字段在渲染前后可以是两种值，这是有意的；错的是重读的比较拿这个字段的原始键当「画出来的样子」。10 月 6 日设置应用改版（#3782）引入 `themed`，10 月 8 日新装默认改为跟随系统皮肤（见 [触屏键盘新装默认跟随系统皮肤](../feature/2026-10-08-touch-keyboard-default-follows-app-theme.md)）之后，所有新装用户都会进到这个循环。classic 种子不可选，`refreshAppTheme` 解析不出时也回退到秋杉，所以跟随系统皮肤下种子不会是 classic，不存在不触发的情况。

## Decision

- `KeyboardSkin.displayKey(seed)` 给出一张皮肤在某个应用主题种子下实际画出来的样子的键。跟随系统皮肤（`followsAppTheme()`：不是用户设计且 id 为 `system`）只取 `id:dark`，因为它的颜色只由明暗和种子决定；其他皮肤取 `key()`。种子的 `id:season` 对所有皮肤都拼进去：命名皮肤的回车键和开启态底色取应用主题的季节色（`KeyboardSkin.withAppTheme`），换应用主题或换季时命名皮肤也要重画一次。工具栏品牌键原本也是拼种子的理由之一（logo 色不论哪张皮肤都取季节色），2026-10-31 起它改取当前皮肤自己的配色（`KeyboardSkin.logoDisc` / `logoMark`），见 [手机键盘品牌键跟随当前皮肤](2026-10-31-mobile-logo-follows-keyboard-skin.md)。
- `ImeStyler.themed` 改用同一个 `followsAppTheme()` 判断该不该着色，判定只有一处。
- `reloadPreferences` 前后都比 `skin.displayKey(imeStyler.appThemeSeed())`，不再比 `skin.key()`。比较的是输入（明暗、种子、非跟随皮肤的配色），不是着色后的派生值，也就不依赖 `skin` 字段此刻有没有被着色过。
- `applyPreferencesSnapshot` 在 `refreshLocalSettings()` 之后立刻 `imeStyler.refreshAppTheme(true)`：应用主题存在本地设置里，季节随月份变，每次重读都按刚读到的本地设置重新解析种子，候选条外观（`candidateAppearanceFor` 也经 `themed` 着色）和皮肤比较用的就是同一个当前种子。渲染时的 `refreshSeasonIfNeeded` 一分钟才查一次，只靠它的话设置页换了应用主题，键盘最多要等一分钟。
- `reloadPreferences` 另比一次本地设置快照（`AndroidLocalSettings.Snapshot.equals`），变了就 `render()`。单手模式、调试开关这类本地设置是在 `render()` 里套用的（`refreshSeasonIfNeeded`），这个循环存在的五天里它们靠每秒一次的误重画即时生效；修掉循环之后要显式地在它们变化时重画一次，否则设置页改了要等下一次按键才看得到。`load` 按文件时间戳缓存快照，没变时返回同一个对象，这一比几乎没有代价。

## 守卫

- `platforms/android/tests/keyboard/KeyboardSkinDisplayKeySmoke.java` 按重读的真实顺序构造前后两张皮肤（渲染后的 `system(dark, 种子)` 与重建的 `system(dark)`），断言它们的原始 `key()` 不等而 `displayKey` 相等；换明暗、换季、同一季换应用主题、换皮肤、命名皮肤改配色都必须让 `displayKey` 变化，换季重画之后的下一次重读又必须稳定。
- `check-host.sh` 拒绝 `reloadPreferences` 里重新出现 `previousSkin = skin.key()` 或 `previousSkin.equals(skin.key())`，并要求比较走 `skin.displayKey(`。

## Alternatives considered

- **让 `skin` 字段始终保存着色后的皮肤**（在 `applyPreferencesSnapshot` 里 `skin = imeStyler.themed(nextSkin)`）：比较两边自然一致，`key()` 也不必改。但 `skin` 有五六个赋值点（`onCreate` 的皮肤片段、无会话外观、键盘里换皮肤、隐私模式重建会话……），`ImeStyler.themed` 的注释明确说「不必改 SVC 的每个赋值点」，就是为了不让每个赋值点都记得着色；只改这一处，其他赋值点照旧是 classic 色，换一条路径进来就又是两种值。
- **在 `reloadPreferences` 两边都比 `imeStyler.themed(skin).key()`**：语义最直接，就是「画出来的颜色」。但季节变化时 `themed` 对命名皮肤原样返回，捕捉不到品牌键 logo 色的变化，还得另比种子（品牌键这一条自 2026-10-31 起不再成立，见 [手机键盘品牌键跟随当前皮肤](2026-10-31-mobile-logo-follows-keyboard-skin.md)；回车键和开启态底色仍随种子变）；而且每秒都要在种子变化后新建一张推导出的皮肤。比输入（明暗加种子）更便宜，也能在不依赖 Android 运行时的 JVM 冒烟里直接测。
- **只用 `refreshAppTheme(false)`，沿用一分钟的节流**：少一次每秒的本地设置读取和日期计算。但这两样本来就在每次重读里做（`refreshLocalSettings` 每秒都调），强制解析的增量只是一次字符串比较和一次 `LocalDate.now`，换来的是设置页换应用主题后键盘下一秒就跟上。

## Consequences

- **收益**：偏好和本地设置没变时，重读不再触发 `render()`，键盘空闲时不再每秒整页重画。换皮肤、换明暗、换应用主题、跨月换季、改本地设置时各重画一次。
- **代价**：每次重读多一次 `refreshAppTheme(true)`（本地设置里取一个字符串、读一次当前月份）和一次本地设置快照比较。`displayKey` 是 `KeyboardSkin` 的新公开方法，以后加了新的着色规则（例如命名皮肤也跟应用主题换底色）要同步改它和 `followsAppTheme()`，否则又会出现两边不等或该重画不重画；冒烟里的换季、换主题断言会先报出来。
- **未覆盖**：表情和手写面板的皮肤（`emojiSkin`、`handwritingSkin`）不在重读比较里，`applyPreferencesSnapshot` 也不重建它们，这次没有改动它们的刷新时机。
