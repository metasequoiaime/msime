# Agent Note: Windows 候选窗绘制 canvas 皮肤的九宫格面板与自由挂件

Status: implemented

## Problem

共享 catalog 早已按 msime-windows `c0a9d371` 解析 `base = "canvas"` 的 `[candidate_window.border_image]`、`content_insets_dip`、`outer_insets_dip`、`line_gap_dip` 和 `[[candidate_window.decorations]]`（[清单协议对齐](2026-10-19-windows-skin-schema-parity.md)），网页、macOS、iOS、Android 和 HarmonyOS 也都画了出来，Windows 原生候选窗却只画共享层给的不透明 `canvas` 外观：为 msime-windows 写的全贴图皮肤在这里没有面板、没有挂件，内容也不按内边距收进面板。这是 msime-windows 皮肤视觉兼容的第三步（前两步见 [版式等键](2026-10-19-windows-candidate-skin-layout-keys.md) 与 [卷轴展开](2026-10-19-windows-candidate-skin-expand-panel.md)）。约束是不是 canvas 的皮肤和没有皮肤时，卡片要和以前逐位相同；同时有另一项工作在改 msimeui 的 `DeviceResources` 与卡片底色（系统材质），这一项只动 canvas 的绘制路径。

## Decision

`CandidateSkin.h` 的 `candidate_skin_canvas` 从 `SkinSummary` 读出画布（图片路径按包内相对路径解析，过同一道不出包、不经符号链接、文件存在的检查），交给 `CandidateWindow::set_skin_canvas`。几何全在不依赖 Win32 的 `CandidateCanvas.h`，含义照 msime-windows 的 candidate_skin_layers、candidate_skin_overlay、candidate_skin_scene 和 msimeui 的 DrawNineSlice。

### 面板

- 切片 `slice_dip`（写了 `stretch_dip` 时用它）以源图像素计，边宽 `width_dip` 以 DIP 计、没写时取（盖过之后的）切片数值，`fill = false` 不画中心块；一、二、四值按 CSS 四边展开。超出图片的切片按整张图算，对边的宽放不下时四边同比缩小，左右（上下）切片交叉时中间一列（行）为空。
- 每块的边按渲染目标的 DPI 和当前变换对齐设备像素，块用 `D2D1_EXTEND_MODE_CLAMP` 的图像画刷画：DrawBitmap 缩放时会采到源矩形外，把相邻块的颜色渗进接缝。拿不到 `ID2D1DeviceContext`（分层窗口回退）时退回 DrawBitmap。
- 浅色主题有 `light_source` 时换那张图，按卡片实际的明暗（主题固定的明暗优先）选。面板画出来时它就是卡片：底色、背景图、外框和阴影都不画（同 CSS `border-image` 与 msime-windows 的 `panelImage`）；用户的不透明度只淡化面板，不淡化挂件和文字。打字闪光照旧画在面板上。

### 内容内边距与行距

msime-windows 让卡片内边距等于 `content_insets_dip`（每边不小于边宽），没写时等于边宽加基底内边距。本宿主的版式（`candidate_card_metrics`）照旧按自己的内边距排，`candidate_canvas_content_offset` 算出面板比这张卡片四边各多出的距离（可以是负数），整块内容挪进面板。行、首行、点击、悬停提示和读屏仍用卡片坐标，不必改；阴影、底色、吉祥物装饰图和挂件对着面板画。最小宽度量的是整块面板（`CandidateCardInput::canvas_extra_width`），和 msime-windows 把最小宽度设在带内边距的卡片上一致。`line_gap_dip` 是拼音行与候选之间的距离，落在拼音行的下外边距上（只在显示拼音时），盖过版式表的 `preedit_margin_dip.bottom`。

### 挂件与窗口空白

- 挂件的框（`width_dip` × `height_dip`）按九种锚点贴着面板再加偏移，图片在框里等比缩放居中；按 `z_index` 稳定排序，低于 1 的在面板和阴影之下，其余在候选文字之上。随进出场动效淡入淡出，不拦点击（命中测试本来只看候选行）。动图播放由后续[图片动画](2026-10-19-windows-skin-image-animation.md)补齐：APNG 与 GIF 按有界合成后的逻辑画布逐帧播放，WebP 仍依赖 WIC 编解码器。
- 窗口在阴影边距之外照 msime-windows 再留 `outer_insets_dip`；挂件的框伸得比它更远时按挂件留，每边至多 500 DIP（`outer_insets_dip` 的上限），工作区放不下时只按比例收小这部分。`card_bounds` 每次排版算出 `shadow_insets_`（内容卡片四周的空白）与 `canvas_offset_`，窗口仍按面板贴光标摆放；`card_on_screen()` 与 `painted_card()` 报的是面板，截图工具的「卡片没画出窗口」自检照样成立。

### 解码与配色

