# Agent Note: 在水杉应用里搜索本机剪贴板历史

Status: implemented

## Problem

#5973：本机剪贴板历史最多 50 条，用户只记得某段复制内容里的一个词时，只能在键盘剪贴板面板里一条条翻，或者逐条粘贴出来找。面板（`ImePanels.renderClipboardHistory`）只按共享存储的顺序列出全部条目，没有查询，也没有过滤；面板盖满整个键区，输入法里也没有任何「把按键输出改写到键盘内部文本框」的机制，所以现状下连输入查询词的地方都没有。

## Decision

- **搜索放在水杉应用里的一页，不在键盘面板里放查询框**（产品拍板）。新页面 `ClipboardSearchPage`（`PageId.CLIPBOARD_SEARCH`，标题「剪贴板历史」），读键盘写的同一份共享存储（`ClipboardHistoryStore(getFilesDir())`），和面板里看到的是同一份历史。入口与页面组织复用 #5971 的编辑页：键盘本机分段顶行在「清空」左边加「搜索」（只在历史开启、读出了至少一条时出现），由 `MSIMEInputService.openClipboardSearch` 走 `openHostPage(ClipboardSearchPolicy.SEARCH_PAGE)` 深链打开。这一页不需要参数，所以带了关键词，设置首页搜「剪贴板历史」「搜索剪贴板」也能进来。
- **筛选是不依赖 Android 的纯策略** `ClipboardSearchPolicy`：查询去掉首尾空白，和每条文字一起按 `Locale.ROOT` 转小写后做子串匹配；结果保留共享存储给的顺序（置顶在前，然后按时间），不按匹配程度重排；空查询返回全部；无结果时显示「没有包含“xx”的记录」。页面只在查询变化时重画结果卡片，搜索框本身不重建，焦点和输入法的组字不受影响。查询放在 ViewModel 里，旋转、换深浅模式后不丢。
- **结果项的操作复用已有的**：点按一条经 `ClipboardActions.copyText` 复制回系统剪贴板；行尾「编辑」按 #5971 的方式把 `ClipboardHistoryPolicy.editKey` 交给 `ClipboardEditPage`（这次是应用内 `SettingsNavigator.open`），「删除」走 `ClipboardHistoryStore.remove`，和面板里的左滑删除一样不再确认。共享存储的操作都在 `HostTask` 的工作线程上。
- **从键盘来的和从应用里来的分开收尾**，判据是深链参数里宿主自己加的 `HostDeepLink.ARG_EXTERNAL`（应用内导航不带它）：
  - 搜索页从键盘打开时，复制之后弹出这一页并 `moveTaskToBack`，回到原来的应用接着粘贴；从应用里打开时留在页面上。
  - 编辑页的 `leave()` 改成同样的判断：从键盘打开时照旧回到原来的应用；从搜索页打开时只弹回搜索页，搜索页在 `onBecameVisible` 里重读。
- **打开搜索页之前 `forgetCurrentClip()`**，理由和编辑相同：用户可能在那一页把系统剪贴板里当前那一条删掉或改掉，已处理身份存在 `:ime` 进程的 SharedPreferences 里，应用进程写不了；不记的话回来一打开面板，补读又把它记回来。
- 只搜本机历史。云端分段不加搜索：云剪贴板有自己的页面（`CloudClipboardPage`），服务端搜索还要处理网络延迟和代次。`check-host.sh` 守住：「搜索」在面板顶行、走 `openHostPage(ClipboardSearchPolicy.SEARCH_PAGE)`、先 `forgetCurrentClip`、不在输入法里建对话框或文本框，页面名和 `PageId` 一致。

## Alternatives considered

- **在键盘面板里直接搜**（顶行换成查询框，键区露出来，按键输出临时改写到内部 InputConnection） — 不离开当前应用，是 issue 的原话，也是 Gboard 搜表情的做法；但要把 `MSIMEInputService.connection` 临时换掉，引擎上屏、`selectionEcho`、配对标点栈、首字母大写读上文、打字统计和学词都默认它是宿主编辑器，每一处都要确认不会误删宿主文字或把查询词学进词库，改动面和风险都是 L 级。产品选了应用内页面，和 iOS #580、#5971 的编辑页一致。
- **云端分段一起做服务端搜索**（`BackendAccount.clipboard(query)` 已经支持 `?q=`） — 两种来源一个入口；但云剪贴板已有带列表的应用内页面，加搜索要处理防抖和代次丢弃，产品决定这次只做本机历史。
- **复制后一律留在页面上** — 行为统一，不会意外离开应用；但从键盘过来的用户是要找一条去粘贴，按返回只回到设置首页，回原来的应用还要去多任务里切，和编辑页「保存后回到原来的应用」不一致。
- **按匹配程度排序**（完全匹配、前缀在前） — 词多时更快找到；但本机历史最多 50 条，用户按记得的位置（置顶、最近）找，重排会让置顶失去意义。

## Consequences

- **收益**：只记得一个词也能在本机历史里找到那一条，复制、编辑、删除都在同一页；没有动输入法的上屏链路，键盘这边只多一个按钮和一个打开页面的方法；筛选规则有 JVM 冒烟。
- **代价**：搜索要离开当前应用再回来，回来靠 `moveTaskToBack`，不同 ROM 的任务切换表现可能不同，和编辑页一样。打开搜索页时就把系统剪贴板当前那一条记为已处理：如果那一条之前因为隐私模式从没记进历史，之后打开面板也不会补读它。删除只挡得住打开搜索页那一刻系统剪贴板里的那一条：同样的文字在键盘进程不在时被重新复制过一次（身份不同），在页面里删掉后，下次打开面板补读还会把它记回来。从页面复制的那一条是一次新的复制，键盘会照常把它记到最前。
- 以后要在键盘里直接搜，需要先做「内部编辑器」那套 InputConnection 改写；这一页的策略类可以直接复用。

## Verification

`platforms/android/tests/clipboard/ClipboardSearchPolicySmoke.java` 覆盖空查询和空白查询显示全部、大小写、中文子串、首尾空白、查询中间的空格、标点、置顶保持在前、无结果文案，以及系统语言是土耳其语时仍按 `Locale.ROOT` 匹配；`bash platforms/android/check-host.sh` 编译键盘代码、跑全部 JVM 冒烟（含 `SettingsSearchIndexSmoke` 对新 `PageId` 项的解析）并检查上面的守卫。`ClipboardSearchPage` 和 `ClipboardEditPage` 只在 Gradle 构建里编译。
