# Agent Note: Windows 候选窗绘制皮肤的版式、外框、阴影、翻页箭头与收紧宽度

Status: implemented

## Problem

共享 catalog 已经按 msime-windows `c0a9d371` 解析 `[candidate_window]` 的版式表（`geometry`、`horizontal`、`vertical`）、`border_width_dip`、`item_corner_radius_dip`、`shadow`、`backdrop`、`page_arrows` 和 `fit_content`（见 [跨平台候选皮肤清单与 msime-windows 对齐](2026-10-19-windows-skin-schema-parity.md)），Windows 宿主却一个都不画：为 msime-windows 写的皮肤装上以后，卡片还是本宿主自己的内边距、行高、序号位置、外框和阴影，`page_arrows = true` 的皮肤也没有列表里的翻页箭头。这是 msime-windows 皮肤视觉兼容的第一步。约束是不写这些键的皮肤和没有皮肤时，卡片要和以前画得一样。

## Decision

`CandidateSkin.h` 的 `candidate_skin_layout` 从 `SkinSummary` 读出这几组键，交给 `CandidateWindow::set_skin_layout`。`CandidateSkinLayout.h`（不碰 JSON 和 Direct2D）按当前排列和候选字号解析成 `CandidateSkinMetrics`：横排或竖排表里写了的键盖过共享表，em 长度乘候选字号后取整，同 msime-windows 的 `ApplyLayer` 与 `ResolveLength`。`candidate_card_metrics` 多收这份解析结果和这一页最宽序号的实测宽度，只改皮肤写了的那几项，其余字段的算式一字不改；尺寸、行、首行、点击和读屏都还是读这一份 metrics。

### 每个键在这张卡片上的含义

- `padding`：卡片四边到候选的距离，行左右各缩进它，上下边距都是它。
- `item_gap`：竖排两行、横排两行之间的空；横排每列自带的空变成 msime-windows 文字后的 6 DIP 加 `item_gap`（宿主原来的 8 DIP 正是 6 加 2）。
- `item_extra_height`：单行行高是 1.35 倍字号加它。
- `number_scale`：序号字号是候选字号乘它；宿主原来按候选字号画序号，没写时相当于 1。
- `number_inset`：序号从高亮左缘量起左对齐，不小于 5（msime-windows 先减 5 再在 `textPadLeft` 加回 5）；序号宽取实测和 9 DIP 的较大者。没写时序号仍在 0.8 倍字号宽的一栏里右对齐。
- `number_gap`：序号右缘到候选文字的空；没写时是宿主原来的 0.2 倍字号加 8。
- `item_padding_right`：内容后、行（列）右缘前的留白。
- `item_min_width`、`justify_rows`：只对横排生效，每列的最小宽度，以及把一行比列表窄出来的宽度均分给这一行看得见的列。
- `selected_bar_width`（不小于 3）、`selected_bar_height_em`（字号倍数，不取整）、`selected_bar_inside`：选中竖条的宽、高和是否落在高亮左缘之内。
- `item_corners_follow_card`：高亮落在列表四角上的那几个角用卡片圆角画（卡片直角时取 0.01，开关不失效），顶上两角只在首行不占高度时算。
- `preedit_margin_dip`：拼音行的外边距，只在显示拼音时生效，上下两段撑高首行，logo 和首行翻页在剩下那段里居中。
- `trim_hidden_preedit_gap`：这里什么都不用做。msime-windows 用它抵掉拼音隐藏后 body 还留着的 2 DIP 行距，这张卡片的首行和候选之间没有这段行距。
- `border_width_dip`：外框线宽，0 不画外框。
- `item_corner_radius_dip`：高亮圆角，拼音行底色同圆角。
- `shadow`：只缩放宿主阴影的不透明度，`soft` 乘 0.5、`strong` 乘 1.6 且不超过 1、`none` 不画（不画时连兜底阴影也跳过），几何不变，同 msime-windows 的 `ShadowAlphaScale`。
- `backdrop`：本宿主还没有系统材质，按 msime-windows 在透明效果关闭或系统不支持时的回退画：`surface` 补成不透明。后来由 [Windows 候选窗按皮肤垫系统材质](2026-10-19-windows-candidate-backdrop-material.md) 补上了亚克力和云母，这里的不透明底色只在材质画不出来时用。
- `page_arrows`：翻页从首行挪进列表，首行不再为它留宽度和高度，也不画「n / m」。横排是候选右侧的一窄列，箭头贴右下角、竖直居中在最后一行上；竖排是最后一行下方 `button_height + 2` 高的一矮行，「‹」的左端对齐序号左端。按钮尺寸、外边距和字形照 `candidate_skin_scene.cpp` 与 `PagerArrows.cpp`。点击仍走 `candidate_pager_hit`（`CandidatePagerLayout::in_list` 区分两种翻页），上一页在第一页上变暗、不响应，下一页一直可用，与首行翻页同一条规则。悬停铺 `hover`，按下铺 `selected`（透明时仍是 `hover`）。
- `page_arrow_style`：`chevron` 是序号色折线；`triangle` 是候选文字色的圆角实心三角，按钮与候选行等高，横排更宽、前面一条 `page_arrow_divider` 色的 1 DIP 竖线，这个颜色槽位因此第一次被画出来。
- `fit_content`：不垫 160 DIP 最小宽度，行内不留宿主文字后的余量，右侧留白等于序号左侧留白（msime-windows 的 `contentPadRight = contentPadLeft + textPadLeft`），横排每列只带 `item_gap`；`min_width_dip` 照常生效。本宿主的卡片在同一次输入里本来就不只增不减，msime-windows 说的「宽度立刻缩回」不需要另做。

