# Agent Note: Windows 设置的词库页对齐 macOS

Status: implemented

## Problem

macOS 设置（共享设置 UI 的 `DictionarySettingsPage` 桌面分支）的词库页顶上是一条导航：强调色眉标「词库工具」、一句说明和「查找词库」「管理词库」「云端词库」三个标签。「查找词库」进页面就列出所选词库的全部词条，搜索框「词条或编码」输入后 280 ms 实时筛选，词库下拉框还有日语、韩语 · 汉字、粤语、注音、笔画五个只读资源词库；结果是带边框、限高 320 的五列表格，内置词条不给编辑和删除，「新增词条」在底栏左边、翻页在右边。「管理词库」放导入与导出（含「导出当前类型」「导出全部」）、词库更新（单独的版本行、组末说明带 MiB 进度和「已是最新」）、「更多」里进入背单词子页的一行和学习数据（危险样式按钮，结果画在按钮下面）。

Windows 原生设置窗口的词库页是一整列：词库更新在页顶，查询要点「查询」或回车、刚进页面不列词条、走只列个人词条的 `list`；只有五个本机种类；表格四列，内置词条有「调权重」「删除」；只能导入不能导出；云词库是页内一行；背单词整组画在页里，复习卡片在设置里，没有导入词表；学习数据的结果写在页顶的全局通知里。

## Decision

### 导航与标签

- 页顶是 `make_lexicon_nav`：左边 11 DIP 半粗体、`CharacterSpacing` 120 的强调色「词库工具」和 12 DIP 次要色「管理本机词条与云端词库」，右边是 `page_tab_bar`（前缀 `lexicon-tab`）画出的那一条，但从临时容器里取出来放进导航行，不铺满，按 `dictionaryTabs` 每段 96 DIP 定宽；底下一条分隔线。建索引时只登记标签名。
- 标签下面是词库操作的结果（`lexicon_feedback_`，InfoBar，可关），记下产生它的标签，切到别的标签时清掉，对应共享页切标签时的 `clearPhraseFeedback`。查询条件变化时也清（共享页 `use-dictionary-manager.ts` 的防抖查询 effect 先 `clearPhraseFeedback`）：搜索框改字、换「本地词库类型」和换「管理词库」的「个人词库」（共享页的 `dictionaryKind` 也在那个 effect 的依赖里）都走 `clear_lexicon_feedback_for_query`，清掉 `lexicon_feedback_` 并把页上的结果条（`lexicon_feedback_bar_`）就地 `IsOpen(false)`，不重画整页，免得打字时搜索框丢焦点；清除学习数据的结果另记在 `learned_feedback_`，画在「学习数据」组按钮下面，同样随切标签清掉。
- 「完善公共词库」在所有标签上都画。「云端词库」只放打开云词库面板的一行：共享页在这里内嵌 `CloudDictionaryTab`，原生窗口不嵌网页，面板仍由 `MSIME.exe` 显示。

### 查找词库

- 本机词库和资源词库都发 `browse`（与共享页相同），空查询也列出内置词条。种类和下拉框里的名字在 `LexiconImportModel.h`：本机词库改名为「中文 · 拼音」等，提示文案里仍说「拼音」（`dictionaryKindLabel`）。选中的资源词库单独记在 `lexicon_resource_kind_`，导入导出和新增词条始终用本机词库 `lexicon_kind_`，与共享页 `resourceKind` 和 `dictionaryKind` 分开存的做法一致。
- 进页面（`navigate` 每次进入都把 `lexicon_listed_` 清掉）和换词库时就列出；搜索框 `TextChanged` 记下文字、重启 280 ms 的 `DispatcherQueueTimer`，到点从第一页查。每次查询加一个代次（`lexicon_list_generation_`），只有最后发出的那次的回答算数。
- 结果块装在 `lexicon_results_host_` 这个 Border 里，查询回来时 `refresh_lexicon_results` 用 `TryEnqueue` 在当前事件之后只换掉它的内容，不重建整页。整页重画会拆掉搜索框，打字打到一半丢焦点；翻页按钮也不会在自己的点击里被拆掉。结果块不在页面上（别的标签、子页）时什么也不做。
- 表格照 `settings-style.ts` 的 `phrase*`：1 DIP 描边、8 DIP 圆角，36 DIP 浅底表头，每行 40 DIP、悬停浅底，最多 320 DIP 高、在里面滚动；本机词库五列（编码 1 份、词条 1.4 份、权重 48、来源 32、操作 88），来源写「内置」或「个人」，内置词条不画操作（用户拍板与 macOS 一致，只去掉界面，宿主的 `edit` 仍接受内置词条改权重）；资源词库两列，日语和韩语第一列叫「读音」。编辑和新增仍用 ContentDialog。
- 资源词库没装时说「X词库尚未安装。」：共享页还让人去「输入」设置下载，Windows 没有按需资源包，不能照抄。

### 管理词库

