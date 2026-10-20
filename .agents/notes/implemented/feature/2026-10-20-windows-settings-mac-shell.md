# Agent Note: Windows 设置窗口的外壳配色与侧栏对齐 macOS

Status: implemented

## Problem

共享设置 UI 给 Windows 的设计 token 就是 macOS 那一套（`packages/ui/src/theme/platform-tokens.ts` 的 `win` 分支只换字体），原生 WinUI 设置窗口的外壳却停在早先的 Windows 写法：侧栏 224 DIP，六个 `NavigationViewItemHeader` 分组标题，导航项 28 DIP 高、13 DIP 字，选中项整块填品牌绿配白字；侧栏和正文在浅色下同为 `#F6F6F6`，标题栏整行是侧栏色；分组卡片没有描边，深色卡片是不透明的 `#353535`。深色模式的主按钮、提示条按钮和头像用的是深色强调色 `#5FBF84` 配白字，对比度约 2.3:1。窗口 1000 × 760，标题是「水杉输入法设置」。macOS 的设置窗口（Tauri 加共享 React 设置）这些地方都不一样，用户要求以 macOS 为准迁移。

## Decision

外壳的数值逐项取自 macOS 正在使用的共享设置源码（`settings-style.ts` 的 `sidebar`、`sidebarHeader`、`content`、`macosToolbar`，`platform-controls-style.ts` 的 `navItem`，`platform-tokens.ts` 的 `mac` 分支和 `accentPair`，`tauri.macos.conf.json`），写进 `platforms/windows/settings/main.cpp` 的 `Palette` 和几个外壳构件：

- 侧栏平铺 16 个页面，不再插分组标题。`SettingsNavigation.h` 仍给每页记组号，组只决定顺序，与共享 UI 的 `settingsNavGroups` 对齐，`tests/ui/settings_navigation.cpp` 照旧核对。
- 侧栏宽 260（`MainWindow::sidebar_width`，标题栏的侧栏列和 `OpenPaneLength` 共用）。导航项可见底色块高 36、相隔 2、圆角 8，14 DIP 文字，15 DIP 图标，左右外边距 12。
- 选中、悬停、按下都只铺一层中性底色 `hover`（浅色 rgba(0,0,0,.06)、深色 rgba(255,255,255,.1)），文字和图标保持正文色。
- 配色：侧栏（窗口底色）`#E8E8EA` / `#2E2E30`，右侧 1 DIP 边线 rgba(0,0,0,.08) / rgba(0,0,0,.5)；正文 `#F2F2F4` / `#1F1F21`。标题栏里正文上方那一列（含窗口按钮）也用正文色，底部细线 rgba(0,0,0,.07) / rgba(255,255,255,.08)。
- 分组卡片加 1 DIP 描边 `palette_.stroke`。深色卡片取 macOS 的 rgba(255,255,255,.045) 叠在 `#1F1F21` 上的合成色 `#29292B`。
- 新增 `accent_strong`，深浅都是 `#2C7A4B`，用于所有底上是白字的填充：`make_primary`、默认输入法提示条的按钮、账号头像、社区资源图块。`accent` 只留给开关、复选框、滑块。
- 品牌行 24 DIP 图标、8 DIP 间距、15 DIP 半粗体产品名，左缘 22 DIP，与导航项的图标列对齐。
- 窗口初始 1000 × 780，标题「<产品名> · 设置」。最小尺寸保持 800 × 560。

### 改的是哪些模板资源

这些值都通过 `NavigationView` 的 `Resources()` 覆盖模板里的 `{ThemeResource}`，键名对照 Windows App SDK 1.7 固定源码 `microsoft-ui-xaml@a37e4e2c44582980aad0f9d91b11230a119cffac` 的 `NavigationView_themeresources.xaml` 和 `NavigationView.xaml`：

- `NavigationViewItemOnLeftMinHeight` 改 36，`NavigationViewItemButtonMargin` 从默认 4,2 改 0,1：底色块 36、间距 2，导航项本身高 38。
- `NavigationViewItemOnLeftIconBoxHeight` 改 15：模板把图标装进这个高度的 Viewbox。
- `NavigationViewContentGridBorderThickness` 改 1,0,0,0，`NavigationViewContentGridCornerRadius` 改 0：正文格的左边框就是侧栏右侧的边线，去掉 Windows 11 的左上圆角。
- 选中和悬停的底色、前景走 `NavigationViewItemBackground*`、`NavigationViewItemForeground*`。模板里图标的前景也取后一组，没有单独的图标键。
- 圆角设在 `NavigationViewItem.CornerRadius` 上，模板用 TemplateBinding 把它传到底色块。**never** 为此覆盖 `ControlCornerRadius`：它在 `NavigationView` 的资源里，会连正文里的所有控件一起改掉。
- 图标列宽仍是模板默认的 `CompactPaneLength - 8` = 40：图标中心落在侧栏左缘起 32 DIP 处，与 macOS 的 12 + 10 + 20/2 一致。文字起点 56，macOS 是 54。

