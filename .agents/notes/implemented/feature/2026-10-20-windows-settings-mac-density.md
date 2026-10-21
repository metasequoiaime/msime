# Agent Note: Windows 设置的行与组密度、页首和跳转行对齐 macOS

Status: implemented

## Problem

[外壳配色与侧栏对齐 macOS](2026-10-20-windows-settings-mac-shell.md)之后，Windows 原生设置窗口的正文仍是早先的紧凑写法：组名 13 DIP、与卡片相隔 6、组间距 18，卡片内边距 12,6，行高 30（有说明时 38）、上下内边距 4，标签 13 DIP、说明 11 DIP，分隔线只画在内边距以内；正文边距 22,22,22,32。正文顶部又画一遍 22 DIP 的页面大标题，标题栏里的页名是 13 DIP 灰字，旁边常驻一个按跨页面浏览历史回退的返回按钮。打开网页的行是在行尾放「查看」「打开下载页」「前往」之类的文字按钮；「更多选项」用 WinUI 的 Expander，带自己的边框、底色和上下箭头。macOS 设置（Tauri 加共享 React 设置）这些地方都不一样，用户要求以 macOS 为准迁移。

## Decision

数值取自 macOS 正在用的共享设置源码：`packages/ui/src/theme/platform-tokens.ts` 的 `mac` 分支（`groupTitle`、`group`、`groupsGap`、`hover`），`settings-style.ts` 的 `groups`、`contentColumn`、`macosToolbar`，`core/platform-controls-style.ts` 的 `row`、`linkRow`、`moreOptions*`、`groupNote`，以及 `platform-controls.tsx` 的 `LinkRow`、`MoreOptions`。都落在 `platforms/windows/settings/main.cpp`：

- `add_group`：组名 14 DIP 半粗体、左右缩进 8，与卡片相隔 8；组上边距 20，加页面的 Spacing 4，组与组相隔 24。
- 卡片里的行面板左右留 `row_inset`（16），上下不留。`add_row` 经 `append_row_frame` 用 -16 的左右负边距伸到卡片两边，再给自己 16,10 的内边距；分隔线是这个外框的上边框，所以贯穿卡片整宽。标签 14 DIP、说明 12 DIP；标题区最小高度 36（没有说明时 24），加上下内边距得到 56（44）。
- 不是行的内容（说明文字、表单、子卡片）留在 16 DIP 的缩进里。`group_note` 上下各留 10（macOS `groupNote` 的行内边距）；放进分组卡片的子卡片改用 `make_inner_card`，上下各留 10，排在组首组末时不贴外层边框。输入方案的单选行（后来换成「中文方案」卡片）和打字统计的操作区本来就自己画分隔线，改走 `append_row_frame`，线也贯穿整行。
- 正文边距 `content_padding`：上 24、左右 48、下 44。正文窄于 640 DIP（窗口拉到最窄 800）时左右退到 32。
- 正文不再画页面大标题。页名只在标题栏里：15 DIP 粗体、正文色，左右 20 DIP，放不下时截断。原来大标题下的页面说明后来换成了 macOS 的页首引导（说明配水杉娘插画，见[页首引导与侧栏产品图标](2026-10-20-windows-settings-mascot-intro.md)），说明仍带 `settings-page-<页面 id>` 的自动化 ID，`settings_ui.ps1` 靠它确认正文已换成这一页。「候选窗口」「主题」页的预览工具（界面预览开关、编辑主题）仍在它下面右侧。
- 标题栏去掉历史返回按钮，连同 `history_`、`go_back`、`navigate` 的 `remember` 参数和 `update_passthrough` 一起删掉：标题栏里不再有要接收点击的控件，整条都是拖动区，不需要放行区域。
- 新增 `link_row`，对应 `LinkRow`：整行是一个透明按钮（`make_row_button`），悬停和按下铺 `palette_.hover`，行尾是 12 DIP 灰色的「↗」（窗口外）或「›」（本窗口的另一页），按钮以标题命名、以说明描述。`url_row` 改为它的外链形式。「关于」页的「其他平台下载」「历史版本」「隐私政策」「开源许可协议」「第三方组件许可」和问题报告下的「诊断日志」用它。「完整文档」在共享帮助页是带按钮的 `ActionRow`，照旧是按钮。
- 新增 `add_fold`，对应 `MoreOptions`，替换所有「更多选项」式 Expander（输入页里经 `add_more_options` 的折叠区，以及候选窗口、标点、候选词翻译、腾讯云地域、更多音效、语音快捷键、语音服务、本地模型、AI 服务和主题页的「自定义配色」）：标题行是与普通行同样内边距、至少 44 高的整行按钮，标题右边 12 DIP 灰色「›」，展开时用 150 ms 的 `RotateTransform` 动画转到 90°，`UISettings.AnimationsEnabled` 为假时直接转过去；折叠体上边框画出标题行与第一行之间的分隔线，里面的行照常经 `add_row` 伸到卡片两边。展开状态仍由调用方的成员保存，跨页面重建保留，搜索落进折叠区时照旧展开。皮肤页「已忽略 N 个无效皮肤目录」不是设置折叠区，仍用 Expander。

### 无障碍与验收

