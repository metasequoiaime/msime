# Agent Note: 鸿蒙手机工具栏可选工具、整行隐藏与长按进设置

Status: implemented

## Problem

Android 的键盘工具栏有显示方式（输入时显示 / 隐藏）和七个可选工具的开关，工具全关时整行不画（`MSIMEInputService.toolbarRowHidden`），长按工具栏任一按钮进「编辑工具栏」。用户在 Android 上提出这两项后，要求三个移动宿主一致。鸿蒙手机键盘的工具栏却是写死的七格：标志、表情、常用语、剪贴板、皮肤、输入方式、收起，没有任何设置能隐藏其中一格，也没有「隐藏」显示方式，设置页的「屏幕键盘」页不给工具栏开关（`touch_toolbar_components` 只对 iOS、Android 为真），长按工具栏按钮什么也不做。

[iOS 与鸿蒙宿主采用全平台设计稿](2026-10-07-ios-harmony-all-platform-design.md) 当时决定鸿蒙不在 `host_surface.rs` 为自己打开 `touch_toolbar` 的显示开关，理由是那会让剪贴板默认被隐藏，与设计稿相反。这个理由仍然成立，所以本篇不推翻它，而是另起一组只给鸿蒙的偏好，取代那条决定里「鸿蒙的工具栏不可配置」的部分。

已检索 proposed、implemented、rejected 中的 `touch_toolbar`、工具栏、`toolbarRowHidden` 和鸿蒙键盘页。[共享工具栏分组](../simplification/2026-10-15-touch-keyboard-toolbar.md) 持有 `touch_toolbar` 的八项和 `TouchKeyboardToolbarSection`，本篇在同一个分组里加鸿蒙的几行，不改那八项；[Android 工具栏排序](2026-10-15-android-toolbar-order.md) 持有 Android 的长按入口和排序，本篇照它的长按语义做鸿蒙这边，排序不跟；[鸿蒙键盘页布局](2026-10-11-harmony-keyboard-page-layout.md) 的「恢复默认」照旧重置工具栏，现在多重置这一组。Android 与 iOS 这边「工具全关时整行不显示」和 iOS 长按进编辑记在[触屏工具栏工具全关时整行不显示](2026-10-17-touch-toolbar-empty-hides-row.md)。

## Decision

### 偏好：`harmony_toolbar`

共享偏好新增 `harmony_toolbar`（client-core `HarmonyToolbarPreferences`）：`emoji`、`phrase`、`clipboard`、`skin`、`scheme` 五个工具开关和 `hidden`（显示方式选了「隐藏」）。缺省五个全开、`hidden` 为 false，就是有这组设置之前鸿蒙键盘的样子，老用户在动开关之前看不到任何变化。等于缺省时不写进文档（`skip_serializing_if`），没有这个键的旧版本照样能读；文档里只写了几项时其余取缺省。标志键和收起键固定在两端，不可关。

鸿蒙的本机设置就是共享偏好（设置页和键盘读写同一份 `PreferencesStore`），所以开关放在偏好文档里，而不是像 Android 那样另放本地设置文件，也不像按键反馈那样另开一个宿主文件和桥方法。

不加进账号同步：`harmony_settings.rs` 只导出、应用各移动宿主共有的触屏键，工具栏这类只有一个宿主的键原本就不同步（Android 的工具栏键也要等服务端字段表收录才上传），换设备要重选。

### 能力位：`touch_toolbar_tools`

`HostCapabilities` 新增 `touch_toolbar_tools`，只对 `HostPlatform::Harmony` 为真。同一个 HAP 也跑在 2in1 上，而 2in1 的键盘没有这一行，所以鸿蒙设置桥（`Settings.ets` 的 `hostCapabilities`）按设备形态再投影一次：`SettingsFormFactorCapabilities.touchToolbarTools` 只在手机上为真。TS 类型里这一位是可选的，缺省按不提供处理，因此 iOS、Android、Windows、macOS、Linux 和网页预览的设置页一行都不变。

设置页的「屏幕键盘」页在 `touch_toolbar_tools` 为真时，在「工具栏」分组里画「显示方式」（输入时显示 / 隐藏）和五个工具开关，文案与 Android 的「键盘工具栏」一致；选了「隐藏」时工具开关置灰；整行会消失时（隐藏或工具全关）在组末说明键盘会变矮、打字时照常显示候选栏。「恢复默认」删除 `harmony_toolbar`，说明里因此列出「工具栏按钮」。

### 键盘：哪些时候画这一行

规则在纯 TypeScript 的 `TouchToolbarPolicy` 里，扩展 ability 定初始面板高度和视图改高度用同一份：

- `rowHidden`：`hidden`，或五个工具全关（这一行只剩标志和收起），空闲时整行不画，按键行上移，面板变矮（`KeyboardMetrics.totalHeightVp` 的 `withStrip` 为 false 时去掉 50vp 条带和它下面 10vp 的间距）。
- `stripShown`：组字、候选、提示这些要占这一行的内容一出现，候选栏照常画；有面板盖住按键时（表情、符号、语音等，九键长按和日文变体这两种只换按键行键面的不算）也画，因为标志键是回到按键的出口，否则从底行打开的表情面板没有路回去。
- 工具栏照常显示时，关掉的工具不占格，剩下的格子等宽排开。

