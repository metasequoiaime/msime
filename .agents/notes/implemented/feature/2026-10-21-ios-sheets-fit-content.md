# Agent Note: iOS 底部弹窗和选项面板的高度贴合内容

Status: implemented

## Problem

iOS App 里的底部弹窗大多按系统默认弹成整屏。添加常用语、编辑词条、选服务商、举报、保存皮肤这类弹窗只有一两个输入框或几行选项，却占满整屏，下面空一大片；里面的 `Form` 还是系统灰色，不是 App 的页面底色和卡片。系统分享面板包在 sheet 里时同样被撑成整屏。

设置页的选项面板（`DesignSelectRow` 和 `.designOptionSheet`，约 30 处，中文键盘、识别语言、应用主题、云剪贴板同步方式等）也有同样的问题。在 iPhone XR 上，中文键盘只有 4 个选项，卡片却画到约六成屏高（533pt）：选项挤在中间，标题下面空一大段。原因是列表写成 `ViewThatFits(in: .vertical) { 列表; ScrollView { 列表 } }.frame(maxHeight: 屏高 × 0.6)`，卡片总是撑满上限。

维护者两次在真机上看到这些问题，要求全部修掉（「底部弹窗的高度还是有问题，所有的都要修」）。

## Decision

### 底部弹窗：`fittedSheet()`

`shared/backend-ui/core/FittedSheet.swift` 提供 `fittedSheet()`，加在弹窗里的 `Form`、`List` 或 `ScrollView` 上。iOS 18 起它用 `onScrollGeometryChange` 读出滚动内容的高度加上方留白（导航栏、搜索栏），弹窗停在这个高度，往上拉可以到整屏；iOS 17 读不到滚动内容的尺寸，退回 `[.medium, .large]`；macOS 上什么也不做。它放在和 macOS 共用的 `backend-ui` 里，是因为共用的 `CloudDictionaryEditor` 也是 iOS 上的底部弹窗，而 `backend-ui` 看不到 App 的 DesignKit。

真机上量出来的三条规则：

- **不计底部留白。** `.height(h)` 档位不含底部安全区，系统在它下面另加主屏指示条那一截（XR 上量到容器高 = h + 34）。10-21 那版加了最多 34pt 的底部留白，每个弹窗都多出一截。
- **键盘弹起时只给贴合的那一档。** 档位里只要还有 `.large`，键盘一弹出，系统就把停在贴合高度的弹窗撑到整屏，输入框和键盘之间空出一大块。添加常用语一打开就聚焦输入框，所以它一打开就是整屏。键盘在、用户又没拉到整屏时，档位只剩 `.height(h)`，弹窗贴着键盘上沿；键盘收起时再把 `.large` 放回来。键盘状态取自 `UIResponder.keyboardWillShowNotification` / `keyboardWillHideNotification`。
- **外面必须是 `NavigationStack`。** 在 `NavigationView` 里设的 `presentationDetents` 传不到 sheet 上：个人词库编辑器加了 `fittedSheet()` 以后仍是整屏，也没有拖动条，改成 `NavigationStack` 后才贴合。所以用 `fittedSheet()` 的弹窗一律包在 `NavigationStack` 里（部署目标是 iOS 17，`NavigationStack` 都能用）。

所有内容不多的弹窗都用 `fittedSheet()`，App 自己的表单同时加 `.designPage()`：

- 添加常用语
- 选择服务商
- 云词库编辑
- 个人词库的编辑与导入
- 社区举报
- 发布皮肤、发布词库与回复模板
- 发布插件
- 保存我的皮肤
- 登录后发布
- 验证码登录
- 云剪贴板添加内容，以及它的自建中继、WebDAV 填写页
- 词库同步网盘
- 关于统计
- 键盘深链弹出的精简录音页（同一个页面被推入时不加）

两个系统分享面板设 `[.medium, .large]`。以后新加的底部弹窗，内容只有几行时同样加 `fittedSheet()`，并包在 `NavigationStack` 里。

本身内容就多的弹窗保持整屏：

- 照片选择器
- 皮肤试用：下面要留出键盘
- AI 生成皮肤
- 插件编辑器：行表可以很长
- 共享 `backend-ui` 的社区资源详情和编辑页：标题栏写在 `List` 外面，`fittedSheet()` 量不到
- 语音会话页：`fullScreenCover`
- 欢迎页重放：`fullScreenCover`

登录弹窗自己量内容高度，不变。

### 选项面板：按行数算高度

紧凑宽度的选项面板（`DesignOptionSheetView`）里，选项行是固定 56pt 高、字号不随动态字体变化的单行文字，行间是 0.5pt 分隔线，所以列表高度可以直接算出来，不用量。`DesignOptionSheetMetrics.listHeight(rows:limit:)` 给出 `rows × 56 + (rows − 1) × 0.5`，向上取整到整点，上限是屏高的六成。列表放进 `ScrollView` 并加 `.scrollBounceBehavior(.basedOnSize)`，框就是这个高度：选项少时卡片贴着选项，超过上限才滚动。行的 `minHeight: 56` 改成固定 `height`，让算式和布局用同一个数。链式面板的第二层按自己那一层的行数算。

