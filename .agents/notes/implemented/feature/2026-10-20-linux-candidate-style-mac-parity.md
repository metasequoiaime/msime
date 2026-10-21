# Agent Note: Linux 候选窗的几何、文字与样式向 macOS 对齐

Status: implemented

## Problem

Linux 候选窗的 UI/UX 与 macOS 差得最多的是每个用户都会走的两条路径：Fcitx5 经典界面（classicui）的生成主题，以及 IBus 的 lookup table 属性。差异逐项列在一次差异审计里，归到这一批的有：

- 序号写成「1. 」、与候选同样大；注释是「  ab」（两个空格、不加括号）；云和 AI 角标两个空格，IBus 上还是「云」「AI」文字；固定排位的行用主题 accent 着色。macOS 是不带句点的 0.8 倍序号、紧贴候选字的「(ab)」、一个空格的 ☁️ 🤖、固定色 `#379AD3`。
- 关掉 `show_candidate_page_number` 只去掉辅助行的页码，classicui 的 ‹ › 还在；macOS 两者一起隐藏。
- 卡片几何是早先 Linux 设计稿的（6px 内边距加 1px 细边、行边距 10/12/6），macOS 是 4px 内边距（细边在其中）、行边距 8/8/6。
- 「整体大小」只放大字体，圆角、内边距、阴影和图片不随之变化；不透明度和内边距在 Linux 根本不提供；`show_app_logo` 被忽略，logo 总是烘进主题。
- 皮肤的背景图、行圆角 `item_corner_radius_dip`、`[toolbar]` 配色在 Linux 都不生效，共享目录也不发布它们。
- 右键候选操作的文案和顺序与 macOS 不同；正在组词时已选段和读音画成同一条下划线。

产品决策是：Linux 的 `system` 候选主题保留 Adwaita 配色（蓝色选中），几何、字号、排版和交互对齐 macOS；画不出来的路径要优雅退化，不能让 Wayland 或 IBus 变差。原生 X11 画布成为默认渲染器是另一项工作，叠在这一批之上。

## Decision

