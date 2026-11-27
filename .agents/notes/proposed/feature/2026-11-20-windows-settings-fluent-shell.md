# Agent Note: Windows 设置窗口的外壳改回 WinUI 3 的 Fluent 样式

Status: proposed

## Problem

[外壳配色与侧栏对齐 macOS](../../implemented/feature/2026-10-20-windows-settings-mac-shell.md) 把 WinUI 3 设置窗口的外壳逐项改成了 macOS 的数值：不透明的灰色侧栏和正文、去掉 NavigationView 的选中指示条和正文格圆角、导航项 38 DIP 高 15 DIP 图标、文字和卡片用 Apple 的色值。结果是控件行为是 WinUI，外观却既不像 macOS（没有半透明材质）也不像 Windows 11。Windows 端维护者在 2026-11-20 提出「设置界面有很多地方不是那么 WinUI 3」，决定 Windows 端优先像 Windows，先做一版原型对比。

## Proposal

第一步只动外壳和中性色，品牌绿强调色、页面布局和行组密度不动：

- 窗口在 Windows 11 上铺 `MicaBackdrop`（`MicaController::IsSupported()` 为真时），根网格和标题栏透明；不支持 Mica 的系统铺 `SolidBackgroundFillColorBase`（`#F3F3F3` / `#202020`）。
- 删掉标题栏正文列的底色和细线，整条标题栏透出 Mica。
- 删掉 `make_navigation` 里对 NavigationView 模板资源的覆盖，只保留侧栏底色透明；选中指示条、导航项尺寸、正文格的 LayerFillColorDefault 底色和左上圆角回到模板默认值。导航项不再单独设字号、高度、圆角和边距，图标 16 DIP。
- `make_palette` 的中性色换成 Fluent 主题 token 的值：`text`／`sub`／`faint` 是 TextFillColorPrimary／Secondary／Tertiary，`card`／`stroke` 是 CardBackgroundFillColorDefault／CardStrokeColorDefault，`divider` 是 DividerStrokeColorDefault，`hover` 是 SubtleFillColorSecondary。删除只给 macOS 外壳用的 `content_bg`、`edge`、`title_edge`。

调色板仍是代码里的常量，没有改成 `{ThemeResource}`：这个工程在代码里构建界面，`Application::Resources().Lookup` 取到的是应用主题的值，不跟随根元素的 `RequestedTheme`（窗口可以独立于系统选浅深色）；常量与 token 同值、随 `make_palette(dark)` 重建，效果相同。

后续各步看原型对比再定：页面大标题放回正文、卡片和行照 Windows 11 设置的 SettingsCard 规格（4–8 DIP 圆角、行首图标）、「更多选项」换回 Expander、强调色是否跟随系统。

## Alternatives considered

- **用 msimeui 自绘设置窗口**：外观完全可控，还可能合并 Win7 和现代两套设置。否决：msimeui 缺 ScrollViewer、NavigationView、ContentDialog、InfoBar 等十来种控件，读屏、键盘焦点也要自己补，工作量是 msimeui 现有规模的一到两倍；外观问题本身是资源覆盖造成的，撤掉覆盖就能解决。
- **保持 macOS 外观，只补 Mica**：改动最小，跨平台视觉一致。否决：侧栏选中样式、正文格和配色仍是 macOS 的，用户提出的「不像 WinUI 3」大部分来自这些地方。
- **配色全部改成 `{ThemeResource}`**：高对比度主题能自动生效。暂不采用：原因见上面调色板一段；要做得经 `XamlReader` 或 Style 设属性，改动面大，留到确定走 Fluent 之后。

## Acceptance criteria

- Windows 11 浅色、深色下侧栏和标题栏透出 Mica，选中项有强调色竖条，正文格左上有圆角。
- Windows 10 上窗口是不透明底色，没有黑底或透明穿透。
- `tests/tools/settings_ui.ps1` 全部通过，截图与改动前逐页对比。
- 维护者看过对比图后决定继续还是撤回；继续时把[外壳对齐 macOS](../../implemented/feature/2026-10-20-windows-settings-mac-shell.md)里被取代的部分互链，并同步 `platforms/windows/settings/README.md` 的视觉规格。

## Risks

- `palette_.card` 变成半透明。[外壳对齐 macOS](../../implemented/feature/2026-10-20-windows-settings-mac-shell.md)当时正是因为卡片色还用在主题卡片、行背景等不一定叠在正文色上的表面，才放弃半透明；这些表面现在都在正文格里，底色一致，但要逐页看截图确认。
- 侧栏选中竖条取强调色资源；根网格上 `apply_accent_resources` 换成了品牌绿，竖条可能是绿色而不是系统强调色。
- 页面布局（页首插画、标题栏页名、10 DIP 卡片圆角、「›」跳转行）仍是 macOS 的，这一步之后会是两种风格混合的过渡状态。
