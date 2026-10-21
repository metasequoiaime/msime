# Agent Note: Windows 候选窗绘制皮肤的卷轴展开（expand_panel）

Status: implemented

## Problem

msime-windows `c0a9d371` 的皮肤可以写 `[candidate_window] expand_panel = true`（内置的 `wechat_scroll` 就是这样），把横排候选窗换成微信输入法式的卷轴展开：候选行右端一个「∨」按钮，点开后首行下面是后续候选的等宽网格，滚轮上下滚动。共享 catalog 早已解析出 `expandPanel` / `expandRows`，[第一步](2026-10-19-windows-candidate-skin-layout-keys.md)把版式、外框、阴影、翻页箭头和收紧宽度画了出来，卷轴展开仍只解析不绘制。难点不在画法，而在数据：本宿主的候选窗只拿到 Engine 当前这一页（view 最多 9 个候选），网格要的是这一页之后的候选，点网格里的候选还得按不在当前页上的下标上屏。约束是不写 `expand_panel` 的皮肤和以前完全一样。

## Decision

### 行为（照 skins/README.md「卷轴展开」与 candidate_presenter.cpp、candidate_skin_scene.cpp）

- 只在横排生效。`candidate_skin_metrics` 只在横排时打开 `expand_panel`，打开时清掉 `page_arrows`；首行也不画本宿主自己的「n / m」翻页（来源没有首行翻页，开了卷轴展开的候选窗上没有任何翻页控件）。竖排照常是列表和翻页。
- 候选右侧一窄列放按钮，和第一步的列表翻页箭头共用 `candidate_list_arrow_column`：按钮 1.1 倍字号取整见方，左右外边距 2 与 1，竖直居中在第一行上，序号色折线，收起「∨」、展开「∧」，悬停铺 `hover`，按下铺 `selected`（透明时仍是 `hover`）。按钮固定在右上角，不随内容滚动。
- 展开后首行仍是 Engine 当前那一页（带序号，数字键、空格、方向键照常由 Engine 处理），下面隔一个 `item_gap` 是网格：没有序号和选中态，文字和首行候选文字对齐，字体、字号、行高跟首行；格宽按 `CandidateList::Measure` 的等宽网格（内容宽度放得下几格 2.6 倍字号就分几列，不超过每页候选数，宽的候选占几格，放不下就换行）。一屏是首行加 `expand_rows`（2-8，缺省 4）行网格；内容更高时右侧让出 10 DIP，画 4 DIP 宽的序号色滑块（不透明度 0.45，悬停或拖动时 0.85，不画轨道，可拖动，点轨道跳转）。网格按卡片能有的最大宽度（工作区的一半）去排，和来源按 `maxW` 量网格一样，所以有一整行网格时卡片就是那么宽。
- 滚轮：展开时首行与网格一起滚，一格滚一行（行高加行距），高精度滚轮按比例，到顶或到底停住，不看「鼠标滚轮翻页」开关；收起时按开关翻页，与别的皮肤相同（`candidate_wheel_route`）。
- 网格悬停铺 `hover`，在同一个候选上按下又松开就上屏。网格一次排 300 个候选（来源的 `kExpandedCandidateLimit`），滚到离底部一行半以内再排一批。
- 继续输入时保持展开；网格第一项（下标和文字）换了，也就是翻了页或接着打字，就回到顶部、只排第一批，只是追加了候选则保持滚动位置（来源的 `sameContent`）。候选窗关闭（上屏或取消）、焦点换到别的客户端、改成竖排、换成不展开的皮肤时收起；释义页和只能用键盘选的列表上不显示按钮，照常画列表和翻页。
- 读屏：按钮是「展开候选」/「收起候选」，可执行；网格里看得见的候选是列表项（只有文字），可执行即上屏；首行整行滚出滚动区的候选不报。悬停提示只登记首行看得见的那一段。

几何、滚动偏移的夹取、点击、滑块、批次、回到顶部和收起的规则都在不依赖 Win32 的 `platforms/windows/src/candidate/CandidateExpandPanel.h`，`CandidateWindow` 只负责量字、画和转发鼠标。

### 后续候选从哪来、网格里的候选怎么上屏

用已有的两个 C ABI，不加新协议：`msime_client_all_candidates` 一次复制这一代的全部候选（全局下标），`msime_client_select_any_candidate` 按这一代的全局下标选。macOS 的展开网格（[2026-10-12-macos-expanded-candidate-grid.md](2026-10-12-macos-expanded-candidate-grid.md)）、Android 与 iOS 的展开面板用的也是它们。