视图在决定 `stripIdle` 的几个状态上挂 `@Watch('syncStripHeight')`，`show()` 换界面后也调一次；只有「画不画这一行」真的变了才请求面板改高度，组字时每个字母都会走到这里但不触发 resize。

### 长按进设置

工具栏的每个按钮（五个工具、标志 `BrandTile`、收起）都挂 `LongPressGesture`：给一下按键反馈（`KeyboardSession.holdFeedback`，即按键振动），回到按键，再经已有的 `openSettings` 打开设置应用的 `screen-keyboard` 页。长按被识别后这次触摸不再算点按，原来的点按动作不会跟着执行。`screen-keyboard` 加进 `SettingsPageLink.PAGES` 和 `apps/harmony` 的 `LINKED_PAGES`，`scripts/test-harmony-settings-links.py` 核对两份一致。

## Alternatives considered

- **鸿蒙也读共享的 `touch_toolbar`，打开 `touch_toolbar_components`**：一组偏好三个宿主共用，设置页不用加东西。但它的缺省是剪贴板关，鸿蒙老用户会在升级后丢掉剪贴板按钮，这正是 2026-10-07 那条决定拒绝它的理由；它的八项里还有五项（键盘设置、AI、简繁、全半角、标点）鸿蒙工具栏上没有，常用语和输入方式又不在里面，设置页会给出一半不起作用的开关。
- **像按键反馈那样放在鸿蒙宿主自己的文件里**：完全不碰共享偏好的结构，其他宿主看不到这个键。但要新增一个桥方法、一个页面侧的读写钩子和键盘侧的文件读取，而鸿蒙的设置页本来就整份读写共享偏好、键盘本来就在偏好变化时重读；按键反馈单独成文件是因为它要跟 Android、iOS 的本地反馈结构对齐，工具栏没有这层约束。
- **只做长按、不做隐藏**：改动最小。但用户要的两项是一起提的，三个移动宿主要一致，只做一半会让鸿蒙的长按打开一个没有对应能力的页面。
- **整行隐藏时保持面板高度不变，只把这一行留空**：不用在组字开始、结束时改面板高度，按键不会上下移动。但用户要的正是键盘变矮；留一条空白和现在只剩标志、收起的那一行没有实质区别。

## Consequences

- 收益：鸿蒙手机键盘能关掉不用的工具，能整行隐藏让键盘变矮；长按任一工具栏按钮直接到能改它的设置页，和 Android 一致。缺省不变，没动过开关的用户看不到差别。
- 代价：工具栏隐藏时，开始组字、打开面板会让键盘长高一行，结束后再变矮，面板 resize 是异步的，可能有一帧按键被裁或留白。隐藏时功能面板（只有标志能打开）也就打不开了，这一点与 Android 相同；Android 的功能面板为被关掉的表情、剪贴板补了格子，鸿蒙的功能面板没有补，关掉剪贴板、常用语、输入方式后，键盘上就没有它们的入口。
- 长按打开的是整个「屏幕键盘」页，工具栏分组在「布局」「尺寸」之后，手机上可能要往下滚一屏；Android 打开的是只有工具栏分类的编辑页。鸿蒙也没有做工具排序。
- `harmony_toolbar` 不随账号同步。
- macOS 的 `preference_coverage.py` 把 `harmony_toolbar` 列为 macOS 不适用。

## Verification

- `cargo test -p msime-client-core` 覆盖 `harmony_toolbar` 的缺省、缺省不进文档、部分文档和存取往返，以及 `touch_toolbar_tools` 只对鸿蒙为真。
- `platforms/harmony/tests` 的 node 套件覆盖 `TouchToolbarPolicy`（缺省、部分文档、全关、隐藏、组字和面板时重新显示）、不带条带时画出的框架与面板高度相等、2in1 框架不看这一位，以及手机才声明 `touchToolbarTools`、`SettingsPageLink` 收录 `screen-keyboard`。
- vitest 覆盖工具栏分组的鸿蒙几行（勾选、显示方式、置灰、全关说明、没有能力位时不画），以及鸿蒙手机「屏幕键盘」页改开关后保存出的 `harmony_toolbar`、2in1 页面没有这组开关。
- `hvigorw assembleHap` 的 ArkTS 编译通过本篇的全部 `.ets` 改动。当时 develop 上 `KeyboardSession.fetchAiTranslation`（#8400）自带的 ArkTS 错误会让编译先失败，验证时在本地临时去掉那个函数体才走到打包，那处错误不属于本篇。
- ArkTS 的视图行为（`@Watch`、条件画条带、`LongPressGesture` 与 `onClick` 同挂时只触发长按、面板 resize 的时序）没有在鸿蒙设备或模拟器上看过。
