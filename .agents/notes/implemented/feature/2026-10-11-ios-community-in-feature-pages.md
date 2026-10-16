# Agent Note: iOS 社区融进各个功能页，原社区标签换成对话

Status: implemented

社区重新有底部标签页，见 [触屏宿主底部第二格恢复为社区](2026-10-16-mobile-community-bottom-tab.md)；功能页里的社区内容与「我的发布与收藏」仍按本篇。

皮肤页的社区预览与「浏览更多皮肤」已被 [iOS 与鸿蒙手机皮肤页改为内置、我的、社区顶部分类](2026-10-16-mobile-skin-page-top-tabs.md) 取代：社区皮肤改为皮肤页的「社区」分类，`SkinCommunityView` 重新有嵌入形态。底部标签另见 [AI 对话改为二级页面](2026-10-16-mobile-chat-secondary-page.md)。词库、回复模板的入口不变。

## Problem

iOS App 照全平台设计稿有「设置 / 社区 / 统计 / 我的」四个标签页（见 [iOS 与鸿蒙宿主采用全平台设计稿](2026-10-07-ios-harmony-all-platform-design.md)）。社区标签页按皮肤、词库、回复模板分三段，和用它们的功能页互不相连：在皮肤页点「去社区找皮肤」会切到另一个标签页，返回键回不到皮肤页；词库页列着几份社区词库，完整列表却在另一个标签页；回复模板只给键盘的高情商回复用，在社区里看不出它和哪个设置有关。产品决定去掉社区模块，「把它融合进各个功能里，和皮肤一样」，底部仍保持四个标签，社区那一格换成「AI 对话」。

## Decision

- **底部是设置 / 对话 / 统计 / 我的。** `AppNavigation.Tab` 的 `community` 换成 `chat`，只为社区标签页服务的 `communityCategory`、`communityRoot`、`discoverSkins()` 和标签页的根 `CommunityHomeView`（分段切换加统一的发布按钮）一起去掉，`SkinCommunityView` 不再有 `embedded` 形态。
- **「对话」复用 App 里已有的 AI 对话页 `KeyboardTryoutView`，不另写对话功能。** 它本来就是「边聊天，边试键盘」的页面（经水杉后端交给 EveryAPI），之前从设置状态卡的「试用键盘」推入，iPad 在侧栏里选中 `.tryout`。现在这两处都改为 `AppNavigation.openChat()` 切到对话标签页，iPad 侧栏的 `.tryout` 去掉，同一个页面不再有两份入口。`chatFocusRequest` 区分来路：从「试用键盘」过来时把焦点放进输入框（与原来推入时一样弹出键盘），直接点标签栏进来不弹键盘。标签页上的导航栏标题是「对话」。引导流程最后一步和社区皮肤试用里仍各自嵌着这个页面：前者出现时还没有标签栏，后者要在原地「恢复原皮肤」或「保留使用」。
- **每类作品放在用它的功能页里，页内直接列出前几份，最后一行「浏览更多…」在这一页自己的导航栈里推入该类作品的完整列表。** 完整列表就是原来的 `SkinCommunityView` 和 `CommunityResourcesView`，搜索、分类、详情、评分、举报、发布、编辑新版本和下架都原样保留：
  - 皮肤：`SkinSettingsView` 在主题网格之后列出社区皮肤第一页的前 4 款，用和主题格相同的两列缩略图。点一款就换上：还没领取的先 `CommunitySkinInstall.take` 下载进「我的设计」（计入下载次数），再 `CommunitySkinInstall.apply` 放到键盘上，正在用的那款描强调色外圈。原来的「去社区找皮肤」改为「浏览更多皮肤」（标识符仍是 `skinCommunityLink`），推入「皮肤社区」。
  - 词库：`DictionarySettingsView` 的「发现词库」照旧列出前 8 份并带「添加」，末尾加「浏览更多词库」（`dictionaryCommunityLink`），推入「词库社区」。
  - 回复模板：放在「键盘 → AI 润色与回复」页（`ServiceSettingsView(kind: .ai)`），因为高情商回复用的就是这一页配置的服务。`CommunityReplyTemplatesSection` 列出前 3 份并带「添加」，末尾「浏览更多回复模板」（`replyCommunityLink`）推入「回复模板社区」。列表和这一节的「添加」共用 `CommunityReplyInstall`。
  - 短语：iOS 没有社区短语包（`CommunityResourceKind` 只有 `dictionary` 和 `reply`），表达页不加社区内容，与之前一致。