套用皮肤时 `CandidateCanvasImages.h` 用和 `DeviceResources::GetBitmapFromFile` 相同的 WIC 路径完整解码面板和每个挂件的第一帧（`candidate_canvas_drawable`）。面板解得出来，Server 才在 `msime_client_resolve_theme` 请求里带 `canvas_panel_drawn`，取共享层保留的贴图原配色（`panelCandidate`，取色器仍优先）；解不出来时面板留空，卡片退回不透明 `canvas` 外观的可读配色。解不出来的挂件不画。两种情况每款包在诊断日志里各记一次。工具条、菜单、语音浮层和双拼键位图不画面板，始终用回退配色。静态位图由 `GetBitmapFromFile` 按路径缓存，动画的 CPU 快照与当前 GPU 帧归[图片动画缓存](2026-10-19-windows-skin-image-animation.md)；设备丢失时 GPU 资源随 `DiscardTarget` 清掉、下一帧重建，皮肤重载时随 `invalidate_skin_images` 清掉；明暗切换时重新选图、重新确认。

共享 catalog 的 `CANVAS_RENDERER_NOTICE` 不点名宿主，`fill_from_canvas_look` 的回退只在宿主没说画出面板时使用，都不用改，其他宿主的行为不变。

## Alternatives considered

- **照搬 msime-windows 的 `outer_insets_dip` 语义，挂件超出外留白就被窗口裁掉** —— 与来源逐像素一致，窗口也不会因为一个偏移写得很远的挂件变大。不采用：挂件的框在排版时就算得出来，按它多留空白不需要解码图片；没写或写小了外留白的包（自制皮肤很常见）在这里不会被裁掉半个挂件，写对了的包结果与来源相同。代价是这类包的窗口比来源大一些，所以加了每边 500 DIP 的上限和工作区收缩。
- **把内容内边距直接写进 `CandidateCardMetrics`（改 pad_x、pad_y、slack_y）** —— 少一层坐标换算。不采用：pad_x 同时决定行缩进、首行翻页的位置和收紧宽度的对称留白，内边距四边不等时没法用一个 pad_x 表达；整块平移只动卡片外框和窗口空白，现有的版式算式与点击、读屏都不用改，也就不会碰到非 canvas 皮肤。
- **画的时候再决定配色（GetBitmapFromFile 成功才切到贴图原配色）** —— 不用在套用皮肤时多解码一次。不采用：配色由 Server 经共享层解析，候选窗画的时候拿不到 `panelCandidate`，而面板失败时一帧白底浅字比多解码一次更糟；在套用时确认还能顺带挡掉坏挂件，否则 GetBitmapFromFile 不缓存失败，每一帧都会重新解一次。
- **挂件和面板播放动图帧** —— 来源用 `GetImageFrame` 按动画时钟换帧。不采用：要给 msimeui 加逐帧解码与换帧定时器，正和材质那项工作改同一个文件；当时本宿主的装饰图也只画第一帧，留到装饰图一起播放时再做。该并行修改约束已解除，由后续[图片动画决定](2026-10-19-windows-skin-image-animation.md)取代首帧妥协，几何与配色决定保留。

## Consequences

收益：为 msime-windows 写的全贴图皮肤在 Windows 候选窗上得到九宫格面板、按明暗换图、内容内边距、拼音行行距、面板上下两层挂件和外留白，配色用包为贴图挑的原色；Windows 不再是用不透明回退的宿主，只剩 Linux。非 canvas 皮肤与无皮肤时所有新路径都关着（`candidate_canvas_margins` 原样返回阴影边距，`canvas_extra_width = 0` 时卡片尺寸逐位不变）。

代价：
- 动图的首帧与 GIF 逻辑画布缺口由[图片动画](2026-10-19-windows-skin-image-animation.md)补齐；系统没有 WebP 解码器时 WebP 挂件仍不画。
- 挂件伸出面板时窗口变大，透明的空白仍挡着它下面的窗口的点击（与阴影边距相同）。
- 套用皮肤和切换明暗时多解码一次面板与挂件（包内像素上限 800 万）；面板画出来时卡片多一次带 `canvas_panel_drawn` 的主题解析。
- 原生设置卡片与社区导出的静态画布由[设置预览](../bug-fix/2026-10-20-windows-canvas-settings-preview.md)接入，真实候选窗仍独立负责焦点、定位、动画和系统材质。

验证：新加的 `windows-candidate-canvas`（读取与越界、按明暗选图与解码过滤、内容内边距与基准内边距、行距、九种锚点与等比缩放、分层、窗口空白与上限和收缩、九宫格的切块与越界切片与同比缩小与像素对齐、面板最小宽度）以及 `windows-candidate-card-size`、`-skin`、`-skin-layout`、`-expand-panel`、`-shadow` 在 Mac Studio 上用 `c++ -std=c++17 -Wall -Wextra -Werror` 编译通过；`CandidateWindow.cpp`、`server_main.cpp` 和截图工具 `main.cpp` 用本机 `x86_64-w64-mingw32-g++ -fsyntax-only -Wall -Wextra -Werror` 检查通过。新验证皮肤 `verify-canvas-frame` 补上单张面板、`stretch_dip` 两值简写、缺省 `width_dip`、`fill = false`（中心涂成品红，截图里出现就是错）、没写外留白时挂件伸出面板和 GIF 挂件，`cargo test -p msime-client-services skin::` 断言它的解析与 GIF 逐帧解码。Direct2D 的实际画面靠 PR 上 Windows candidate skin gallery 的截图核对。
