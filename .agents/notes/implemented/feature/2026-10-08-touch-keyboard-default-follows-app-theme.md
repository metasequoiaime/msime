# Agent Note: 触屏键盘新装默认跟随系统皮肤，与应用主题同色

Status: implemented

## Problem

新装用户打开「试用键盘」，键盘是薄荷绿渐变加深绿回车键，应用界面是秋季的米色加赭红，赭红的发送键紧挨着深绿的回车键。两个默认值是分别定的，后来没人对齐：

- 9 月 30 日 #2178 把触屏构建（Android、iOS、鸿蒙）新装的 `global_theme` 定为 `custom`，自定义主题带「薄荷晨光」键盘设计。它是固定配色。
- 10 月 6 日设置应用改版，应用主题默认「四季」`siji`，配色随月份变。

薄荷晨光又属于用户自己的键盘设计，`KeyboardSkin.withAppTheme` 对这类皮肤原样返回，连回车键的强调色也不跟应用主题。所以除了绿色系的季节，键盘和应用永远是两套颜色。在别人的应用里看不出来，最显眼的恰恰是我们自己的引导和试用页。

另外，Android 15 三键导航下，键盘下面的导航栏是一条白边：系统默认对三键导航栏做对比度增强，导航栏透明时铺一层接近不透明的白衬底，盖住了键盘底色。

## Decision

- `Preferences::default()` 在所有构建上都是 `global_theme: system`、空的自定义主题，去掉了 `TOUCH_KEYBOARD_BUILD` 和两个按构建区分的默认值函数。三端的跟随系统皮肤本来就按应用主题当前季节取色（Android `ImeStyler.themed`、iOS `KeyboardTheme.resolve(GlobalThemeCatalog.systemId)`、鸿蒙 `AppThemeStore`），键盘与应用界面因此是同一套颜色，随季节一起变。
- 已保存过偏好的用户不受影响：文档里总带着 `global_theme`。改变的只是新装，以及「恢复默认设置」回到的样子。
- 薄荷晨光保留：`TouchKeyboardSkinDesign::mint_morning` 和各端皮肤模板列表里的第一位都不动，喜欢的用户一点就能换回去。
- Android 的 `ImeFrame.applyNavigationBar` 对输入法窗口调用 `setNavigationBarContrastEnforced(false)`（Android 10 起）。导航栏按钮的深浅已经按键盘底色切换，不需要系统再垫衬底。

## Alternatives considered

- **保留薄荷晨光默认，把应用默认主题改成固定的春雅（绿色系）**：键盘保持 #2178 选定的品牌样子，应用也对得上。但应用全年固定一种颜色，四季主题就失去了默认的意义，而且只要用户换了应用主题，割裂就回来了。
- **保留薄荷晨光默认，只让它的回车键和强调色跟应用主题**：改动小，发送键和回车键颜色一致。但薄荷绿底配秋冬的米色、灰白界面，整体仍是两套颜色。另外按现有规则，用户自己的设计保留自己的强调色，为一个默认值破例会让「自定义设计」的语义不一致。
- **只改 Android 的默认值**：影响面最小。但默认值在 client-core 里三端共用，iOS 和鸿蒙有同样的四季应用主题和同样的割裂；分平台再加一层条件，只会让默认值更难推断。

## Consequences

- **收益**：新装用户第一眼看到的键盘与应用同色，换季或换应用主题时一起变；Android 15 三键导航下键盘底色铺到屏幕底边。
- **代价**：推翻 #2178「手机默认薄荷晨光」的决定；宣传图或文档里如果用薄荷晨光当默认键盘，需要另行更新。
- **验证**：在 Studio 上用 `rbuild` 跑 `cargo test -p msime-client-core` 全部通过（1091 项），clippy 零警告。在全新的 API 35 专用 AVD 上装 release 签名的 APK：首页「皮肤」显示「跟随系统」，试用键盘是秋季米色、回车键与发送键同为赭红；三键导航下白边消失；手势导航并关掉系统输入法按钮时，键盘底栏与键盘同色。iOS 和鸿蒙没有装机验收，依据是两端跟随系统皮肤按季节取色的现有测试。

相关：出厂薄荷配色曾在 Android 候选条启动时露出来，见 [2026-10-08-android-candidate-strip-startup-palette](../bug-fix/2026-10-08-android-candidate-strip-startup-palette.md)；季节主题只用于强调色和跟随系统皮肤的约定，见 [2026-10-07-ios-harmony-all-platform-design](2026-10-07-ios-harmony-all-platform-design.md)；同一次反馈引出、已在 #6004 合入的键盘底栏见 [2026-10-08-android-keyboard-bottom-bar](2026-10-08-android-keyboard-bottom-bar.md)。想要 iOS 自带键盘样子的用户另有「原生」皮肤，「跟随系统」仍是季节色，见 [2026-10-09-ios-native-keyboard-skin](2026-10-09-ios-native-keyboard-skin.md)。