- 「导入与导出」：「个人词库」「文件格式」两个下拉框，一排「导入」「导出当前类型」「导出全部」。原来「本地词库管理」里的导入挪到这里，规则不变，结果改写在标签下面，成功后从第一页重新列出。
- 「导出当前类型」在工作线程上按 1000 行一页发 `export`（Rime 按标准格式读），按 `export_payload` 整理：去空行，拼音只留两个字以上的词，加 UTF-8 BOM；汉字自动注音和细胞词库只能导入，选了它们时按钮不可用。「导出全部」逐类发 `list`（空查询只列个人词条，快捷短语带上的内置行跳过），每行「类别、编码、词条、权重」，开头一行表头。超过 32 MiB 时说「词库导出文件过大，请分批导出。」。读完回到界面线程，用 `IFileSaveDialog`（写法同 `pick_lexicon_file`，默认文件名与共享页 `dictionaryExportName` 相同）选位置，用 `write_settings_document` 先写临时文件再改名。
- 「词库更新」照 `DictionaryUpdates`：开关没有说明，「词库版本」单独一行（`plugin_summary_row`），运行时按钮写「更新中…」，组末说明随状态变（下载时「正在下载 X / Y MiB」，已下载向下取整、总量向上取整）。2 秒轮询照旧；只是说明变了时就地换掉那一行字（`dictionary_update_note_`），开关、版本、按钮、能否回滚变了才重画整页，这一组不在页面上时不重画。
- 「学习数据」的按钮前景用 `danger_color()`。

### 背单词子页

- 「更多」里「背单词：用输入过的英文单词复习词汇」一行（`link_row`）进入子页，设置类别 `settings:vocabulary`（设置窗口的页面路由 `vocabulary`）和插件页的「去背单词」直接打开它；头上的「词库」回到「管理词库」，焦点落回那一行；离开词库页或从搜索结果落到词库页时子页收起，子页开着时点「词库」这一页本身的结果也收起（`navigate` 遇到同一页直接返回，`open_search_result` 自己重画）。子页画在词库页里（`vocabulary_view_`），不进搜索索引。和 macOS 的 `vocabulary` 子页一样，标题栏写「背单词」，正文顶上没有页首引导（`settings-mascot-intro.tsx` 里没有 vocabulary），「‹ 词库」是正文第一项。打开时焦点落在「‹ 词库」上：打开总会发一次 `load`，回来后整页重画、换掉返回按钮，所以 `vocabulary_pending_focus_` 留到那次重画（`vocabulary_busy_` 已落下）才清掉，焦点落到新按钮上。
- 内容照 `VocabularyReviewPage` 在能开面板的宿主上的那一支（`openPanel`）：「背单词」组的三格指标（30 DIP 强调色数字）、「在这里管理词书；复习在背单词面板里进行。」的说明和「开始复习」，词书一组（词书、每日新词、「导入词表（CSV / TXT）」和格式说明、插件词书说明、删除词表），清空复习进度。导入词表用 `IFileOpenDialog` 选 .csv/.txt，不超过 1 MB，解码顺序与导入词库相同，词表名取文件名去掉扩展名（最多 64 个 UTF-16 单元），经 `msime_client_vocabulary_review` 的 `import`。
- 设置里不发复习卡片，「开始复习」打开背单词面板，与 macOS 相同。为此在 `client-core` 的 `SurfaceRoute` 加了 `Vocabulary`（路由 `vocabulary`，面板几何就是原来 Tauri 里 `VOCABULARY_PANEL` 那一份：`vocabulary-panel`、560 × 680、底部居中），Tauri 的 `open_vocabulary_panel` 改走 `open_route_panel`；`MSIME.exe` 启动参数和转发来的第二次启动本来就按 `route.panel_for` 打开任意面板路由，不用再改。原生窗口在 `shell_links` 里加 `vocabulary_panel`，按钮经 `open_shell` 启动 `MSIME.exe --route=vocabulary`，没选词书或正在读写时不可用（共享页 `disabled={busy || !selected}`）。`vocabulary` 面板路由与设置类别 `settings:vocabulary` 是两条路由：后者仍打开设置里的这个子页。

## Historical audit

- [词库页顶部标签与词条查询](2026-10-17-dictionary-top-tabs-and-lookup.md)：部分重叠。那是共享 UI 的同一决定，本篇是它在 Windows 原生窗口的落地，规则照搬；云词库内嵌一项不跟。
- [Windows 设置的页内标签框架](2026-10-20-windows-settings-page-tabs.md)：部分重叠。本篇用它的 `page_tab_bar` 和 `in_tab`，但把标签条放进自己的导航行、定宽不铺满。
- 检索 `rg --hidden --glob '!.agents/notes/archived/**' "LexiconImportModel|背单词|build_lexicon_page|词库页" .agents/notes/`，其余命中是共享 UI 或移动端的词库与背单词决定（动作归属、进度原子写、Android 菜单等），与原生窗口无关。

## Alternatives considered