「输入」页语言行的选项面板，打勾的选项和行上显示的方案一致，都由 `ChineseInputScheme.represented(among:enabled:current:)` 决定：键盘当前方案属于该语言时用它，否则用该语言第一个已启用的方案。以前面板只在键盘当前方案属于该语言时才打勾。真机上键盘在全拼时，双拼行写着「小鹤」，打开面板却一个勾都没有；五笔也一样。原因不在面板组件（`DesignOption.contains`、链式第二层都没问题），而在设置页传给面板的 `selected`。

常规宽度的弹出列表（`DesignOptionPopover`）用的是 `List`，行高跟着动态字体变，算不准。iOS 18 起每一层列表用 `onScrollGeometryChange` 量出「内容高度 + 上下留白」（推入第二层时留白里含导航栏），弹出框按正在显示的那一层定高，上限 480；从第二层退回时用第一层上次量到的值。量到之前，以及 iOS 17 上，沿用原来的估算 `行数 × 52 + (有标题 ? 96 : 40)`。

## 第一次修复为什么丢了

10-21 的修复写在 `fix/ios-voice-session-page` 分支上，共四个提交：

- `c30be602bf`：语音与 AI 服务页的卡片样式
- `92a9713182`：语音只保留长按空格这一个入口
- `9b557fb2a2`：小弹窗改成半屏起
- `911fd63834`：弹窗贴合内容

#9106 在 14:21 squash 合并时只带走了分支上的第一个提交 `ea7ff0cc90`。这四个提交是在合并后的 14:24 到 14:43 推上去的，推到了已经合并的分支上，从没有进 develop，PR 页面上也看不出来。这次把 `9b557fb2a2` 和 `911fd63834` 摘到新分支上，再在其上修改；另外两个由别的 PR 恢复。

10-21 那版只在真机上编译安装过，没有逐个打开弹窗看高度。所以上面三条规则一条也没发现，`NavigationView` 里的十个弹窗即使合进去也不会生效。

## Alternatives considered

- **统一 `[.medium, .large]`**：一行就能加，iOS 17 也一样。但一两个输入框的弹窗停在半屏仍然空着半截，内容多一点的（关于统计）又被截在半屏要滚动。登录弹窗当初正是为此改成按内容量高度的。
- **把这些表单改成 `VStack` 再量高度**：iOS 17 也能贴合，但要逐个重写表单，丢掉 `Form` 的分组、键盘避让和读屏行为，改动面远大于问题本身。
- **把修饰器放进 App 的 DesignKit**：命名与 App 其他设计组件一致，但共用的 `CloudDictionaryEditor` 用不到它，只能在调用处从外层加，而读滚动尺寸的修饰器要加在滚动视图本身上。
- **键盘弹起时照旧保留 `.large`**：用户随时能往上拉，系统的行为也不用干预。但最常用的添加常用语一打开就聚焦输入框，结果仍是整屏，维护者报的正是这种情况。键盘在时用户也很少需要整屏，内容比可用空间高时系统照样把弹窗限在最大高度。
- **选项面板用 `.fixedSize(vertical:)` 或量 `GeometryReader` 定高**：不用写死行高。但行高本来就是写死的（字号固定、单行），算出来的高度和布局同源，有单测守着；量高度要多一次布局，面板入场动画时还会跳一下。

## Consequences

- 小弹窗的高度跟着内容走：内容变化时（比如出现一行错误、换了导入方式）跟着调整，用户拉到整屏后不再自动收回。
- 弹窗刚出现的一瞬间以半屏打开，量到内容高度后再落到贴合的高度，会有一次很短的调整。iOS 17 仍是半屏起。
- 键盘弹起、弹窗不在整屏时拉不到整屏，收起键盘后可以。
- 选项面板的行高写死在 `DesignOptionSheetMetrics` 里；以后要让选项跟随动态字体，或者允许换行，就得改成量高度，单测 `DesignOptionSheetMetricsTests` 会提醒这个约束。
- 发布皮肤、保存我的皮肤沿用 `NavigationStack` 默认的大标题，比用行内标题的弹窗高出约 50pt；这次没改标题样式。

## Verification

真机 iPhone XR（iOS 18.7）Debug 签名构建，逐个打开、截图，用竖屏点数量高度（截图在 `/tmp/ios-sheets/`，没有入库）：

- 选项面板。中文键盘：卡片 533pt → 271pt（标题 45 + 4 行 226）。双拼方案（6 项）：553 → 403。识别语言、应用主题、云剪贴板同步方式同样贴合。九键滑动输入数字（3 项）：213。
- 常规宽度弹出列表：XR 横屏是常规宽度，打开中文键盘弹出列表，高度贴合标题和 4 行。
- 添加常用语：整屏 → 246pt。键盘弹起时贴在键盘上沿，往上拉能到整屏。
- 自建中继：整屏 → 约 431pt。
- 举报约 706pt，发布插件约 254pt，保存我的皮肤约 267pt。
- 个人词库：添加和编辑词条约 578pt，导入约 496pt。
- 关于统计：半屏截断 → 约 491pt，全文可见。
- 选择服务商、发布皮肤、发布词库、两个 WebDAV 填写页：内容本来就接近整屏，贴合后仍接近整屏。

验证码登录、精简录音页、云词库编辑、登录后发布、云剪贴板添加内容这几个在当时的账号状态下打不开，或者一打开就会开始录音，没有在真机上看；它们和已经看过的弹窗用同一套修饰器和结构。

此外，`DesignOptionSheetMetricsTests` 覆盖高度算式，`platforms/macos/scripts/typecheck_backend_swift.sh` 通过。