### 与用户偏好的先后

和颜色、字体一样：用户在 Windows 设置里真正能改的项由用户说了算，其余由皮肤决定，皮肤没写的回到调色板。卡片圆角是用户的 `candidate_corner_radius`，其次皮肤的 `corner_radius_dip`，再次调色板；高亮圆角是皮肤的 `item_corner_radius_dip` 或调色板的，用户设了卡片圆角时仍被它封顶（`candidate_row_radius`）；外框线宽、阴影、材质和版式在 Windows 上没有用户偏好（`candidate_padding` 只有 macOS 声明），皮肤直接决定；用户的不透明度照常淡化底色、背景图和外框；用户的选中竖条开关仍决定画不画竖条，皮肤只改它的形状。`paint()` 里有同样的一段说明。

### 没写的键回到哪里

msime-windows 里没写的键沿用 `base` 外观的出厂清单（`wechat` 的 `padding = 2`、`microsoft` 的 `page_arrows = true` 等）。共享 catalog 目前只按外观补颜色和卡片圆角、不补版式，所以这里没写的键回到的是本宿主自己的版式，而不是 `base` 外观的。

## Alternatives considered

- **把 msime-windows 的 `CandSkinTokens` 与 msimeui `CandidateList` 整套搬过来，替换 `CandidateCardSize.h`** —— 最强的理由是与 msime-windows 逐像素一致，连没写的键也按 Fluent 的出厂值画。不采用：本宿主的卡片有 logo、首行翻页、释义预留行高、横排单行收窄这些 msime-windows 没有的规则，整套换掉会让每一个现有皮肤和无皮肤的卡片都变样，违背这一步的约束；逐键映射到现有 metrics 既能保住默认，又能让写了键的皮肤得到来源的含义。
- **在共享 catalog 的 `windows_looks` 里按 `base` 外观补版式** —— 这样没写的键也有 msime-windows 的值，`base = "microsoft"` 的皮肤会自动有三角翻页箭头。不采用：已经在用的 `base = "wechat"` / `graphite` 等皮肤即使一个版式键都没写，卡片也会变样，而且这是改所有宿主共享的扫描结果；留到确定各宿主都要这些出厂版式时再做。
- **`page_arrows` 只把箭头挪进列表，首行保留「n / m」** —— 页码信息不丢。不采用：同一张卡片两处翻页控件，首行也因此一直占着高度；为 msime-windows 设计的皮肤本来就没有页码。
- **真的画亚克力和云母** —— 这是 `backdrop` 的本义。需要把 msimeui 的 `DeviceResources` 从 DirectComposition 交换链换成 Windows.UI.Composition（DispatcherQueue、HostBackdropBrush、效果画刷），没有 Windows 工具链验证，MinGW 交叉构建里的 WinRT 头文件也没有核实过；这一步先做来源本来就有的「材质不可用」回退，免得半透明底色直接透出后面的窗口。之后的做法和 MinGW 头文件的处理见 [Windows 候选窗按皮肤垫系统材质](2026-10-19-windows-candidate-backdrop-material.md)。
- **`shadow` 档位直接用 msime-windows 的两层阴影参数** —— 档位写了以后更接近来源的样子。不采用：来源对这个键的定义是只缩放阴影不透明度、不改几何，缩放宿主自己的阴影符合这一定义，`soft` / `strong` 也和没写时属于同一套阴影。