- 候选窗点开「∨」时经 `WindowsServer::set_candidate_expansion` 告诉 Server，`CandidateMailbox` 记下这个租约。之后每当这个租约发布了新的一代（按键送达、点选送达、翻页、候选动作、云候选、AI 候选、静止重排），`SessionController::refresh_expansion` 在输入队列上排一个任务（同一时刻最多一个），`FocusedSession::expanded_candidates` 在没有等待确认的回复、不是释义页、列表能用鼠标选时调 `msime_client_all_candidates`。只补上释义的同一代不用再取。
- `all_candidates` 会让引擎放出扣着的尾部，这时代次前进（[完整候选 release 代次](../bug-fix/2026-10-11-full-list-generation-release-build.md)）。任务随即重新读 view，经 `CandidateMailbox::action` 发布并重新请求释义，和翻页一样；不这样做，这一页的身份停在旧的一代，首行的点选都会被拒绝（Android 踩过同一个坑，[Android 展开候选后同步视图](../bug-fix/2026-10-15-android-expanded-candidate-view.md)）。
- 列表经 `expanded_candidates_from_snapshot` 解析（占位行、空的和超过 4096 字节的跳过），繁体输出时在锁外换成繁体，存进 `CandidateMailbox`，只在快照和它同一会话、同一代、同一种简繁时挂到 `CandidatePresentation::expanded` 上（共享不可变的一份，候选窗每轮读快照不复制）。切换简繁（Ctrl+Shift+F）只换投影、不换代次，所以简繁也是「要不要再取」的条件之一，否则网格停在旧字形上、点哪个都因文字对不上被拒绝。换代后到重新取到之前，那一页没有网格，这一帧沿用上一帧展开区的高度，卡片不先缩回一行再长回来；列表到了而这一代后面一个候选也没有时按实际内容排，不留空白（`candidate_expand_held_viewport`）。
- 开没开以 Server 这一侧为准：快照带着 `CandidatePresentation::expansion_open`（`CandidateMailbox` 是否为这个租约开着展开）。候选窗关闭时 Server 在 `HideCandidateWnd` 上先清掉，而候选窗按 50 毫秒的轮询才看到，有退出动画时 `hide_now` 还会更晚；上屏后马上接着打字时候选窗可能一直没收起。所以 `reposition` 每一帧按 `candidate_expand_stale` 对照：Server 已经收起、换了客户端或这一页不能展开，候选窗就收起，不会出现一个开着、一个关着、「∧」下面永远空着的情况。
- 点网格里的候选：`CandidateClick` 带上它在完整列表里的下标和画出来的文字（`expanded_text`），走同一个 `request_selection`。Server 先在画出来的那一帧挂着的列表里找同一下标、同一文字，再在输入队列上由 `ReplyComposer::select_candidate` 对这一代再取一次完整列表核对，然后经 `ServerSession::select_any` 调 `msime_client_select_any_candidate`；指针报告、云候选整段上屏、送达确认和之后的组字续接都与点首行相同。代次或文字对不上就当作过期的点击拒绝。

### 每批取多少

一次取整份：`all_candidates` 没有分批参数，而它放出尾部之后这一代就没有更多了。「滚到底再加载一批」在这里是宿主一侧的：网格每次多排 300 个，免得几百个候选每次重画都要排版；文字宽度按文字缓存，DirectWrite 只量一次。缓存长过上限（4096 与这一批两倍中较大的那个）时只留这一批的，不整个清空，否则排过的不同文字一多，每一帧都要把这一批重新量一遍（`candidate_expand_trim_widths`）。

### TIP 续接网格里的候选

点网格里的候选若没有上屏文字、组字接着往下走（`ui_continued_selection`，网址、顶级域名补全这类候选会排进网格），TIP 要在自己的会话里重放这次选择（`tsf/HostCandidateContinuation.h` 的 `ContinueHostCandidate`）。它原来只在本会话的当前页上按文字找，网格里的候选不在那一页上，找不到时这个连接被标脏重置，组字丢掉。现在当前页上没有时，TIP 到本会话这一代的完整候选列表里按文字找唯一的一个，按全局下标选（`EngineSessionAdapter::all_candidates` / `select_any`，就是 `msime_client_all_candidates` 与 `msime_client_select_any_candidate`；TIP 本来就直接持有这个会话的句柄）；取列表时引擎可能放出尾部、代次前进，所以用列表自己的代次。页上有的照旧按页上的选，页上重复、列表里重复或找不到时照旧拒绝，前后组字状态的核对不变。

网页 SDK 的同类实现见[网页卷轴展开与版本化点选](2026-10-20-web-candidate-expand-panel.md)，它在 Worker 内发布完整排序列表，不改变这里的 Windows 进程边界。

## Alternatives considered

