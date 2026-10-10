# Agent Note: Fcitx5 装饰图按可用高度缩小，九宫格切片容纳整条描边

Status: implemented

## Problem

皮肤顶部装饰图交给 classicui 的 overlay 后有四类缺陷（PR #4371 评论 r4203158959 / r4203158963 / r4203158964 与计划第四节的 4.5 / 4.6）：

- **按宽度缩放后仍可能过高。** #5226（`69f0237b4`）已把 PNG 装饰图按 `width_dip` 缩放成 1x/@2x，但只按宽度求比例；声明尺寸较小的皮肤仍会高出「预留带宽 + 卡片内边距」，整图可见的条件不成立。
- **rc4203158959 缩放失败丢装饰**：基线（#5226）已经在缩放返回 `nullopt` 时退回原图，不需要改实现，但缺测试固定。
- **r4203158963 卡片内边距重复计算**：`fcitx_candidate_theme_files` 里 `inset + G::padding` 与放置/缩放各写一份。
- **r4203158964 PNG 宽度解析重复**：签名/IHDR/范围校验散落，读宽只读一份。
- **4.6 九宫格边框进拉伸中心**：slice 只按 `radius`，`radius = 0`、`border_width > radius` 时边框色落进被拉伸的中心，整块卡片被拉成边框色。

## Decision

- **PNG 尺寸集中解析**：`fcitx_png_size(bytes)`（`CandidateFcitxTheme.h`）校验 8 字节签名、偏移 12 的 `IHDR`、宽高各在 `1..kFcitxOverlayMaxSide`（=2048，对应共享层 `skin::catalog::MAX_IMAGE_SIDE`）；`fcitx_png_width`/`fcitx_png_height` 是它的薄包装。超限即返回 `nullopt`，因此声明超限的图根本不进 cairo，不会为小文件分配巨量像素。
- **可用高度缩放**：`FcitxOverlayScaler` 从 `(png, width)` 扩为 `(png, width, room)`；`scale_fcitx_overlay_png` 先按 `width` 求比例，再与 `room / source_height` 取小（`room <= 0` 不限制高度）。`stage_fcitx_overlay` 用 `room = ceil(top_dip) + padding`，@2x 传 `2 * room`，因此 @2x 正好是 1x 的两倍。几何依据：`OverlayClipMargin.Top = shadow_top`、`OverlayOffsetY = shadow_top + pad - height`，整图可见即 `height <= band + pad`。
- **共享内边距**：`fcitx_card_padding(colors) = max(1, border ? max(0, border_width) : 0) + FcitxPanelGeometry::padding`，放置（`content_left`/`logo_top`/`OverlayOffsetY`/`OverlayOffsetX`）与缩放可用高度共用同一个值。
- **九宫格容下整条描边**：`edge = max(radius, border_width)`，`slice_left/right/top/bottom` 由它决定（左上切片在有关标志时仍取标志边界）。直角卡片拉伸中心只剩填充色。
- **回退**：非 PNG、读不出尺寸、声明尺寸超限、1x 或 2x 任一缩放失败都退回原图，装饰不丢；退回的图读得出高度时按底边定位，读不出时从带顶按原尺寸画。

## Alternatives considered

- **只在缩放器里保留按宽度缩放** — 改动最小；但声明预留带小于缩放高度的皮肤（如 acg-idol 例）装饰仍被裁，正是本轮要修的缺陷。
- **为读尺寸引入图片解码库** — 能拿到任意格式的尺寸；但仓库不新增依赖，且 cairo 已在 addon 里链接，PNG 头解析足够覆盖通过校验的皮肤资源，其他格式原样退回。
- **把九宫格切片改成整段边框色填充** — 能避免边框进中心；但直角/小圆角的卡片圆角与阴影几何会一起变形，改 `edge` 能保持设计几何，只在边框宽时才扩大固定切片。

## Consequences

- **收益**：装饰图整张落在窗口里（缩放成功且 room 生效时）；缩放失败/非 PNG/超限不再丢装饰；内边距只有一处；直角与小圆角卡片的拉伸中心只含填充色；超限图不进 cairo。
- **代价与已知上限**：**「退回原图不丢装饰」不等于「原图仍完整可见」**——非 PNG 或超限退回时读不出高度，仍可能被 `OverlayClipMargin` 裁掉顶部。可见几何变化：皮肤声明预留高度小于按 `width_dip` 缩放后的高度时，装饰会等比缩小到 `band + pad`（有意）。`edge` 在 `border_width > radius` 时才改变切片，真实解析路径 `border_width` 只有 0/1，所以实际变化集中在用户/皮肤选了小圆角（含 0）的卡片。既有快照期望随之改变：`corner_radius 0.0` + 1px 描边的 `[InputPanel/Background/Margin]` 由 12 变 13。room 的推导来自基线测试与注释，未读 fcitx5 源码，语义若不同需设备截图复核。

## Verification

`platforms/linux/tests/candidate/candidate_fcitx_overlay_scale.cpp`（room 缩放 `(200,100)=76×100`、`(85,119)==(85,0)`、`fcitx_png_size` 边界 2048/0/4096/非 PNG、暂存端到端 1x/@2x/`OverlayOffsetY`、超限与坏 PNG 退回原图、1x 成 2x 败退回）、`platforms/linux/tests/candidate/candidate_fcitx_theme.cpp`（九宫格拉伸中心像素回归覆盖 `wide(border 3)+radius 0`、`wide+radius 3`、`wechat_dark(border 1)+radius 0`；负描边宽度与 0 生成相同 theme）。失败回归证据见各阶段报告（先测后改，单点回退复现 exit 134）。
