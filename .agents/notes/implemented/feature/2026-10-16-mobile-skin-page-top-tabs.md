# Agent Note: iOS 与鸿蒙手机皮肤页改为内置、我的、社区顶部分类

Status: implemented

## Problem

产品方要求「皮肤页面的不同皮肤改成顶部 tab」，并与 Android 一致。Android 的 `SkinsPage` 早已在标题下固定「内置 / 我的 / 社区」三个分类（见 [Android 皮肤分类改为固定顶部 Tab](2026-10-13-android-skins-top-tabs.md) 和 [Android 社区皮肤 Tab 直接承载完整列表](2026-10-13-android-skin-community-tab-list.md)）。iOS 和鸿蒙手机的皮肤页却仍是一列滚动：内置主题、几款社区皮肤和「浏览更多皮肤」、保存的设计混在一起，分类位置随滚动消失。

## Decision

iOS 的 `SkinSettingsView` 和鸿蒙手机的共享皮肤页在标题下固定「内置 / 我的 / 社区」，一次只显示当前分类，页面打开期间记住所选分类。

- **内置**：内置主题卡片（按键盘明暗过滤，同原来），以及原有的外部皮肤包；iOS 在末尾保留「明暗与候选颜色」。
- **我的**：先是 AI 设计皮肤卡片，再是皮肤库里保存的设计，点一下就换上（iOS 走 `GlobalThemePreference.apply`，与编辑器的「使用」、键盘皮肤面板同一路径）。没有保存的设计时写「还没有保存的皮肤。可到社区获取喜欢的皮肤。」，与 Android 同句。iOS 末尾是「设计我的皮肤」；鸿蒙的「我的皮肤」卡片也在这一类。
- **社区**：直接嵌入完整的社区皮肤列表，搜索、分类、详情、举报、发布都在原处。iOS 用 `SkinCommunityView(embedded: true)`，嵌入时不改页面标题；鸿蒙由设置外壳在面板里画 `CommunitySkinsPage`，放在设置 `<form>` 之外，因为社区页自带搜索和发布表单。没有社区客户端的宿主不显示这一类。

原先皮肤页里的前几款社区皮肤预览和「浏览更多皮肤」一起去掉，社区内容只在「社区」分类里。iOS 的分段控件复用 `DesignSegmentedControl`（标识符 `skinTab-0/1/2`），鸿蒙复用 `useSettingsTabs`。桌面、鸿蒙 2in1 和 Android 不变。

iOS 顺带修正一个已有缺陷：键盘已用着一款自定义设计时，在社区里对另一款点「使用」，按钮停在「使用」。当前设计存在带点号的键（`customKeyboardSkin.v1`）里，`@AppStorage` 观察不到它的变化；现在改为 `@State` 持有，并在列表出现、主题变化和「使用」之后重读。

### 旧笔记审计

- [鸿蒙手机皮肤网格列出皮肤库里的设计和社区皮肤](2026-10-11-harmony-skin-grid-saved-designs.md)：部分取代。网格里列社区第一页的做法去掉，保存的设计改到「我的」分类；皮肤库读取与打勾规则仍成立。
- [iOS 社区融进各个功能页](2026-10-11-ios-community-in-feature-pages.md)：部分取代。皮肤页的社区预览与「浏览更多皮肤」改为「社区」分类，`SkinCommunityView` 重新有嵌入形态；词库、回复模板的入口不变。
- Android 两篇皮肤分类笔记：无冲突，本篇让 iOS 与鸿蒙跟上它们。

## Alternatives considered

- **保留一列滚动，只在顶部加分类锚点。** 改动最小；但滚动后分类再次消失，Android 当初也因此改成固定分类。
- **「社区」分类只放几款预览加「浏览更多」。** 能少挂一份完整列表；但多一层推入，Android 已经改为直接承载完整列表，三端应一致。
- **鸿蒙把分类状态放在皮肤页组件内。** 封装更紧；但社区面板必须在设置表单之外，分类只能由外壳持有，与桌面「我的皮肤 / 社区皮肤」的做法相同。

## Consequences

- 收益：三端皮肤页结构一致，分类始终可见；社区皮肤一级可达。
- 代价：iOS 切离「社区」分类会丢掉其搜索词和已加载列表，回来时重新加载，Android 保留。鸿蒙的分类栏随页面滚动，Android 固定在标题下。iOS「内置」仍有「自定义」主题格，Android 已把它交给「我的」。鸿蒙从「我的 → 我发布的皮肤」等入口仍打开全屏社区图库，不切到「社区」分类。

## Verification

- iOS：Mac Studio 上 `xcodebuild build-for-testing`（`MSIMEClientTests`、`MSIMEClientUITests`）通过；本机 iPhone 17 / iOS 27.0 模拟器上单元测试 934 通过、7 跳过；13 个涉及皮肤页的界面测试 12 个通过，其中新增 `testSkinPageTabsTakeCommunitySkinIntoMine` 覆盖三类内容、空状态、社区「获取 → 使用 → 使用中」和「我的」里出现并选中。`testSettingsPersistAndExposeGuideAndTryout` 在打开皮肤页之前的 `reachSettingsLink` 失败，与本次改动无关，未在 develop 构建上复核。
- 鸿蒙与共享界面：`tsc --noEmit` 通过，`vitest run tests/settings` 2199 个测试全部通过（含三类分类、我的空状态、无社区客户端时不显示社区、2in1 与 Windows 无分类栏、社区面板在表单之外），`stage-settings.sh` 通过。HAP 未打包，未在设备上看过。

共享皮肤页的分类栏和图库接线归入[设置皮肤社区区域](../simplification/2026-10-19-settings-skin-community-section.md)；分类状态继续由外壳的既有 hook 持有，图库仍在偏好表单之外，不改变原生平台实现。
