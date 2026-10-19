# Agent Note: 候选窗、候选菜单和托盘卡片按窗口客户区排版，不按交换链尺寸

Status: implemented

## Problem

[候选皮肤截图](../testing/2026-10-19-windows-candidate-skin-gallery.md)第一次在 Windows runner 上跑出来，十款测试皮肤里大半的卡片右边（有的连下边）被窗口裁掉：横排的列表翻页箭头和卷轴展开的「∨」按钮整个在窗口外，竖排的外框和圆角缺了右边。不带皮肤的卡片看着正常，但右边的阴影没了，下边的阴影只剩 8 像素（应有 40）。

原因在 msimeui 的 `DeviceResources`：合成交换链的宽高按 64 像素分档（`AlignSwapExtent`），而且只增不减，Direct2D 的渲染目标就是整个交换链缓冲区，所以 `GetRenderTarget()->GetSize()` 常比窗口客户区大，最多大 63 像素，卡片变小之后还会大得更多。`CandidateWindow::paint` 却按 `target->GetSize()` 推卡片外框（`candidate_shadow_frame`），而窗口的尺寸是 `card_bounds` 按 `candidate_card_size` 定的。两边不一致，卡片按缓冲区画，比窗口宽出「分档后的宽度减窗口宽度」，DirectComposition 把超出窗口的部分裁掉。截图里的数字对得上：宽 576（正好 9 × 64）的窗口一张都没被裁；592 宽分到 640，多出 48，超过右边 32 的阴影边距，卡片被裁 16 像素；224 宽分到 256，多出的 32 正好吃掉右阴影；高 208 分到 256，多出 48 超过下边 40 的边距。皮肤只是让窗口尺寸落在了分档的不同位置上，没有皮肤时一样会发生：用户先打出一页长候选、再打一页短的，缓冲区留在长的那一档，短卡片就被画成长卡片的宽度、右边整段裁掉。

这和截图工具套用皮肤的顺序无关：工具和 Server 启动时一样先建窗口、调完 setter 再刷新；Server 运行中换皮肤时（`set_theme_palette` 加各个 skin setter，再由主循环刷新）缓冲区同样只增不减，只会更明显。候选菜单（`CandidateFlyoutWindow`）和托盘卡片（`TrayMenuWindow`）的外框也是按 `target->GetSize()` 画的，同一个毛病；悬浮状态栏、输入模式提示、双拼键位图和语音浮层不读渲染目标尺寸，不受影响。

## Decision

`DeviceResources::GetClientSize()` 返回窗口客户区按当前绘制 DPI（`DpiForHwnd`，含用户缩放的 DPI 覆盖）换算成的 DIP 尺寸，和渲染目标坐标同一个单位。候选窗、候选菜单和托盘卡片画外框时用它，不再用 `target->GetSize()`。交换链照旧分档、只增不减，省掉的重新分配还在；超出客户区的那部分缓冲区画不画都看不见。

候选窗另记下上一帧实际画出的卡片矩形，`CandidateWindow::painted_card()` 只读地交出来（物理像素、客户区坐标），Server 不用；截图工具拿它和客户区、`card_on_screen()` 比，以后画图与定窗口再用两个尺寸时 CI 会失败。

## Alternatives considered

- **交换链不分档，每次按客户区精确 `ResizeBuffers`** —— 渲染目标就等于窗口，所有调用方都不用改。不采用：分档是为了打字时卡片宽度每变一点就重新分配缓冲区、重新绑定 D2D 目标的开销，候选窗每个按键都可能变宽；而且只增不减也是为了来回变宽时不抖动。只让读尺寸的三处改读客户区，代价最小。
- **在 `paint` 里用 `PushAxisAlignedClip` 把多出的部分裁掉** —— 不画到窗口外。不采用：裁掉的正是卡片右边和下边的外框、圆角和箭头，问题是卡片排错了尺寸，不是画多了。
- **只修候选窗** —— 这次截图只拍到候选窗。不采用：候选菜单和托盘卡片是同一行代码、同一个原因，它们的外框和圆角同样会缺右下角。

## Consequences

收益：卡片、外框、圆角、列表翻页箭头和「∨」按钮都画在窗口里，阴影边距回到设计的 32/20/32/40；候选菜单和托盘卡片的外框也完整。不带皮肤时的排版算式不变，只是画出来的卡片尺寸回到 `card_bounds` 本来定的那个，右边和下边的阴影重新出现。

代价：多一次 `GetClientRect`；`DeviceResources` 多一个公开方法，`CandidateWindow` 多一个只给截图工具用的只读方法。

验证：截图工具对每张截图检查 `painted_card()` 落在客户区里、和 `card_on_screen()` 每边相差不超过 2 像素，并检查截图最外一圈是底色（`edge_bleed`），在 PR 的 Windows candidate skin gallery 作业上运行；`windows-candidate-skin-gallery-plan` 覆盖这两项判断。候选菜单和托盘卡片没有截图，只做了 MinGW 语法检查。