折叠标题行是普通 `Button`，没有 UIA 的 ExpandCollapse 模式；展开与否写在 ItemStatus 里（「已展开」「已收起」）。`settings_ui.ps1` 的 `Set-CandidateMore` 改为按折叠区里「行内预编辑」在不在判断状态、点按 Invoke 切换，再核对 ItemStatus；每页还确认窗口里没有名为「返回」的元素。

## Alternatives considered

- **保留 Expander，只用资源覆盖它的外观**：展开收起和 UIA 的 ExpandCollapse 都是现成的。否决：对照 Windows App SDK 1.7 固定源码 `microsoft-ui-xaml@a37e4e2c44582980aad0f9d91b11230a119cffac` 的 `Expander.xaml`，箭头字形、箭头外边距、标题内边距和内容边框厚度都是 `StaticResource`，页面资源改不动（见[模板陷阱](../bug-fix/2026-10-17-windows-settings-winui-template-traps.md)）；标题的 ToggleButton 在悬停时只改箭头小方块的底色，整行没有悬停底色；展开时内容还有一段位移动画。要做成 macOS 的样子只能整份换模板。
- **自己写一个带 ExpandCollapse 模式的折叠按钮**：给按钮派生类写自定义 AutomationPeer，读屏能报「展开 / 折叠」。否决：要在这个不经 XAML 编译器的 C++/WinRT 工程里第一次写可组合派生类和自动化提供者，本机又编不了 WinUI，出错只能靠 CI 一轮轮试；ItemStatus 已能把状态告诉读屏。
- **卡片行面板不留左右内边距，逐处给非行内容补缩进**：和 macOS 的 `groupBlock` 写法一样，行不用负边距。否决：直接往分组里放说明、表单、子卡片、列表的地方有六十多处，漏一处就贴着卡片边；留 16 DIP 缩进、让行用负边距伸出去，只有组首组末的说明和子卡片要补上下间距。
- **行高一律 56**：macOS 的 `rowHeight` 不分有没有说明。否决：没有说明的开关行在 Windows 上会留出大片空白，一列开关显得松散；按差距清单取 56 / 44。
- **页面说明一起去掉**：macOS 没有插画引导的页面正文顶上什么都不写。否决：引导行（13 DIP 说明配插画）是后续一批的事，在那之前先留着这行说明；它也是界面验收确认切页的稳定标识。

## Consequences

- **收益**：Windows 设置的组、行、字号和正文边距与 macOS 同一套数值，页名只出现一次，跳转行和折叠区与 macOS 一样整行可点、没有多余边框。标题栏不再需要维护放行区域。
- **代价**：同样的内容比原来高出不少，800 × 780 的窗口一屏能放的行变少。折叠区失去 ExpandCollapse 模式，读屏只从 ItemStatus 得知状态。卡片四角的悬停底色依赖 WinUI 的 `Border` 按 `CornerRadius` 裁剪子元素；若某个版本不裁，组首组末的跳转行悬停时会露出直角，只影响悬停的观感。
- **重访信号**：macOS 的 `platform-tokens.ts` 改了 `group`、`groupTitle`、`groupsGap`，或 `settings-style.ts` 改了 `contentColumn`、`macosToolbar`；Windows App SDK 升级后 `Button` 模板不再读 `ButtonBackgroundPointerOver` 这组资源。
- 页首插画、引导文案和「关于」页不放引导行由[页首引导与侧栏产品图标](2026-10-20-windows-settings-mascot-intro.md)接手，改的是 `make_page_header`。
- 本篇取代[模板陷阱](../bug-fix/2026-10-17-windows-settings-winui-template-traps.md)第三条里「小标题 13 DIP、说明 11 DIP」的取值：「连接 AI 助手」的小标题和说明跟着 `add_row` 改为 14 / 12 DIP，那一条「内容直接进行面板、不套第二张卡片」的结论照旧成立。[Windows 界面采用 macOS 的分组与选中样式](2026-10-15-windows-ui-macos-layout.md)里的分组卡片与两列布局继续有效，具体数值以本篇为准。

## Verification

- 本机没有 WinUI 3 / C++/WinRT 工具链，`main.cpp` 只能由 PR 的 `CI (platforms)`「Windows native settings」以 `/W4 /WX` 编译链接，并运行 `tests/tools/settings_ui.ps1`。新的 `Set-CandidateMore`、ItemStatus 核对和「没有返回按钮」检查都在那里跑；浅深色、800／1000 宽的整页截图用来审看组距、行高、分隔线、跳转行和折叠区。
- 资源键（`ButtonBackground*`、`ButtonForeground*`）对照上面那个固定提交的 `CommonStyles/Button_themeresources.xaml`：默认 `Button` 模板的悬停、按下状态把 `ContentPresenter` 的底色换成这两组 `ThemeResource`；Expander 的结论对照同一提交的 `Expander.xaml` 和 `Expander_themeresources.xaml`。
- `scripts/test-windows-settings-ui-pages.py`、`test-windows-input-habits-settings.py`、`test-windows-editions.py`、`test-windows-config-keys.py`、`test-windows-state-dir-parity.py`、`test-windows-expression-symbols-parity.py`、`test-reference-source-inventory.py` 通过。