- **不带范围的 `CommunityResourcesView` 是一个独立页面。** 标题是「词库社区」「回复模板社区」，行内标题，自己带发布按钮；带范围时，「我发布的」带发布按钮，「收藏的」不带。
- **「我的 → 我的发布与收藏」不变。** 它本来就直接推入 `SkinCommunityView(onlyMine: true)` 和按范围分开的 `CommunityResourcesView(kind:initialScope:)`，与功能页推入的是同一组列表，是唯一一处跨类型的索引。之前唯一跨标签页跳到社区的是皮肤页的 `discoverSkins()`。
- **模拟器的社区预览（`-communityPreview`）没有账号，`SkinCommunityView` 和 `CommunityResourcesView` 的发布按钮在这种情况下直接打开表单，** 代替原来 `CommunityHomeView` 里同样的旁路，UI 测试靠它走到发布表单。
- 键盘扩展不受影响：它只读 App Group 里的皮肤库和回复模板库，这次没有改它们的格式或写入方式。

## Alternatives considered

- **保留社区标签页，只在功能页多加入口。** 最强的理由是改动最小，想随便逛的人仍有一个总入口。不用它，是因为产品要的是去掉这个模块，而且两边都能进会留下两条返回路径不同的入口，跨标签页跳转丢上下文的问题还在。
- **去掉社区标签后只留三个标签。** 最强的理由是不必为空出来的位置找内容。不用它，是因为产品要求底部保持四个标签，把社区那一格换成 AI 对话。
- **「试用键盘」照旧在设置里推入一份对话页，对话标签页是另一份。** 最强的理由是设置页的行为和 UI 测试都不用变，返回键回到设置首页。不用它，是因为两份页面各有一个 `KeyboardChatModel`，在一处聊到一半，到另一处看不到，产品也要求去掉重复入口。
- **「浏览更多」用 sheet 弹出完整列表，或者切到一个隐藏的社区导航栈。** 最强的理由是完整列表可以保持原来的大标题和分段外观，不依赖调用方的导航栈。不用它，是因为完整列表里还要推详情页、再弹发布和登录面板，sheet 里套导航栈再套 sheet，返回和关闭的语义都变复杂；推入同一条栈，返回键自然回到功能页。
- **回复模板放在「表达」页。** 最强的理由是设计稿把社区内容（「发现短语」）画在表达页。不用它，是因为设计稿那一格是短语包，iOS 没有；回复模板只被高情商回复使用，它的端点、模型和开关都在 AI 润色与回复页，放在表达页反而要解释它和标点、联想有什么关系。
- **皮肤页的社区皮肤沿用「获取 / 使用」两步胶囊。** 最强的理由是与完整列表一致，也不会因为点错一下就计一次下载。不用它，是因为产品要求和皮肤一样，而皮肤页上的主题格都是点了就换上；两步胶囊仍在「浏览更多皮肤」里。

## Consequences

- **收益**：每类社区作品都在用它的地方，进完整列表再返回仍停在原来的功能页；AI 对话从设置里的一个按钮升格为常驻标签，对话记录在切换标签时保留。
- **代价**：没有一个同时浏览三类作品的总入口，跨类型的只剩「我的发布与收藏」。皮肤页点一下社区皮肤就会下载并占用「我的设计」的一个槽位（共 12 个），满了会提示删除一款。完整列表从大标题的标签页根变成了行内标题的推入页。对话页离开时仍按原来的做法取消正在等的回复（`onDisappear` 里 `chat.cancel()`），切到别的标签再回来要点「重新发送」。点「试用键盘」不再推入页面，而是切换标签，返回设置要点标签栏。其他平台不在这次改动里，Android、鸿蒙仍有社区标签页；它们若跟进，应按同样的作品归属放置。

## Verification

- 构建：`xcodebuild build -scheme MSIMEApp`（iPhone 18 Pro Max 模拟器，iOS 27，Xcode 27.0）。
- UI 测试：`OnboardingUITests` 的 `testFeaturePagesBrowseCommunityAndPublish`（三个功能页各自列出社区作品并推入完整列表、发布表单可达、没有社区标签）、`testSkinPageAppliesCommunitySkinAndBrowsesMore`（点社区皮肤即换上，完整列表里显示使用中）、`testCancellingPublicationPreservesCommunitySearch`、`testSkinDownloadOpensTryoutAndRestores`、`testMainTabsKeepIndependentNavigation`（四个标签，点「对话」进对话页且不弹键盘）、`testKeyboardHomePrioritizesTryoutAndQuickAdjustments` 与 `testChatLoginIsFocusedAndCancelReturnsToTryout`（「试用键盘」切到对话标签）。
