# Agent Note: Windows 设置侧栏搜索改成 macOS 的结果列表

Status: implemented

## Problem

macOS 设置（共享设置 UI 的桌面分支）的侧栏搜索是一个没有描边、放大镜在左的输入框，有查询时侧栏的页面列表整个换成结果列表：先列名字匹配的页面，再列标题匹配的设置行，每条下面一行写位置「页 › 标签 › 分组」，条数不设上限，查不到时写「没有匹配的设置」；上下键移动高亮、回车打开、Esc 清空，打开时切到所在标签、展开折叠区并把焦点交给控件。Windows 原生设置窗口用的是 WinUI 的 `AutoSuggestBox`：占位文字「搜索设置」，查询图标在右，带系统描边和底线；结果是下拉建议，最多 12 条，每条写成「标题 · 页名」，标题、说明和页名都参与匹配（输入一个页名会列出那一页的前 12 行），查不到时写「没有找到相关设置」，导航列表本身不变。用户要求以 macOS 为准迁移。

## Decision

- 搜索框换成普通的 `TextBox`，外面一层 32 DIP 高的 Grid（下边距 8，即 `sidebarSearch` 的 `mb-2`），左边 10 DIP 处叠一个 15 DIP 的次要色放大镜（照 macOS 的 16 × 16 SVG 用椭圆和圆头线画，不接收指针），输入框左内边距 33 给它让位。圆角 8、`BorderThickness` 0，底色浅色 rgba(0,0,0,.055)、深色 rgba(255,255,255,.07)，悬停不变，13 DIP 字，占位文字「搜索」。这些都经 `TextControl*` 模板资源覆盖：背景、前景、占位文字的普通 / PointerOver / Focused 三态，边框色在聚焦时是强调色，`TextControlBorderThemeThicknessFocused` 改成四边 2 DIP，即 macOS 聚焦时的强调色外框。自动化 ID 仍是 `settings-search`，读屏名仍是「搜索设置」。
- 结果列表是 `NavigationView.PaneCustomContent` 里的一个 ScrollViewer（`settings-search-results`），模板里排在页面列表之上；有查询时各 `NavigationViewItem` 收起、列表显示，查询清空时反过来。这一行的模板高度按内容定，所以 `fit_search_results` 按搜索框底部到导航视图底部的距离给列表定 `MaxHeight`（再留 16），导航视图 SizeChanged 时重算，列表自己滚动。
- 匹配照 `use-settings-shell-navigation.tsx`：查询 trim 后转小写；页面按侧栏名字匹配，也认共享 UI 的注册名（`page_search_names`：「屏幕键盘」对「键盘与手写」，「AI 辅助」「AI 助手」对 AI 页）；设置行只按 `record` 的标题匹配，说明不再进索引（`record` 的说明参数保留，调用处不动）。页面结果按侧栏顺序排在前面，设置行按索引顺序排在后面，不设上限；同一页同一位置的同名行只列一次。
- 位置：建索引时 `page_tab_bar` 记下这一页的标签名（`index_tabs_`），`in_tab` 换块时清空分组，`add_group` 记下组名（`group_index_`），`record` 拼成「页 › 标签 › 分组」写进 `SearchEntry::path`，缺哪一段就省掉。fold 的名字不进位置，折叠区仍由 `fold_holds_focus` 展开。
- 每条结果是一个按钮：内边距 12 / 8、圆角 8、条间 4，13 DIP 标题，设置行下面相隔 4 是 12 DIP、60% 不透明度的位置；悬停 10% 强调色，高亮 15%（ItemStatus「已选择」）。按钮不进 Tab 顺序，读屏名是标题，帮助文本是位置，自动化 ID 是 `settings-search-result-<序号>`。查不到时是 14 DIP、60% 不透明度、礼貌播报的「没有匹配的设置」：新建的文字块只设 LiveSetting 不会被念出来，进了可视树后自己发 LiveRegionChanged，上一次已经查不到时不再重复发。
- 键盘：搜索框的 PreviewKeyDown 处理上下键（循环移动高亮并滚进视野）、回车（打开高亮的那条）和 Esc（清空）；输入法组字期间（`TextCompositionStarted` 到 `TextCompositionEnded`）不接管。焦点始终在搜索框里，高亮换到哪条由搜索框的自动化对等体发一条通知（`RaiseNotificationEvent`，ImportantMostRecent），念标题和位置，代替 macOS 搜索框上的 `aria-activedescendant`；原来的 `AutoSuggestBox` 靠建议列表的选中让读屏念出，换成普通 `TextBox` 后不这样做就一声不响。回车时框里的文字和结果对应的查询不同（UI 自动化写值后立刻回车，TextChanged 还没到）就先重查。
- 打开结果照 macOS 的 `openResult`：先清空查询、换回页面列表；页面结果只切页；设置行写 `focus_row_`、把所在标签写进 `page_tabs_`，再切页或重画，`render_page_at` 滚到那一行并把键盘焦点交给控件。「自定义主题」落在组里唯一的折叠区上，焦点交给折叠区的标题按钮（`add_fold` 的 `search_title`），不是只滚到组。点击结果时打开动作放到下一轮调度，因为清空查询会卸下这个按钮。
- 换明暗重建外壳时照旧写回查询，结果列表随 TextChanged 重建。