- **照 msime-windows 的 `MoveToExpandedCandidate`，点网格时先翻到那一页再按页内序号选** —— 最强的理由是只用 `msime_client_select` 一种选择接口，Engine 的当前页也停在被选的候选上。不采用：要先发若干次 `MSIME_NEXT_PAGE`，每次都推进代次、进生命周期和脚本插件的钩子，而 `msime_client_select_any_candidate` 就是为这种「不在当前页上的候选」准备的，按同一代的身份选，过期的一代由运行时拒绝。
- **按显示文字重新定位过期的点击** —— 来源在点下去之后候选被异步结果插队时，按文字找同字里最靠前的那个上屏。不采用：本宿主首行的点选一律按会话、代次和下标核对，对不上就拒绝；网格沿用同一条规则，用户在新一帧里再点一次，不会上屏一个用户没看到的位置上的词。
- **在按键回复里同步带上完整列表**（来源的做法：面板开着时工作线程发布每一页都带上后续候选） —— 最强的理由是网格和首行永远同一帧到达，没有「这一代还没有网格」的一瞬间。不采用：`SessionPump::Presentation::delivered` 的契约是不调用 Engine、只复制有界数据，而 `all_candidates` 可能推进代次，在送达回调里改代次会让刚送出的回复和候选窗对不上；改成送达之后另排一个输入队列任务，代价是偶尔一帧没有网格，用沿用上一帧高度把它的观感压到只是网格空一下。
- **续接状态下不让点网格**（网址、顶级域名补全或整句改字时 Server 拒绝 `expanded_text` 的点击，候选窗也不画网格和按钮） —— 最强的理由是不碰注入每个进程的 TIP。不采用：TIP 已经直接持有自己会话的句柄，补两个和 `select` 同样形状的薄封装就能在完整列表里找到那个候选；拒绝的话，网址补全时网格里的候选看得见却点不了。
- **网格只取一页（在 Server 里截到 300 个再交给候选窗）** —— 跨线程复制的数据有上界。不采用：来源也只是显示前 300 个，再多就看不到；这里整份交过去、按批排出，滚到底能看到全部，复制的是共享的一份。
- **在共享 catalog 里让 `base = "wechat_scroll"` 的包默认打开 `expand_panel`** —— 来源写 `base = "wechat_scroll"` 的外部皮肤就沿用卷轴展开。不采用：第一步已经决定 catalog 不按外观补版式（会改动所有宿主共享的扫描结果），这里沿用同一个决定，包要卷轴展开就自己写 `expand_panel = true`。

## Consequences

收益：为 msime-windows 写的卷轴展开皮肤在 Windows 横排候选窗上得到按钮、网格、细滚动条、滚轮滚动和网格点选；不写 `expand_panel` 的皮肤和竖排的卡片逐位不变（`windows-candidate-card-size` 原样通过，`windows-candidate-expand-panel` 断言关掉或竖排时没有按钮那一列、首行高度和右侧留白不变）。Engine 和 C ABI 没有改动。

代价：
- 没写 `expand_panel` 但 `base = "wechat_scroll"` 的包在这里没有卷轴展开，见上。
- 换代后到后续候选取到之前有一帧没有网格（卡片高度不变）；开着展开时每次换代多一次 `all_candidates` 和 JSON 复制，放出尾部时还多一次读 view 和释义请求。
- TIP 的续接多了一条退路（到完整列表里找），也就多一次 `all_candidates`；只在当前页上找不到时才走。
- 网格里的候选没有右键菜单（置顶、删除、固定排位只认当前页），也没有悬停提示，与来源相同。
- 卷轴展开的皮肤上没有翻页按钮：收起时用键盘或（开关打开时）滚轮翻页，展开时只能用键盘翻页，与来源相同；原生设置窗口的皮肤预览还不画「∨」按钮。
- 继续输入时的展开、回到顶部以及按钮、滑块、网格的观感都没有在真实 Windows 上看过。

验证：新加的 `windows-candidate-expand-panel`（按钮列、网格列数与跨格、换行、一屏高度、滚动条与重排、高度上限与沿用、卡片尺寸、点击与滚出、滑块与拖动、状态同步与回到顶部、滚轮、批次、收起判断、沿用高度的条件、宽度缓存的修剪、滚轮路由）、扩充的 `windows-candidate-skin-layout`（`expandPanel` / `expandRows` 读取与越界）、`windows-candidate-accessibility`（按钮、网格项、滚出的首行）和 `candidate_mailbox.cpp`（列表只跟着同一代和同一种简繁、换代或切换简繁要再取、快照带着 Server 是否开着、收起与隐藏和断开时清掉、解析拒绝身份不符的列表）、新加的 `windows-host-candidate-continuation`（当前页上没有时到完整列表里找并用列表的代次选、页上有的照旧、重复或找不到或读不到列表时拒绝），以及原有的 `windows-candidate-card-size`、`-skin`、`-palette`、`-layout-reload`、`-shadow`、`-window-style`、`-skin-reload`、`-theme-reload`、`-wheel`、`windows-skin-resource-revision`，在 Mac Studio 上用 `c++ -std=c++17 -Wall -Wextra -Werror` 编译并通过（无障碍与 mailbox 两项在 Studio 上用一个原样返回的繁简转换替身链接）。`CandidateWindow.cpp`、`SessionController.cpp`、`ServerSession.cpp`、`ReplyComposer.cpp`、`FocusedSession.cpp`、`InputQueue.cpp`、`WindowsServer.cpp`、`server_main.cpp`、`tsf/EngineSessionAdapter.cpp` 和 `tsf/Key/KeyHandler.cpp` 用本机 `x86_64-w64-mingw32-g++ -fsyntax-only -Wall -Wextra` 检查无错误，前几个也无警告；`KeyHandler.cpp` 只有它原有的几条未使用变量警告，这次改动的头文件不引出新的。链接、完整交叉构建和 Wine 套件交给 PR 上的 Windows CI。
