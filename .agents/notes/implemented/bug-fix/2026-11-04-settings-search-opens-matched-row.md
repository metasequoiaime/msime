# Agent Note: 设置搜索结果带着命中的设置项打开页面

Status: implemented

## Problem

Android 设置首页搜「显示页码」，结果是「键盘」、副标题「显示页码」，点开却停在第一个分类「布局」，这一行在「工具栏」，要自己切过去再找。#10719 的真机验收（android-9-search）发现了它，但不是那次引入的：`KeyboardFragment` 的搜索结果和首页普通入口走同一个 `open(page)`，调 `SettingsNavigator.open(context, page, null)`，不带命中的关键词，也不带分类。`DetailPage` 早就认 `HostDeepLink.ARG_SECTION`（键盘的语音引导靠它打开「语音输入 › 服务」），只是搜索从没传过；`PageId` 的关键词表只到页面一级（见 [关键词表由冒烟逐页核对](2026-10-09-android-settings-search-index.md)），没有「这一行在哪个分类」的信息。

iOS 有同类问题：`SettingsPage.destination` 无参创建 `PluginsSettingsView()`，分段固定从「我的插件」开始，`.plugins` 的关键词却有「社区插件」。HarmonyOS 的设置是共享设置页，手机上没有设置搜索；平板 / 2in1 侧栏搜索经 `revealSettingsSearchTarget` 点开所在 `tabpanel` 的标签并聚焦控件，没有这个问题。

## Decision

- 搜索结果把命中的关键词作为目标交给页面。`SettingsSearchAnchorPolicy.anchor`（Android，纯 Java）与 `SettingsSearchAnchor.anchor`（iOS）取第一个包含查询的关键词，规则与 `PageId#matches`、`SettingsPage.matching` 相同；只命中页面标题时没有目标，页面和从首页那一行进来一样打开。结果副标题也用它，原先的 `matchedKeyword` 删掉。
- Android 的目标走新参数 `HostDeepLink.ARG_ROW`（`"row"`）。和 `ARG_SECTION` 一样只用于导航和滚动，外部深链带来也只影响初始位置。
- `DetailPage.onCreate` 只在首次打开（没有保存的状态）时读 `ARG_ROW`。目标等于某个分类标题时直接选中那个分类（插件页的「社区插件」），不等内容画出来。否则记为 `pendingRow`，在内容列每次布局后的回调里（与 `updateSections` 同一个回调）找标题与它完全相同的可见文字：隐藏的子树跳过，未选中的分类列例外。行在别的分类时先切过去并返回，等下一次布局排好再滚，否则拿到的是隐藏时的位置。滚动把这一块放到可见区中间，避开顶上固定的分类导航和底部导航栏，然后用强调色铺一层前景（不透明度 56/255，停 0.7 秒、0.9 秒褪掉），并把读屏焦点移过去。
- 高亮哪一块由 `GroupCard.searchTarget` 决定：文字在卡片的一行里时是这一行，是分组标题（例如「键盘工具栏」）时是这一组的卡片。`GroupCard` 给分组和卡片打上字符串标记来认出它们，页面不用知道 `GroupCard` 的层级。
- `pendingRow` 在找到后、用户自己点分类时、页面滚动位置变化时清空：行要等异步读完设置才画出来，所以要多等几次布局，但用户已经开始自己操作时不再把页面拉走。
- iOS 只有插件页有分段命中：`PluginsSettingsView(searchAnchor:)` 按 `SettingsSearchAnchor.section` 选起始分段，`Segment` 带上自己的标题，分段控件的选项也改从它生成。手机根页和 iPad 侧栏都按当时的查询算目标；iPad 详情按选中项重建（`.id(selection)`），换选中项时才按新的查询定起始分段。iOS 的页面没有逐行滚动，这次不加。

## Alternatives considered

- **在 `PageId` 里给每个关键词标上所在分类** — 不用在视图树里找。但关键词表有几百项，键盘页一页就上百个，分类信息要和页面代码手工同步，页面挪一行就要改两处；冒烟只能核对「在不在表里」，核对不了「在哪个分类」。从已经画出来的视图里找，页面怎么排就落在哪，和共享设置页 `revealSettingsSearchTarget` 按 DOM 祖先找 `tabpanel` 是同一个思路。
- **包含匹配而不是完全相同** — 别名或稍长的标题也能定位到。但「显示页码」会落到任何含这四个字的长标题上，「键盘」这类短关键词几乎命中整页第一行；完全相同时找不到就停在页面开头，和以前一样，不会定位错。
- **固定等一段时间（例如两秒）后放弃寻找** — 不会一直在每次布局后扫视图树。但时长是猜的：词库页的「发现词库」要等联网才画出来，慢网下会被提前放弃；用户的操作（点分类、滚动）才是该停的明确信号，找不到时每次布局扫一遍几百个视图的代价可以接受。
- **iOS 也做逐行滚动和高亮** — 和 Android 对齐。但 iOS 的页面是 SwiftUI 的 `ScrollView` / `List`，各行没有统一的标识，要给每页的行补 `id` 并用 `ScrollViewReader`，改动面远大于这次的缺陷；这次只修「停错分段」。

## Consequences

- **收益**：Android 搜「显示页码」打开「键盘 › 工具栏」并停在这一行、短暂高亮；搜「社区插件」在 Android 和 iOS 都直接打开「社区插件」分段。所有带分类的详情页（表达、词库、AI 润色与回复、开发者选项、隐私、关于等）都按同一条路径定位，新页面不用额外接线。
- **代价**：关键词要和页面上的行标题、分类标题一字不差才能定位；别名（「键盘调整」「折叠屏」）打开时停在页面开头。`GroupCard` 的分组和卡片占用了视图的 `tag`，别处不能再给它们设 tag。一直找不到目标的页面在用户滚动前每次布局都会扫一遍视图树。
- **验证**：`SettingsSearchAnchorPolicySmoke` 钉住选关键词、分类命中、完全相同的认行规则和参数键；`check-host.sh` 的源码守卫钉住两类搜索结果都经 `openResult` 传 `ARG_ROW`、`DetailPage` 读它并在布局回调里找行；`gradlew compileFullReleaseJavaWithJavac` 编译通过。iOS 的 `SettingsSearchAnchorTests` 由 iOS CI 跑，规则另用 `swiftc` 在 macOS 上编译运行过。
- **未覆盖**：没有在真机或模拟器上点过搜索结果：Android 上的切分类、滚动位置、高亮观感和 TalkBack 焦点，iOS 上搜「社区插件」进插件页的起始分段，都留给设备轮次。