- **一处定义行文字。** `src/candidates/CandidateRowText.h` 给两个前端共用：`candidate_annotation_text` 照 macOS 的 `MSIMEParenthesizedAnnotation` 把注释括起来紧贴候选字（已带半角或全角括号的原样保留），`candidate_source_badge` 给一个空格的 ☁️ 🤖，`candidate_index_label` 给与数字键一致的序号（第十个是 0）。Fcitx5 的序号后跟一个空格代替 macOS 的 1.5pt 间距，主题写 `LabelTextSizeFactor=80`。固定排位的行照 macOS 画 `#379AD3`，不再用皮肤的 accent：IBus 未选中时画这个颜色并保留行尾「固定 N」（GNOME Shell 的候选弹窗不认 IBus 的颜色属性，颜色画不出来时文字仍能说明这一行被固定了）；Fcitx5 原生画布未选中时把正文画成这个颜色；classicui 的候选文字没有颜色属性，正文后加只供显示的「  固定N」（`candidate_fixed_position_marker`）。脚本插件注释仍是「〔…〕」，classicui 和 IBus 都没有第二行可以斜体显示。
- **classicui 主题的几何改成 macOS token。** `FcitxPanelGeometry` 存 100% 时的长度，`fcitx_panel_metrics` 按 `candidate_scale_percent` 缩放出实际值。内容到卡片外沿的距离照 macOS 的 `CandidateInset` 取描边宽度与内边距中较大的一个。行圆角照 `StyledCandidateSkin` 推导（用户圆角减内边距、只调内边距时的下限、否则皮肤包的行圆角或 6px），但不超过行的上下文字边距：classicui 高亮的 `Margin` 同时是九宫格固定切片和高亮伸出文字的距离，二者不能分开。被截短时 macOS 靠同心保证的「高亮不伸出卡片圆角」不再成立，`fcitx_card_inset` 就把内边距加大到对角线上高亮圆弧不越过卡片圆弧（用户圆角 32 时 4px 变 8px），而不是让首行、末行的高亮角画到阴影上。
- **用户样式写进主题（`FcitxCandidateStyle`）。** 缩放乘到阴影以外的所有长度和 logo 上（logo 从更大一档的图标用 cairo 缩放）；阴影边距、偏移和模糊不缩放，因为 Wayland 上 classicui 不读 `ShadowMargin`，阴影边就是候选窗离光标的偏移；`candidate_padding` 代替 4px；不透明度只乘卡片底色和描边的 alpha，卡片下的阴影被擦掉，并打开 `EnableBlur`、给出卡片形状的 `BlurMask` 和 `BlurMargin`。`host_surface.rs` 因此对 Linux 声明 `candidate_window_opacity`、`candidate_padding`、`app_logo`，IBus 宿主仍经 `candidate_panel_limit = ibus_panel` 让设置页说明这些在 IBus 下不生效、整体大小只缩放字号。
- **`show_candidate_page_number` 关掉时不生成 ‹ › 两张图，** classicui 只在两张图都在时才画翻页按钮；原生画布按同一偏好不画 pager。设置页 Linux 上的行名随之改成「显示页码和翻页按钮」。`show_app_logo` 关掉时主题不烘 logo，偏好和样式都进主题输入指纹，切换开关会重画主题。
- **共享目录多发布三样东西，** `host_candidate_catalog` 的条目加 `item_corner_radius_dip`；每个条目另带背景图的绝对路径与 `background_fit`、`background_opacity`，以及按声明明暗分组的 `toolbar` 配色（不含菜单用不到的 `handle`）。起初只给自定义主题选中的皮肤发布这两样，但 Fcitx5 的「主题」菜单换皮肤时只存偏好、不重写运行时选项，从菜单换上的皮肤和升级前写下的旧文件就读不到它们，所以改为每个包都发布。整份运行时选项只有 16 KiB，放不下时 `runtime_options_with_skin_catalog` 先从末尾去掉没选中的包的这几样（`HOST_CATALOG_DRAWN_EXTRAS`），仍放不下才整包丢弃，已装的皮肤不会因为它们从菜单里消失。`ThemePackage::from_host_catalog_entry` 的严格读法把它们列为忽略键，条目原样交回主题解析不受影响。
- **背景图画成 classicui 背景的叠图。** 宿主用共享层的解码器读图，按 `fit` 画进一张按布局估的卡片大小的 PNG（竖排约 240×280、横排约 560×80，随缩放变化），乘上两层不透明度，居中贴在卡片上，裁剪四边各收进描边宽度和圆角弧高。背景与装饰共用唯一的叠图，装饰图画得出来时背景图让位；canvas 包不画背景图。
- **`[toolbar]` 配色盖过 classicui 菜单从候选调色板推出的槽位**（background→底色、icon→文字、hover→悬停、border→外框、divider→分隔线）。Linux 不画悬浮工具栏，它的功能都在输入法菜单里，这就是 macOS `ToolbarSkinTokens` 在 Linux 上的落点。
- **候选操作照 macOS 的顺序和文案：** 置顶、固定到第 1–5 位、取消固定、删除。Linux 没有子菜单，固定排位平铺；macOS 的「取消置顶」来自它本机的置顶表，Linux 的置顶交给 Engine，没有状态可查，始终是「置顶」。
- **组词中的预编辑分成两个子句：** IBus 已选段单下划线、读音双下划线（与整句改字的焦点子句同一种画法），Fcitx5 已选段下划线、读音下划线加 `HighLight`，与整句改字的 Fcitx5 焦点子句同一种画法。没有用 `Bold`：Fcitx5 没有粗下划线这种格式，`Bold` 在 fcitx5-gtk（Pango 字重属性）和 fcitx5-qt（`QFont::Bold`）里是字形加粗，会把读音的字母画粗画宽，而 macOS 两段都是常规字重、只是下划线粗细不同。

## Alternatives considered

