# Agent Note: Windows 设置窗口三处显示缺陷都出在控件模板和构件的用法上

Status: implemented

## Problem

真机打开 `msime-client-settings.exe` 看到三处，都不是逻辑错误，而是 WinUI 控件模板和本窗口自己的构件被用错：

1. 侧栏六个分组标题的下半截被裁掉，「工具」看着像「丁目」、「支持」像「古挂」。`main.cpp` 的 `make_navigation` 给 `NavigationViewItemHeader` 设了 `FontSize(11)` 和 `Height(24)`，但 Windows App SDK 的默认模板里，`HeaderText` 的 `Style="{StaticResource NavigationViewItemHeaderTextStyle}"` 显式写死 `FontSize=14`——样式 setter 盖过从父元素继承的值，`FontSize(11)` 从来没有生效过；模板内层 `InnerHeaderGrid` 又写死 `Height="40"`，14 DIP 的文字在 40 里垂直居中，外层压到 24 就从中间截断。`Padding` 同样没用：模板根不读它，水平边距走 `NavigationViewItemInnerHeaderMargin`。

2. 「维护与诊断 › 连接 AI 助手」的 MCP 配置框里只有一个字符「`{`」。`append_mcp_section` 先 `code.Text(snippet)` 再 `code.AcceptsReturn(true)`，而 TextBox 默认是单行，单行状态下写入多行文本会被截到第一个换行——`config_snippet` 的 `serde_json::to_string_pretty` 第一行正好只有 `{`，之后再打开多行也找不回来。host-api 这一侧是好的：直接调 `msime_client_mcp_status` 拿到的 `config` 是完整的 265 字符。

3. 同一张卡片的内容比同页其他组向右错开一整个缩进，两个助手是「小标题 + 路径 + 下面一个按钮」堆两遍。`add_group` 返回的已经是卡片里的行面板（自带卡片和 12 DIP 内边距），这段却又套了一张 `make_card` 和 25 DIP 内边距；小标题用 14 DIP，比 13 DIP 的组标题还大。

## Decision

分组标题覆盖 `NavigationViewItemInnerHeaderMargin` 为 `{16,-8,16,-8}`，把模板写死的 40 DIP 收进 24 DIP 的行高里，文字整个落在其中；不在 header 实例上设 `FontSize` 和 `Padding`，原处留注释说明模板不读它们。

`AcceptsReturn` 在 `Text` 之前设；配置框给 `MinHeight(188)`／`MaxHeight(280)` 和两个方向的滚动条，长片段在框内滚动。

「连接 AI 助手」的内容直接进 `add_group` 的行面板，不再套第二张卡片；小标题 13 DIP、说明 11 DIP，与 `add_row` 和 `README` 记的密度一致；两个助手改用 `add_row`，左边名称与配置文件路径、右边写入按钮，窄窗口时按钮落到标签下面，和本页其他行同一个形状。

### 这个窗口里哪些模板值能覆盖

`view.Resources()` 里插入的键只对模板中的 `{ThemeResource ...}` 生效——它在运行时沿元素树向上查找，所以 `NavigationViewItemOnLeftMinHeight`、`NavigationViewItemInnerHeaderMargin` 这类都能改。`{StaticResource ...}` 在 `generic.xaml` 解析时就地绑定完毕，页面资源够不着，`NavigationViewItemHeaderTextStyle` 属于这一类：**never** 指望用资源字典改它的字号，要改只能整份换掉 `Template`。控件实例上的本地值（`Foreground`）优先于样式 setter 照常生效，被样式显式设过的属性（`FontSize`）则不会。

## Alternatives considered

- **分组标题放弃固定高度，用模板的 40 DIP** — 一行代码，立刻不裁。否决：六个分组各长 16 DIP，1000×760 的窗口里侧栏要滚动才看得到「关于」，而 24 DIP 的紧凑密度是这个窗口照 macOS 设置定下的风格（`settings/README.md` 的「13 DIP 标签和 11 DIP 说明」）。
- **用 `XamlReader` 换掉 `NavigationViewItemHeader` 的 `ControlTemplate`** — 只有这样才能把字号真正做到设计要的 11 DIP。否决：为一个分组标题引入运行时 XAML 解析，外加一份要跟着 Windows App SDK 升级维护的模板副本；14 DIP 在这个位置可接受。
- **把 `config_snippet` 的换行改成 CRLF** — 最初以为「只显示 `{`」是 XAML 只认 `\r` 造成的。实测否决：改完照旧只有一个字符，真因是属性顺序；而且那会动到 macOS 和 Linux 共享的 host-api 输出。修好顺序后 UIA 读出的文本是 266 字符、行尾为 `\r`，证明 XAML 自己会把 `\n` 规范过去。
- **留着嵌套卡片，只把内层内边距调成 0** — 改动更小。否决：外层卡片的圆角和背景被内层再画一遍，同色时看不出来，换主题或改卡片色就露馅。

## Consequences

- **收益**：六个分组标题完整；MCP 配置可读、可复制；这张卡片的左缘、标签字号和行形状与同页其他组一致。
- **代价**：分组标题固定在 14 DIP，比设计意图大，要改只能换模板。`{16,-8,16,-8}` 依赖模板里 `InnerHeaderGrid` 仍是 40 DIP——这是对 Windows App SDK 内部布局的一处取值假设，不是公开契约。
- **重访信号**：升级 Windows App SDK 后看一眼侧栏分组标题是否又被裁、配置框是否还完整；`InnerHeaderGrid` 的高度一变，负边距就要重算。
- **缺口**：这一页没有能发现这三类问题的自动化测试，`tests/ui/settings_navigation.cpp` 只核对导航的页面与分组结构。排版回归仍然只能靠打开窗口看。

## Verification

Windows 11、Visual Studio 2022 生成工具 17.14、Windows App SDK 1.7.250606001，`RelWithDebInfo` 构建后打开真实窗口（`--route=settings:dev` 直达该页）逐项对照：六个分组标题完整；配置框经 UIA `ValuePattern` 读出 266 字符的完整 JSON（修复前是 1）；卡片内文字左缘与上面「导出设置」对齐；两个助手各占一行、按钮在右。
