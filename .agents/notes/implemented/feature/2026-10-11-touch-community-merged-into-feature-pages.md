# Agent Note: 触屏设置去掉「社区」标签页，社区内容并进皮肤、词库和 AI 辅助页

Status: implemented

触屏壳重新有社区标签页，见 [触屏宿主底部第二格恢复为社区](2026-10-16-mobile-community-bottom-tab.md)；功能页里的社区内容仍按本篇。

## Problem

共享设置界面在触屏宿主上有四个底部标签页：设置 / 社区 / 统计 / 我的（来自全平台设计稿，见 [iOS 与鸿蒙按全平台设计稿改版](2026-10-07-ios-harmony-all-platform-design.md)）。「社区」把三类互不相干的东西放在一个分段控件后面：键盘皮肤、词库，以及鸿蒙上叫「短语」、实际是 AI 高情商回复用的回复模板。用户要换皮肤得先离开皮肤页去另一个标签页，装词库同理；而皮肤页的网格（[鸿蒙手机皮肤网格列出皮肤库里的设计和社区皮肤](2026-10-11-harmony-skin-grid-saved-designs.md)）和词库页的「发现词库」已经各自直接列出社区的第一页，「社区」标签页成了同一批内容的第二个入口。

仓库负责人的要求是「去掉社区模块，把它融合进各个功能里，和皮肤一样」。本篇只管 `packages/ui` 的共享设置界面；Android、iOS 的原生应用各自处理。

## Decision

- 触屏宿主（`mobilePlatform` 为真：鸿蒙手机，以及同样渲染这套外壳的 Android、iOS Tauri 界面和 iPad 侧栏）的底部标签页不再有「社区」。`mobilePrimaryPageIds` 去掉 `community`，`mobileLastPageByTab` 和 `harmonyTabIcon` 随之去掉它；空出的第二格换成「对话」，底部仍是四个标签页，见 [触屏底部标签栏的第二格从「社区」换成「对话」](2026-10-11-touch-chat-tab-replaces-community.md)；`settingsPageProjections` 在触屏宿主上不提供 `community` 页，旧的历史记录或深链接指向它时落回默认页。桌面不变：「社区」页仍只从账号页「我的内容」进入，`CommunityPage` 等组件都保留。
- 每类社区内容都在它所属的功能页上，完整图库是那一页上的子视图：
  - 皮肤页：鸿蒙手机的网格照旧列出社区第一页，网格下方一行「浏览更多皮肤」；其他触屏宿主沿用「在线皮肤」一行，按钮改叫「浏览更多皮肤」。
  - 词库页：鸿蒙手机的「发现词库」照旧列前 8 个、可直接添加，组末一行「浏览更多词库」；其他触屏宿主加一组「社区词库」，只有这一行。
  - AI 辅助页：新的一组「回复模板」列出社区的前 5 个回复模板（鸿蒙原先「社区」里叫「短语」的那一段就是它们），「添加」与图库里的「添加」同一条路（`addCommunityReply`：收藏、读最新一版、存进高情商回复键盘，没有这个键盘的宿主只收藏），鸿蒙手机用 toast 提示结果，其他触屏宿主写在组里；组末一行「浏览更多回复模板」。桌面的 AI 辅助页不变。
  - 「我的 → 社区作品」各行（我发布的皮肤 / 词库 / 回复模板、收藏的词库 / 回复模板）以及其他走 `openCommunity` 的入口，在触屏宿主上先去到对应的功能页，再在上面打开相应范围（我的作品 / 收藏）的图库，范围映射沿用 `communityDestinationView`。
- 图库子视图由 `useCommunityGallerySubview` 管理，状态放在设置外壳的模型里，`CommunityGallerySubview` 画在设置 `<form>` 之外：图库自带搜索和发布表单，嵌在设置表单里会被 WebView 当成外层表单提交（`dictionary-edit-dialogs.tsx` 的对话框为此不用 `<form>`）。它和 AI 皮肤抽卡一样以页面背景覆盖整个界面（复用 `fullScreenSurface` / `fullScreenColumn`），标题前带返回按钮。图库内容就是原来的 `CommunitySkinsPage` / `CommunityResourcesPage`，鸿蒙手机上用 `look="harmony"`，搜索、分类、详情、评分、举报、试用、发布、我的作品和收藏都在。
- 历史记录约定：打开图库时先 `selectPage` 到所在功能页，再压一条 `{ page: <功能页>, communityGallery: { category, scope, initialMine } }`，与皮肤页的 `skinSubpage` 是同一种做法。`popstate` 时图库的开关只取自记录里的 `communityGallery`，且记录的 `page` 要等于它所在的功能页；离开那一页时图库随之关上。图库里的详情原先压的是写死的 `page: "community"`，现在由 `historyPage` 传入所在功能页（缺省仍是 `community`，桌面不变），而 `pushMobileSettingsState` 会带上当前记录里的 `communityGallery`，所以系统返回依次关掉详情、图库，最后才离开功能页。
- 图库关上时功能页重读它依赖的本机数据：皮肤网格在图库打开期间不算可见，关上后重读皮肤库和社区第一页；鸿蒙手机词库页在图库关上后重读词库列表，图库里添加的词库出现在「已安装」，「发现词库」的「添加」变成「已添加」。图库里「添加」词库与原来的社区页一样：宿主有命名词库时 `installCommunity` 装成独立词库，否则导入本机词库。