- **Linux `system` 主题也换成水杉品牌绿：** 与 macOS 像素一致，改动只是几个 token。维护者明确要求 Linux 保留 Adwaita 配色，只对齐几何与交互，不采用。
- **背景图烘进卡片的九宫格中心：** 不需要占用叠图，装饰图和背景图可以同时出现。classicui 的九宫格中心按卡片大小在两个方向各自拉伸，固定切片又要留给圆角和阴影，图片会被拉变形，照片类背景完全不可用；叠图虽然只能按估计的卡片大小排，至少不变形，所以选叠图。
- **背景图只在原生 X11 画布里画：** Cairo 可以按实际卡片尺寸精确铺排。但原生画布目前只接管 canvas 包，也只在 X11 上存在，Wayland 和大多数用户仍走 classicui，这样做等于大多数人仍看不到背景图。
- **classicui 序号的间距用细空格：** 宽度更接近 macOS 的 1.5pt。各字体里细空格的宽度差别比普通空格大，有的 CJK 字体甚至没有这个字形会回退到别的字体，普通空格更稳定。
- **IBus 去掉「固定 N」文字，只靠 `#379AD3`：** 与 macOS 完全一致。GNOME Shell 是最常见的 IBus 面板，它不画颜色属性，去掉文字后那里就看不出哪一行被固定了，不采用。
- **原生画布把右键菜单的「固定排位」做成第二级列表：** 更像 macOS 的子菜单。平铺的「固定到第 N 位」已经把动作写全，第二级只多一次点击，留给把原生画布做成默认渲染器的那项工作去决定。

## Consequences

- **收益：** 每个 Fcitx5 用户的候选卡片、序号、注释、角标、翻页箭头、logo 开关、缩放、内边距和不透明度都和 macOS 一致或接近；皮肤的背景图、行圆角和工具栏配色第一次在 Linux 生效；IBus 用户得到同样的行文字、固定色和菜单顺序。
- **代价：** classicui 的背景图是按估计尺寸排的一张固定图，卡片比它大时露出底色、比它小时只看得到中间；有装饰图的皮肤不画背景图。没有合成器的 X11 上，调低不透明度的卡片会发黑。只有 KWin 会模糊半透明卡片的背后。`LabelTextSizeFactor` 只在 Fcitx5 5.1.18 起生效，更早的版本序号与候选一样大。
- **行为变化：** Linux 开始读 `show_app_logo`。新装默认不显示 logo（共享偏好的新装默认值）；已存偏好缺这个键时照旧显示，但共享层从 #5631 起把默认值 `false` 写进每一份保存过的偏好（这个字段没有 `skip_serializing_if`），而 Linux 以前不读它、总是画 logo，所以升级后大多数现有 Linux 安装的 logo 会消失，用户没有改过任何设置。这是有意接受的：结果与 macOS 新装默认隐藏一致，设置页现在有开关可以打开；一次性迁移需要在共享偏好里加迁移标记，影响所有平台，不值得为一个 logo 做。Linux 设置页多出不透明度、内边距和 logo 三个控件。没写 `content_insets_dip` 的 canvas 包仍用原先的 7px 内容留白，不跟着原生卡片改成 4px。
- **没有覆盖的：** macOS 把第十个候选标「10」而 Linux 和数字键一致标「0」，这是 macOS 一侧的问题，这里不改。macOS 的折行、宽度上限和最小宽度在 classicui 里没有对应项，留给原生画布。
- **相关笔记：** 原生画布成为 X11 默认画法的那一批见 [X11 原生画布默认](../architecture/2026-10-21-linux-x11-native-candidate-default.md)；[Fcitx5 跟随系统保留水杉候选样式](2026-10-09-fcitx5-system-theme-keeps-brand-style.md) 说的品牌标志现在受 `show_app_logo` 控制；[macOS 紧凑候选窗](2026-10-12-macos-candidate-page-controls-toggle.md) 的页码与翻页按钮开关在 Linux 有了同样的含义。
