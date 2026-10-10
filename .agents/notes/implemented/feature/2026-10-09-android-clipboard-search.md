# Agent Note: 在水杉应用里搜索本机剪贴板历史

Status: implemented

## Problem

#5973：本机剪贴板历史最多 50 条，用户只记得某段复制内容里的一个词时，只能在键盘剪贴板面板里一条条翻，或者逐条粘贴出来找。面板（`ImePanels.renderClipboardHistory`）只按共享存储的顺序列出全部条目，没有查询，也没有过滤；面板盖满整个键区，输入法里也没有任何「把按键输出改写到键盘内部文本框」的机制，所以现状下连输入查询词的地方都没有。

## Decision

- **搜索放在水杉应用里的一页，不在键盘面板里放查询框**（产品拍板）。新页面 `ClipboardSearchPage`（`PageId.CLIPBOARD_SEARCH`，标题「剪贴板历史」），读键盘写的同一份共享存储（`ClipboardHistoryStore(getFilesDir())`），和面板里看到的是同一份历史。入口与页面组织复用 #5971 的编辑页：键盘本机分段顶行在「清空」左边加「搜索」（只在历史开启、读出了至少一条时出现），由 `MSIMEInputService.openClipboardSearch` 走 `openHostPage(ClipboardSearchPolicy.SEARCH_PAGE, args)` 深链打开。这一页不需要参数，所以带了关键词，设置首页搜「剪贴板历史」「搜索剪贴板」也能进来。
- **筛选是不依赖 Android 的纯策略** `ClipboardSearchPolicy`：查询去掉首尾空白，和每条文字一起按 `Locale.ROOT` 转小写后做子串匹配；结果保留共享存储给的顺序（置顶在前，然后按时间），不按匹配程度重排；空查询返回全部；无结果时显示「没有包含“xx”的记录」。页面只在查询变化时重画结果卡片，搜索框本身不重建，焦点和输入法的组字不受影响。查询放在 ViewModel 里，旋转、换深浅模式后不丢。
- **结果项的操作复用已有的**：点按一条经 `ClipboardActions.copyText` 复制回系统剪贴板；行尾「编辑」按 #5971 的方式把 `ClipboardHistoryPolicy.editKey` 交给 `ClipboardEditPage`（这次是应用内 `SettingsNavigator.open`），「删除」走 `ClipboardHistoryStore.remove`，和面板里的左滑删除一样不再确认。共享存储的操作都在 `HostTask` 的工作线程上。
- **编辑用的键按共享存储里现在的那一条算**（`ClipboardSearchPolicy.currentEditKey`），不按列表上显示的。在应用里点按一条复制后页面留在原地，而键盘的复制监听（`clipboardWatcher`，整个输入法服务存活期间都注册着）会把它重新记一遍，`push_validated` 给它换上新的时间戳；列表上的旧时间戳算出的键，`ClipboardEditPage.find` 按键精确匹配就找不到，会说这一条已经不在了。所以点「编辑」时先在工作线程上重读存储，按文字找回那一条（共享存储按文字认条目，同一段文字只有一条），用它现在的时间戳算键；读不出或已经不在时退回列表上的键，由编辑页自己说读取失败或已经不在。编辑页的精确匹配不放宽。
- **剪贴板历史开关关着时不列记录，并在这里清空**。设置里关掉 `clipboard_history` 只写偏好，清空要等键盘下一次实时读偏好（`ClipboardHistoryRetentionPolicy`）；水杉不是当前输入法、或关掉之后还没弹出过键盘时，历史还在存储里。这一页不经键盘也能打开（设置首页搜索、深链），是应用里第一个列出全部本机历史的地方，不看开关就会把用户以为已经清掉的记录全列出来。`reload()` 先经 `KeyboardSheets.preferences` 读共享偏好，关着时显示「剪贴板历史未开启」，并按 `clearsHistory(LIVE, false)` 同一条规则 `clearQuietly()`：这一页读到的就是实时偏好。
- **在别的应用里打开的和在水杉里打开的分开收尾**，判据是 `ClipboardEditPage.returnsToCaller`：深链参数里宿主自己加的 `HostDeepLink.ARG_EXTERNAL`（应用内导航不带它），并且键盘带了 `ClipboardHistoryPolicy.RETURN_TO_CALLER_ARG`。键盘只在当前输入框的包名不是水杉自己时带它（`MSIMEInputService.putReturnToCaller`，`ClipboardHistoryPolicy.returnsToCaller`）：
  - 搜索页从别的应用的键盘打开时，复制之后弹出这一页并 `moveTaskToBack`，回到原来的应用接着粘贴；从应用里打开、或键盘在水杉自己的输入框里打开时留在页面上。
  - 编辑页的 `leave()` 用同一个判断：从别的应用的键盘打开时照旧回到原来的应用；从搜索页打开、或在水杉自己的输入框里打开时只弹回上一页，搜索页在 `onBecameVisible` 里重读。