共享 React 的资源模型与添加操作分别归入 `community-resource-model.ts` 和 `community-resource-actions.ts`，原图库页面保留兼容导出。详情与图库共用词库安装操作，各入口的提示及生命周期保持原样，见[社区资源公共操作](../../implemented/simplification/2026-10-16-community-resource-actions.md)。共享图库的列表、详情历史和会话添加状态由[资源图库状态](../../implemented/simplification/2026-10-16-community-resource-gallery.md)持有，原有功能页历史约定保持。

## Alternatives considered

- **保留「社区」标签页，只在功能页上加入口。** 最强的理由是改动最小，现有的标签页导航、测试和截图都不用动，想集中逛社区的人仍有一个地方。不用它：要求明确是去掉社区模块；而皮肤和词库已经各自在功能页上直接列出社区内容，再留一个标签页就是同一份内容的两个入口，标签栏也多占一格。
- **把图库做成独立的设置页路由（例如 `community-skins`），像「词库 → 背单词」那样用 `selectPage` 打开。** 理由是返回、标题栏和滑入动画都由外壳统一处理，深链接也能直达。不用它：这样「社区」只是换了个名字又回到页面列表里，图库也不再从属于功能页（标签页高亮、`subPageParents` 都要另配三份）；皮肤页的编辑器和 AI 抽卡已经用历史记录加全屏子视图解决了返回手势，照它做，三类图库的行为和皮肤页现有的两个子视图一致。
- **在各功能页里就地展开图库（放在页面表单里）。** 理由是不需要外壳层的状态，功能页自己管开关。不用它：图库带搜索和发布的 `<form>`，放进设置 `<form>` 里就是嵌套表单，在 WebView 里会整页提交；图库也会被设置保存时的 `fieldset disabled` 一起禁用。

## Consequences

- **收益**：皮肤、词库、回复模板都在用它们的那一页上浏览和添加，标签栏不再为社区占一格（那一格换成了「对话」）；「我的」里的社区作品各行直接落到对应功能页的相应范围。返回手势的层次与皮肤页编辑器、AI 抽卡一致。
- **代价**：想同时逛三类内容的人要分别去三个页面。图库是覆盖整个界面的子视图，打开时看不到底部标签栏。AI 辅助页每次进入都请求一次社区回复模板列表。触屏宿主上的 `CommunityHomePage`（带分段控件的社区首页）不再被用到，组件仍保留给桌面的「社区」页。
- **已知上限**：Android、iOS 的原生应用是否同样去掉「社区」由各自的改动决定，本篇只覆盖共享设置界面；在原生应用改完之前，两边的导航会不一致。以后给功能页加新的子视图，沿用 `communityGallery` / `skinSubpage` 这类历史记录标记，并让 `popstate` 按记录设置开关，否则返回手势会把功能页一起带走。

## Verification

- `apps/desktop/tests/settings/community-gallery-subview.test.tsx`：鸿蒙手机上「我的 → 收藏的回复模板」去到 AI 辅助页并打开收藏范围的回复模板图库，标签栏里没有「社区」，系统返回依次关掉详情、图库，再回到「我的」；「我发布的皮肤」「我发布的词库」落到皮肤页、词库页的「我的作品」范围；皮肤网格下的「浏览更多皮肤」打开图库并压入 `communityGallery`，返回按钮关掉它；图库不在设置表单里。
- 同一文件里：鸿蒙手机 AI 辅助页的「回复模板」只列前 5 个，「添加」依次调用 `save(id, true)`、`detail`、`storeReply` 并提示「已添加「…」到高情商回复键盘」，「浏览更多回复模板」打开全部范围的图库；Android 外壳把结果写在组里；桌面没有这一组，也不读社区列表；Android 外壳的词库页从「社区词库」打开词库图库。
- `apps/desktop/tests/settings/harmony-phone-dictionary.test.tsx`：「浏览更多词库」打开词库图库，关掉图库后重读词库列表。
- `apps/desktop/tests/community/community-skins.test.tsx`、`community-resources.test.tsx`：传入 `historyPage` 时详情记录留在所在功能页，并保留 `communityGallery`。
- `core/mobile-tab-bar.test.tsx`、`settings/mobile-app-chrome.test.tsx`、`settings-page-projections.test.ts`、`settings-navigation-helpers.test.ts`、`settings.test.tsx`、`skin-community-entry.test.tsx`：触屏宿主的标签栏没有「社区」，也没有「社区」页；桌面有社区客户端时仍有「社区」页。
- 没有在 HarmonyOS 模拟器或真机上验证；视觉与手势由后续在模拟器上检查。
