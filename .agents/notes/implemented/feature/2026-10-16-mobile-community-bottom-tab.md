# Agent Note: 触屏宿主底部第二格恢复为社区，页内切换皮肤与词库

Status: implemented

## Problem

[AI 对话改为二级页面](2026-10-16-mobile-chat-secondary-page.md)（#8213）把底部第二格换成了「全部设置」，底部成了「设置 / 全部设置 / 统计 / 我的」。「全部设置」打开的是设置标签页的同一份分组列表，只少了键盘状态卡，两个标签页做同一件事；#8254 又为它单独维护了一条导航栈。产品方要求底部保持四格，第二格换成真正不同的入口。

## Decision

iOS、Android 和 HarmonyOS 手机（共享触屏壳）的底部是「设置 / 社区 / 统计 / 我的」（第一格后来改名「首页」，见 [手机底部第一格改名首页](2026-10-25-mobile-home-tab.md)）。

- **社区页**：标签页的根页面，标题「社区」，顶部固定「皮肤 / 词库」两个分类（皮肤在前），页面打开期间记住所选分类。每个分类就地嵌入该类作品的完整社区列表，搜索、分类、详情、获取或添加、举报、发布都在原处。iOS 是 `CommunityHomeView`，嵌入 `SkinCommunityView(embedded: true)` 和 `CommunityResourcesView(kind: .dictionary, embedded: true)`，所选分类存在 `AppNavigation.communitySegment`；Android 是 `CommunityTabPage`，经 `DetailPage.asTabRoot` 作为根页、不显示返回按钮，两个分类各挂一个嵌入模式的 `CommunityFragment`，切换只改可见性；共享壳的标签 id 是 `community`，用 `useSettingsTabs` 切换，图库画在设置表单之外。
- **「浏览更多」切到社区标签页**：词库页的「浏览更多词库」、皮肤和词库的完整社区列表入口（Android 的 `CommunityFragment.browse` 与 `HomeActivity.openCommunity`、指向这两类的深链；共享壳的 `openCommunityGallery`）都切到社区标签页的对应分类，不在当前栈里再推入一份。回复模板、短语和「我的 → 我的发布与收藏」仍在原页面推入。
- **皮肤页不动**：「皮肤」仍是设置里的一行，皮肤页仍在设置栈里推入，保留 [内置 / 我的 / 社区](2026-10-16-mobile-skin-page-top-tabs.md) 三个分类。
- **「全部设置」整个去掉**：iOS 的 `SettingsView` 不再有 `Stack`、`navigationTitle`、`showsStatusCard`，`AppNavigation.allSettingsPage` 删除；#8254 让设置栈的推入不按当前标签页过滤的修复保留，切走再切回不丢深层页面。Android 去掉 `tab_allsettings` 与 `KeyboardFragment.allSettings()`；共享壳去掉 `more` 页、`MoreSettingsPage` 和首页上的「全部设置」入口。
- **下标兼容**：Android 的 `HostDeepLink.TAB_COMMUNITY` 是 1，与 #8213 之前的社区标签同一下标，旧的下标 1 深链落在社区。共享壳历史记录里旧的 `tab: "more"` 按页面归属回到设置。没有社区服务的共享壳宿主不显示社区格，只有一类服务时不画分类标签。

### 调研与取舍过程

先按 App Store 截图调研：百度输入法底部是「皮肤 / 表情 / 超会写 / 字体 / 我的」，讯飞是「首页（皮肤商城）/ 智能体 / Biu 社区 / AI 输入 / 我的」，都没有「设置」，内容类入口占一格。第一版取「皮肤」，产品方认为皮肤不够撑起一格，改为社区，并在社区里用分类切换皮肤与词库。

### 旧笔记审计

- [AI 对话改为二级页面](2026-10-16-mobile-chat-secondary-page.md)：部分取代，底部四格与全部设置的决定由本篇接管，AI 对话二级页仍成立。
- [iOS 社区融进各个功能页](2026-10-11-ios-community-in-feature-pages.md)、[Android 社区内容并进各功能页](2026-10-11-android-community-into-feature-pages.md)、[触屏设置去掉社区标签页](2026-10-11-touch-community-merged-into-feature-pages.md)：部分取代，社区重新有标签页；功能页里的社区内容和「我的发布与收藏」仍成立。
- [Android 底部第二格换成对话](2026-10-11-android-chat-tab.md)、[触屏对话标签取代社区](2026-10-11-touch-chat-tab-replaces-community.md)：底部第二格已先后被 #8213 和本篇取代。

## Alternatives considered

- **只留「设置 / 统计 / 我的」三格。** 去掉重复最直接；产品方要求保持四格。
- **第二格放「皮肤」。** 主流输入法都有，皮肤页也已是三分类的完整内容；但水杉以设置与打字体验为主，产品方认为皮肤撑不起一格，社区能同时容纳皮肤和词库。
- **「首页 / 设置 / 统计 / 我的」。** 能分出两种用途；但首页内容要重新设计，常用入口仍与设置列表重叠。
- **社区页列出皮肤、词库、短语、回复模板全部类别。** 一处浏览所有作品；但短语只有 Android 有，回复模板只给高情商回复用，放在各自功能页更好理解，产品方只要皮肤和词库。

## Consequences

- 收益：四格各管一件事，皮肤与词库两类主要社区作品一级可达；「浏览更多」回到同一个社区页，不再各推一份完整列表。
- 代价：社区皮肤同时出现在社区标签页和皮肤页的「社区」分类里。iOS 与共享壳切换分类时另一类的搜索词和滚动位置重置，Android 保留。共享壳的 `settings:community` 路由以前在手机上退回「输入」，现在落在社区标签页。

## Verification

- iOS：`OnboardingUITests` 新增 `testCommunityTabSwitchesBetweenSkinsAndDictionaries`（分类顺序、两类列表、发布按钮跟随分类、推入详情和切换标签页后保留分类），`testFeaturePagesBrowseCommunityAndPublish` 断言「浏览更多词库」落在社区标签页的词库分类，`testMainTabsKeepIndependentNavigation` 断言四格且没有对话、全部设置、皮肤；#8254 的两个栈保留用例改用社区标签页验证。
- Android：`BottomTabsSmoke` 核对四个标签与图标、社区根页的两个分类、设置列表有皮肤行、`browse` 皮肤与词库切到社区；`HostDeepLinkSmoke` 核对 `TAB_COMMUNITY == 1`；设备验收 `ChatNavigationDeviceSmoke` 从设置出发检查皮肤行、社区根页无返回按钮、切到词库与系统返回。
- 共享壳：社区根页、分类切换与保留、「浏览更多」切标签、设置列表重新有皮肤、没有社区服务时无社区格、`more`/`skin` 历史回退均有测试。
- 各层实际跑过的命令和结果在 PR 描述中列出。

共享壳的根页渲染接线由[设置社区根页组件](../simplification/2026-10-19-settings-community-page.md)持有；两分类状态继续归外层导航 hook，离开根页后保留分类、重新挂载时重置图库内部搜索的规则不变。