- **打开搜索页之前 `forgetCurrentClip()`**，理由和编辑相同：用户可能在那一页把系统剪贴板里当前那一条删掉或改掉，已处理身份存在 `:ime` 进程的 SharedPreferences 里，应用进程写不了；不记的话回来一打开面板，补读又把它记回来。
- 只搜本机历史。云端分段不加搜索：云剪贴板有自己的页面（`CloudClipboardPage`），服务端搜索还要处理网络延迟和代次。`check-host.sh` 守住：「搜索」在面板顶行、走 `openHostPage(ClipboardSearchPolicy.SEARCH_PAGE, args)` 并先 `putReturnToCaller(args)`、先 `forgetCurrentClip`、不在输入法里建对话框或文本框，页面名和 `PageId` 一致。

## Alternatives considered

- **在键盘面板里直接搜**（顶行换成查询框，键区露出来，按键输出临时改写到内部 InputConnection） — 不离开当前应用，是 issue 的原话，也是 Gboard 搜表情的做法；但要把 `MSIMEInputService.connection` 临时换掉，引擎上屏、`selectionEcho`、配对标点栈、首字母大写读上文、打字统计和学词都默认它是宿主编辑器，每一处都要确认不会误删宿主文字或把查询词学进词库，改动面和风险都是 L 级。产品选了应用内页面，和 iOS #580、#5971 的编辑页一致。
- **云端分段一起做服务端搜索**（`BackendAccount.clipboard(query)` 已经支持 `?q=`） — 两种来源一个入口；但云剪贴板已有带列表的应用内页面，加搜索要处理防抖和代次丢弃，产品决定这次只做本机历史。
- **只看 `ARG_EXTERNAL` 判断是不是从键盘来的** — 不用键盘再传参数；但这个标记的意思是「经 HomeActivity 的深链进来」，用户正在水杉自己的输入框（设置首页搜索框、这一页的查询框）里打字时，键盘的「搜索」「编辑」也走这条路，复制或保存后 `moveTaskToBack` 会把水杉自己送到后台，落到它后面的桌面。由键盘按当前输入框的包名明说要不要回去。
- **页面显示期间监听剪贴板变化即时重读** — 列表能马上反映键盘的重新记录；但应用进程收到变化通知时，`:ime` 进程不一定已经写完共享存储，重读会和它赛跑，只能靠延时补救。编辑键的正确性改由点「编辑」时重读保证，列表顺序和「N 分钟前」等这一页下次显示时重读。
- **复制后一律留在页面上** — 行为统一，不会意外离开应用；但从键盘过来的用户是要找一条去粘贴，按返回只回到设置首页，回原来的应用还要去多任务里切，和编辑页「保存后回到原来的应用」不一致。
- **按匹配程度排序**（完全匹配、前缀在前） — 词多时更快找到；但本机历史最多 50 条，用户按记得的位置（置顶、最近）找，重排会让置顶失去意义。