- **不加面板路由，按共享页开不了面板的那一支（`reviewing = !openPanel`）把复习卡片留在设置里**：不用碰 `client-core` 和 Tauri。最初是这样做的，评审后改掉：缺口和计划都要求「开始复习」打开面板，用户只排除了几项大件，背单词面板不在其中；而且面板早就存在（`open_vocabulary_panel`），缺的只是一条路由，加上它比在原生窗口里再维护一份卡片便宜。
- **原生窗口自己画一个背单词面板窗口**：不经 `MSIME.exe`。没有采用：面板的复习交互、键盘操作和插件词书都在共享 UI 里，原生再写一份就有两套行为要对齐。
- **同步状态的 InfoBar 和失败列表（「重试」「移除」）**：缺口清单要求读 `pending_count`、`failed_requests`、`snapshot_error`。没有做：桌面宿主的 `browse` 和 `list` 回答里没有这几个字段，`retry`、`dismiss_failure` 在桌面上直接报错（只给 Android 的个人词库队列），共享页走 `browse` 时也把它们置零，macOS 桌面上从来不显示这一块。
- **搜索时整页重画，画完再把焦点和光标放回搜索框**：不需要单独的结果容器。没有采用：整页重画会重建所有控件、滚动位置也要再对，每 280 ms 一次在长列表上可见地闪；就地换掉结果块更便宜，也不用猜光标位置。
- **编辑和新增改成列表上方的内联表单**：照 macOS。没有做：计划里明确保留 ContentDialog，它是 WinUI 惯用的写法，内联表单在整页重画的窗口里还要单独保存草稿。

## Consequences

- 词库页的结构、文案和交互与 macOS 一致：进页面就有词条、实时筛选、能看资源词库、能导出、能导入词表；内置词条在界面上不能再改权重或删除，想调整只能先在别处（宿主接口仍支持）。
- 搜索结果块和词库更新说明两处是就地更新，别的地方仍是整页重画；以后往这一块加控件时要放进 `make_lexicon_results` 才会跟着刷新。
- 背单词子页不进搜索索引，搜「每日新词」找不到；搜「背单词」落到「管理词库」里的那一行。
- 在面板里复习后，设置子页的三格指标要等下次进入子页才更新（每次进入都重读），与共享页挂载时读一次相同。
- `SurfaceRoute::ALL` 多了一项；macOS 的原生宿主不发这条路由（macOS 设置在 Tauri 里直接调 `open_vocabulary_panel`），所以 macOS 的启动面板分派没有接它，和 `clipboard` 一样。
- 导出在内存里拼好最多 32 MiB 的文本再写盘，与共享页相同。
- 资源词库在 Windows 上通常只有粤语、注音、笔画随安装包带着，日语只在日文版里，其余版本选日语时显示没有安装。

## Verification

- `tests/ui/lexicon_import_model.cpp` 加了种类和名字、资源词库提示与说明、页状态、导出当前类型（拼音只留多字词、Windows 列序、空结果）、导出页行数、导出全部的顺序、表头和行、导出文件名、词表上限和词表名的检查；本机用 clang++ 编译运行通过，并用 `x86_64-w64-mingw32-g++ -std=c++20 -Wall -Wextra -Werror -fsyntax-only` 检查。
- 本机没有 WinUI 3 / C++/WinRT 工具链，`main.cpp` 只能由 PR 的 `CI (platforms)`「Windows native settings」以 `/W4 /WX` 编译并运行 `tests/tools/settings_ui.ps1`：词库页在「查找词库」时有搜索框和翻页状态、没有「查询」按钮、没有导入导出；「管理词库」有导入、两个导出、词库更新说明、单独的「词库版本」行和背单词入口，进入背单词子页后不再显示标签、读到进度后有「开始复习」、没有复习卡片、没有词库的页首引导，返回按钮在读到进度、整页重画后重新查找再点，返回后回到「管理词库」；「云端词库」有打开面板的按钮；截下管理、背单词和云端三张图。
- 路由：`crates/client-core/src/host_surface/tests.rs` 核对 `vocabulary` 解析成 `SurfaceRoute::Vocabulary`、面板是 `vocabulary-panel` 560 × 680、`settings:vocabulary` 仍是设置类别，`SurfaceRoute::ALL` 的往返和面板标签检查自动覆盖新路由；`apps/desktop/src-tauri/src/tests.rs` 的可关闭面板检查改为全由路由表覆盖。`tests/ui/settings_navigation.cpp` 核对 `shell_links::vocabulary_panel` 的启动参数是 `vocabulary`。本机盘紧、不跑 `cargo`，Rust 只过了 `rustfmt --check`，编译和单测交给 CI。
- 背单词面板在 Windows 上由「开始复习」打开、能复习并让设置子页下次读到新进度，要在真机上确认。
- `scripts/test-windows-settings-ui-pages.py`、`test-windows-settings-art.py`、`test-windows-input-habits-settings.py`、`test-windows-state-dir-parity.py` 通过。
