# Agent Note: Windows 设置的「我的插件」改成 macOS 的插件库

Status: implemented

## Problem

macOS 的「我的插件」从 10-19 起是插件库（`packages/ui/src/settings/plugin-library-view.tsx` 的 `PluginLibraryView`，见 [macOS 插件页采用分类卡片库](2026-10-19-macos-plugin-library.md)）：顶部「声音与效果」「@ 名单」两张快捷卡片，「已安装 N 个插件」标题旁是「导入文件夹」「导入 .zip」，下面是搜索框、带计数的类型筛选、按类型分节的两列卡片网格、虚线框的空状态和折叠的「插件导入说明」。Windows 原生设置窗口（[Windows 原生设置窗口自己画「插件」页](2026-10-11-windows-settings-plugins.md)）还是共享页非 macOS 分支的分组列表：一组没有标题的两行入口、每类一组、每个插件一行，图标统一用页面图标，「导入插件」组和「从文件导入」一行放在页底，没有搜索、筛选、已安装计数和导入说明。用户要求 Windows 设置以 macOS 为准迁移。

## Decision

- `build_plugin_list` 照 `PluginLibraryView` 画：各块上下相隔 20，快捷卡片（可用宽度够 380 时两列，`responsive_grid(184, 12, 2)`）、标题行（「已安装」17 DIP 半粗体，同一 TextBlock 里接 12 DIP 次要色的「N 个插件」，读到目录后才有；导入按钮在右边）、导入后的说明、搜索框、筛选、分节、「无法载入的插件」、「插件导入说明」。卡片、筛选、搜索框、空状态和折叠说明的尺寸与颜色取自 `plugin-library-style.ts` 和 `styles.css` 的 macOS 一节（`--p-hair`、`--accent-soft`、`--accent-soft-border`），写在 `plugin_library_card`、`plugin_library_chip`、`plugin_library_search`、`plugin_empty_message`、`plugin_import_help` 里。
- 卡片是整张按钮，读屏名是标题，说明和标记作为帮助文本（与原来的列表行相同）；底色、描边和前景经模板资源覆盖，悬停铺淡强调色；插件卡片的类型图标不写死颜色、继承按钮前景，平时次要色，悬停和按下换成强调色（`packIcon` 的 `group-hover:text-accent`），快捷卡片的图标一直是强调色。类型图标是 `kindIcons` 在 Segoe Fluent Icons 里最接近的字形（`plugin_kind_glyph`），缺失的选择用信息图标。卡片说明照 macOS：内置包写「内置」，其余写版本，再接「按键旋律」和非内置包的作者（不再加「作者」二字），脚本插件也用这一句；`PluginPageModel.h` 的 `pack_row_description` 改成这个规则。
- 分类、筛选、搜索和分节的规则放进 `PluginPageModel.h` 的「插件库」一节（`library_categories`、`library_selected_kind`、`library_query`、`library_matches`、`library_groups`、`library_count`、`library_section_title`、`library_empty_message`、`command_table_note`），由 `tests/ui/plugin_page_model.cpp` 核对。Windows 宿主路由 / 指令也运行脚本插件，所以指令表一类总在、脚本插件排在最后，分节标题是「脚本插件（实验）」，筛选按钮上是「脚本插件」。搜索按 JavaScript 的 trim 去掉两头空白再逐字转小写，匹配名称、id、作者、描述和类型名；缺失的选择匹配标题加 id。选中的类型从目录里消失时按「全部」画，记下的值不改。
- 筛选和搜索记在窗口成员 `plugin_library_kind_`、`plugin_library_query_`，打开详情再回来、离开页面再回来都保留（共享页的 `libraryFilter` 在 `usePluginsSection` 里，页面失活时不清）。侧栏搜索结果落到插件页时清掉两者，被藏起来的卡片才画得出来。建搜索索引时不看筛选和搜索，每个包都记下，结果的位置写成「插件 › 类型」；「导入文件夹」「导入 .zip」也记进索引，替代原来的「从文件导入」一行。
- 搜索框打字时只清空并重画分节面板（`fill_plugin_sections`），不重建页面，焦点留在框里。点筛选按钮时整页重画，焦点经 `plugin_pending_focus_ = "filter/<类型>"` 落回刚点的按钮。关上详情后焦点回到原来那张卡片的逻辑不变。
- 筛选按钮放在 `VariableSizedWrapGrid` 的定宽格子里（100 × 34），按钮铺满格子；WinUI 没有按内容宽度换行的面板，这是与 macOS 的 `flex-wrap` 唯一的外观差别。

## Alternatives considered

- **只在原来的分组列表上加搜索和筛选** — 改动小，`plugin_nav_row` 继续用；但标题、计数、卡片网格、空状态和导入说明都不对，迁移的目标就是 macOS 的布局，留一半等于再做一遍。
- **筛选按钮用横向 StackPanel 放进可横向滚动的 ScrollViewer** — 按钮能按内容宽度排、间距与 macOS 一致；但类型多时要横向滚动才能看到后面的类型，macOS 是换行，全部类型一眼可见。选了定宽格子换行。
- **打字时整页重画再把焦点交回搜索框** — 与其他页面的重画方式一致；但整页重画会把滚动位置和输入法的组字状态一起丢掉，`render_page` 的焦点恢复还会把框滚进视野。只重画分节面板既保住焦点，也省掉每个字一次的整页重建。

## Consequences

- 原来列表用的 `plugin_nav_row` 只剩其他页面在用；`script_row_description`（版本 · 作者 X）不再被设置页调用，保留在头文件和单测里。
- `pack_row_description` 的输出变了（「内置」在前、作者不带前缀），单测随之改；这句话只在插件库卡片上用。
- 插件页的布局、悬停色、筛选按钮的换行和焦点落点都没有在运行中的窗口里检查过，要靠 PR 的 Windows CI 截图（`settings_ui.ps1` 现在核对快捷卡片、搜索框、「导入文件夹」和「插件导入说明」在页上，「从文件导入」不在）。
- 脚本插件的分组布局和「导入插件」的说明原来写在 [Windows 授权页](../architecture/2026-10-12-windows-script-grants-page.md)，那篇已加指针；脚本插件的详情、授权对话框和宿主调用仍按那篇。
- 打字统计的概览附注照 macOS 重排，见 [Windows 原生设置窗口自己画「打字统计」页](2026-10-11-windows-settings-typing-statistics.md) 的 Decision。

## Verification

- `tests/ui/plugin_page_model.cpp` 用 `x86_64-w64-mingw32-g++ -std=c++20 -fsyntax-only -Wall -Wextra -Wshadow -Werror` 编过，用本机 clang++ 编译运行输出 `plugin page model: ok`。
- `main.cpp` 依赖 WinUI 和 C++/WinRT 头文件，本机编不了，只能靠 PR 的 Windows CI。

共享状态与导航的模块边界见[插件设置状态与导航](../simplification/2026-10-23-plugins-section-state.md)，本篇的布局和筛选保留规则继续有效。