## Consequences

- **收益**：只记得一个词也能在本机历史里找到那一条，复制、编辑、删除都在同一页；没有动输入法的上屏链路，键盘这边只多一个按钮和一个打开页面的方法；筛选规则有 JVM 冒烟。
- **代价**：搜索要离开当前应用再回来，回来靠 `moveTaskToBack`，不同 ROM 的任务切换表现可能不同，和编辑页一样。打开搜索页时就把系统剪贴板当前那一条记为已处理：如果那一条之前因为隐私模式从没记进历史，之后打开面板也不会补读它。删除只挡得住打开搜索页那一刻系统剪贴板里的那一条：同样的文字在键盘进程不在时被重新复制过一次（身份不同），在页面里删掉后，下次打开面板补读还会把它记回来。从页面复制的那一条是一次新的复制，键盘会照常把它记到最前，但页面在原地不重读：列表上那一条的位置和「N 分钟前」要等这一页下次显示（从编辑页回来、应用回到前台）才更新，编辑、删除、复制都按文字或现在的键操作，不受影响。键盘在水杉自己的输入框里打开这一页时，`HomeActivity.openDeepLink` 照常清空了原来的页面栈，这一点和键盘的其他深链（设置、词库）相同。
- 以后要在键盘里直接搜，需要先做「内部编辑器」那套 InputConnection 改写；这一页的策略类可以直接复用。

## Verification

`platforms/android/tests/clipboard/ClipboardSearchPolicySmoke.java` 覆盖空查询和空白查询显示全部、大小写、中文子串、首尾空白、查询中间的空格、标点、置顶保持在前、无结果文案，系统语言是土耳其语时仍按 `Locale.ROOT` 匹配，以及重新记录换了时间戳后编辑键跟着现在的那一条（已删掉、只是前缀时没有键）；`tests/dictionary/ClipboardHistoryPolicySmoke.java` 覆盖 `returnsToCaller` 三种包名情况和参数名能过深链过滤；`bash platforms/android/check-host.sh` 编译键盘代码、跑全部 JVM 冒烟（含 `SettingsSearchIndexSmoke` 对新 `PageId` 项的解析）并检查上面的守卫。`ClipboardSearchPage` 和 `ClipboardEditPage` 只在 Gradle 构建里编译（`build-apk.sh` 过了）。在 API 35 的专用模拟器（arm64，无窗口）上走过：开着剪贴板历史复制几条合成文字后，本机分段顶行出现「搜索」；点了打开应用里的「剪贴板历史」页，搜索框自动聚焦、键盘弹出；输入 `ALPHA` 只剩两条含 `Alpha` 的记录（不区分大小写，顺序不变），再输入成 `ALPHAzz` 显示「没有包含“ALPHAzz”的记录」；行尾删除后那一条从列表消失；行尾编辑打开编辑页，改字保存后弹回剪贴板历史页（应用仍在前台），那一条原地显示新文字；点一条后回到原来的编辑器，粘贴出的正是那一条。review 之后的修正也在同一台模拟器上走过：经深链从应用里打开（不带返回参数）点按一条复制后页面留在前台，再点那一条的「编辑」，编辑页显示出原文（键盘已把它重新记到最前，取消回来后列表里它是「刚刚」），说明编辑键跟上了新的时间戳；在水杉自己的设置首页搜索框里打开键盘剪贴板面板点「搜索」，复制一条后水杉仍在前台、停在这一页；在合成编辑器应用里点「搜索」，复制一条后回到那个编辑器，粘贴出的正是那一条；在「隐私」里关掉剪贴板历史后打开这一页显示「剪贴板历史未开启」，重新打开开关后显示「还没有记录」。从设置首页搜索结果点进来、置顶条目的显示、横屏、API 28 和真机没有验证。