### 不跟 macOS 的地方

- 最小尺寸不跟 macOS 的 360 × 540。macOS 在窄于 601 时收起侧栏，这个窗口没有窄窗口布局，侧栏常驻。
- 品牌图标不另裁 6 DIP 圆角。Windows 用的是 `msime.ico` 的 PNG 帧，四角本来就透明，裁了也看不出区别；WinUI 的 `Image` 也没有圆角裁剪。
- 侧栏和标题栏都是不透明色，不做 macOS 的半透明加模糊，系统材质另行处理。

## Alternatives considered

- **保留分组标题，只改选中色和尺寸**：改动最小，`NavigationViewItemHeader` 那套负边距（见[模板陷阱](../../implemented/bug-fix/2026-10-17-windows-settings-winui-template-traps.md)）也不用动。否决：macOS 侧栏是平铺的，组名是这次要对齐的主要差别之一；而且标题的 14 DIP 字号来自 StaticResource 改不动，留着它就和 14 DIP 的导航项一样大，层级反而不清楚。
- **导航项高 36、模板边距不动**：只改 `Height`，不碰 `NavigationViewItemButtonMargin`。否决：模板上下各留 2 DIP，可见底色块只剩 32，比 macOS 矮一截；左右再各让出 4 DIP，图标就对不上品牌行。
- **覆盖 `ControlCornerRadius` 得到 8 DIP 圆角**：差距清单最初这样建议。否决：这个资源在 `NavigationView` 上会作用到它的整个内容，正文的按钮、输入框、下拉框都会变成 8 DIP。
- **深色卡片直接用半透明白**：与 macOS 写法相同。否决：`palette_.card` 还用于主题卡片、行背景等别的表面，不全都叠在正文色上，半透明会随底色变。改用在正文色上合成好的不透明色。

## Consequences

- **收益**：Windows 设置的外壳和 macOS 同一套数值：平铺侧栏、中性选中、两层底色、带描边的卡片。深色模式下白字按钮的对比度从约 2.3:1 回到品牌绿的 5.3:1 左右。侧栏宽度只有一个常量，标题栏和侧栏不会再改漏一处。
- **代价**：外观依赖 Windows App SDK 1.7 模板的几个资源键：`NavigationViewItemButtonMargin`、`NavigationViewItemOnLeftIconBoxHeight`、`NavigationViewContentGridBorderThickness`、`NavigationViewContentGridCornerRadius`。升级 SDK 后键名一变，覆盖会静默失效，导航项变回 32 DIP 的底色块，或正文格变回左上圆角，功能不受影响。
- **重访信号**：升级 Windows App SDK，或 macOS 设置的 `settings-style.ts`、`platform-tokens.ts` 改了侧栏、正文或卡片的取值。
- 本篇取代[Windows 界面采用 macOS 的分组与选中样式](2026-10-15-windows-ui-macos-layout.md)里的两项：1000 × 760 的初始窗口，以及实心绿色的侧栏选中底。那一篇的其余约定继续有效。[模板陷阱](../../implemented/bug-fix/2026-10-17-windows-settings-winui-template-traps.md)第一条讲的分组标题已经不存在，那里关于 `ThemeResource` 和 `StaticResource` 能否覆盖的结论照旧成立，本篇的资源覆盖就依赖它。

## Verification

- 本机没有 WinUI 3 / C++/WinRT 工具链，`main.cpp` 只能由 PR 的 `CI (platforms)`「Windows native settings」编译链接。那一项还会运行 `tests/tools/settings_ui.ps1`：窗口高度改为 780，新增窗口标题以「 · 设置」结尾的检查，浅深色、800／1000 宽的截图用来审看侧栏、标题栏分列和卡片描边。
- `tests/ui/settings_navigation.cpp` 用 `x86_64-w64-mingw32-g++ -std=c++20 -fsyntax-only -Wall -Wextra -Werror -DMSIME_EDITION_FULL` 通过语法检查，又在本机用 clang++ 编译运行，退出码为 0。
- `scripts/test-windows-settings-ui-pages.py`、`test-windows-editions.py`、`test-windows-input-habits-settings.py`、`test-windows-config-keys.py` 通过。
- 资源键和模板结构对照上面那个固定提交的 XAML 源码逐个核对。导航项高度、图标列和圆角的实际像素，要看 CI 截图或 Windows 真机。