## Consequences

Linux X11 的字体与候选布局由[原生画布布局](2026-10-20-linux-canvas-layout.md)独立接入，沿用同一份共享清单字段，不依赖本宿主的尺寸模型。

收益：为 msime-windows 写的皮肤在 Windows 上得到它们写下的内边距、行距、行高、序号位置与字号、竖条、贴角高亮、拼音行外边距、外框、高亮圆角、阴影档位、列表翻页箭头和收紧宽度；不写这些键的皮肤和无皮肤的卡片不变（`windows-candidate-card-size` 断言空版式下 metrics、卡片尺寸和每一行逐位相同）。版式规则都在不依赖 Windows 的头文件里，本机就能测。

代价：
- 没写的键回到本宿主版式而不是 `base` 外观的出厂值，同一个包在两边仍会有差别；`base = "microsoft"` 但没写 `page_arrows` 的皮肤在这里没有列表翻页箭头。
- `backdrop` 只是不透明回退，没有模糊材质（已补上，见 [Windows 候选窗按皮肤垫系统材质](2026-10-19-windows-candidate-backdrop-material.md)）。
- 打开 `page_arrows` 的皮肤看不到「n / m」页码，读屏也不再报告页码这一项（两个箭头照常报告、可执行）。
- 九宫格 `border_image`、自由挂件、insets 和 `line_gap_dip` 见 [Windows 候选窗绘制 canvas 皮肤](2026-10-19-windows-candidate-canvas.md)，`expand_panel` / `expand_rows` 见 [Windows 候选窗绘制皮肤的卷轴展开](2026-10-19-windows-candidate-skin-expand-panel.md)；原生设置窗口的皮肤预览也还不画这些键。

验证：`windows-candidate-card-size`（新增空版式逐位相同、各键的尺寸与行、列表箭头位置与点击、贴角半径）、新加的 `windows-candidate-skin-layout`（JSON 读取、越界值当作没写、横竖排覆盖、em 取整、阴影与底色换算）、`windows-candidate-accessibility`（列表箭头不报页码）以及 `windows-candidate-skin`、`-palette`、`-layout-reload`、`-shadow`、`-window-style`、`-skin-reload`、`-theme-reload`、`windows-skin-resource-revision` 在 Mac Studio 上用 `c++ -std=c++17 -Wall -Wextra -Werror` 编译并通过；`CandidateWindow.cpp` 用本机 `x86_64-w64-mingw32-g++ -fsyntax-only -Wall -Wextra` 检查无错误无警告。`server_main.cpp` 缺 curl 头文件没有单独检查，链接与完整交叉构建交给 PR 上的 Windows CI。没有在真实 Windows 上目测：Direct2D 的逐角圆角路径、圆角三角箭头、悬停与按下底色、阴影档位的观感都还要在屏幕上看一遍。