## Historical audit

- [Windows 设置窗口参照 macOS 改版](2026-10-15-windows-ui-macos-layout.md)：部分重叠。它定下「独立的侧栏搜索」，本篇把这个搜索的样子和结果呈现换成 macOS 的；那篇验证段里「关闭仍聚焦在搜索框上的建议列表」描述的是当时的 `AutoSuggestBox`，属于历史验证记录。
- [页内标签框架](2026-10-20-windows-settings-page-tabs.md)：部分重叠。结果切到所在标签的机制（`SearchEntry::tab`、`page_tabs_`）沿用那篇，本篇只加标签名用于位置。
- [外壳配色与侧栏](2026-10-20-windows-settings-mac-shell.md)：相邻。侧栏的导航项资源由那篇管，本篇不改。
- [设置搜索定位与桌面内容归位](2026-10-19-settings-findability.md)、[设置搜索按键](../../implemented/bug-fix/2026-10-14-settings-search-ime-keys.md)：共享 UI 的同一决定和组字保护，本篇是在 Windows 原生窗口的落地。
- 检索 `rg --hidden --glob '!.agents/notes/archived/**' "侧栏搜索|设置搜索|搜索结果|AutoSuggestBox|search_index_|没有找到相关设置" .agents/notes/`，其余命中是 Android、移动端和字体输入框的搜索或下拉，与本篇无关。

## Alternatives considered

- **保留 `AutoSuggestBox`，只去掉右侧图标、覆盖描边**：差距清单最初的建议，改动最小。否决：结果仍是下拉建议，替换不了侧栏列表；而且它会自己处理上下键和提交，要和建议列表的选中、无结果占位那一套继续打交道（原来就要把无结果提示从「已选建议」里挡回去）。普通 `TextBox` 加自管的结果列表更直接。
- **结果放进 `NavigationView.MenuItems`**：每次输入清空页面项、换成结果项，滚动和焦点都由导航视图负责。否决：导航项的选中会触发 `SelectionChanged` 导航，要再区分页面和结果；查询清空时还要重建页面项并恢复选中；两行的结果也得改导航项的模板。
- **在根 Grid 上盖一层覆盖侧栏的结果面板**：高度天然是剩余空间，不用算 `MaxHeight`。否决：要知道搜索框在模板里的确切位置来对齐上边，同样要在运行时量；被盖住的页面项仍在读屏树里。
- **保留说明和页名参与匹配**：Windows 原来找得更宽。否决：没有条数上限之后，输入一个页名会列出那一页全部的行，说明里的常用词也会带出大量无关结果；macOS 只读显式行标题。

## Consequences

- **收益**：Windows 的侧栏搜索与 macOS 同一套外观和交互：结果就在侧栏里，能看到每条在哪一页、哪个标签、哪个分组，不再被 12 条截断，页面本身也能直接搜到。
- **代价**：只按标题匹配后，以前靠说明里的词能搜到的行现在搜不到，与 macOS 相同。结果没有上限，单字查询可能一次建几百个按钮，每次按键重建整个列表。
- **依赖模板**：`TextControlBorderThemeThicknessFocused` 和 PaneCustomContent 在页面列表之上的位置都取自 Windows App SDK 1.7 的模板；升级后键名或结构变了，聚焦外框会静默消失，或结果列表的位置、最高值不对，功能不受影响。
- **位置的分组是近似的**：`group_index_` 取这一行之前最近的 `add_group`，不经 `add_group` 画的组、或画在组后面但不属于组的条目会带上前一个组名。
- **重访信号**：macOS 的 `use-settings-shell-navigation.tsx`、`settings-navigation-search.tsx`、`settings-style.ts` 的 `sidebarSearch` 改了；或升级 Windows App SDK。

## Verification

- 本机没有 WinUI 3 / C++/WinRT 工具链，`main.cpp` 只能由 PR 的 `CI (platforms)`「Windows native settings」以 `/W4 /WX` 编译链接并运行 `tests/tools/settings_ui.ps1`。脚本新增一段：搜「剪贴板」时第一条是没有位置的页面结果「剪贴板」，「剪贴板历史」的帮助文本是「剪贴板 › 设置 › 剪贴板」，页面列表收起，截下 `<主题>-search-results.png`；搜一个不存在的词时出现「没有匹配的设置」；Esc 后页面列表回来、结果列表消失。原有的合成画布、「打开手写识别板」切到「手写」标签、「主字体」和「行内预编辑」展开折叠区几项搜索检查继续用回车打开第一条结果。
- `scripts/test-windows-settings-ui-pages.py`、`test-windows-input-habits-settings.py`、`test-windows-state-dir-parity.py`、`test-reference-source-inventory.py`、`test-windows-editions.py`、`test-windows-config-keys.py` 通过。
- 搜索框外观、结果列表的最高值和滚动、放大镜位置要看 CI 截图或 Windows 真机。
