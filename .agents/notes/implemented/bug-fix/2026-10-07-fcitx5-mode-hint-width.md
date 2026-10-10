# Agent Note: Fcitx5 中/英模式提示按活动水杉主题与装饰宽度补宽

Status: implemented

## Problem

水杉主题带皮肤装饰图时，classicui 在候选窗顶行显示的「中」「英」提示只有一行文字，面板宽度不够，装饰被 `OverlayClipMargin` 从两侧切掉。PR #4371 的三条评论各自指出一条会踩的坑（基线里都还没落地，属旧 PR 的实现问题）：

- r4203158949：只用 `Theme == msime` 判断正在画的主题，没结合 `UseDarkTheme`、classicui 实际系统明暗与 `DarkTheme`。浅色水杉 + 第三方 `DarkTheme` + 桌面深色时，classicui 画的是第三方主题，宿主却会去补宽。
- r4203158955：在 1×1 image surface 上用 Pango 默认 96 dpi 测量，与 classicui 实际排版用的 DPI 不一致。
- r4203158968：每次中英切换都存配置、重解析 theme.conf、重读 PNG 头、建 cairo/pango layout，最多 1024 次重排。

## Decision

- **活动主题判据**：`fcitx_draws_candidate_theme(theme, dark_theme, use_dark_theme, system_dark)`（`FcitxEngine.cpp`）与 classicui `reloadTheme` 同规则——`use_dark_theme && system_dark ? dark_theme : theme` 是否等于 `msime`；`currentUI() != "classicui"`（kimpanel）直接排除。
- **所需宽度在写主题时算一次**：`fcitx_hint_width_from_theme(theme_file)` 读宿主刚写好的 theme.conf 的 `Gravity`/`OverlayOffsetX`/`OverlayClipMargin`/`ContentMargin`/`TextMargin`，用 `fcitx_png_width` 读暂存装饰图（1x 的逻辑宽度），面板宽度按 `fcitx_overlay_panel_width`（Top Center 两边各留 `max(clip_left, clip_right)`；靠边从对齐边量 `OverlayOffsetX` 再留对面 `clip`），再减文字两侧边距得 `fcitx_hint_text_width`。没有装饰、非 PNG、尺寸超限返回 0。
- **热路径缓存**：结果存 `hint_text_width_`，只在写主题成功时刷新；`hint_inputs_` 缓存活动主题、`Font`、`ForceWaylandDPI`，随主题/字体同步刷新；接管后按刚写入的配置更新，第一次提示不能沿用接管前的第三方主题。合并 develop 的每拍缓存（#5988，输入没变就直接返回）后，提示配置另有一份落盘判据（`Theme`、`DarkTheme`、跟随深色、字体、Wayland 字体 DPI），同时进入尝试缓存的 key：任一项变化都重读一次 classicui 现值，避免第三方主题下第二次字体变化被旧尝试缓存吞掉；接管成功后记录实际落盘的判据。主题输入未变且非主动选择时只刷新提示，不重写或重新接管，否则一拍连 `getConfig()` 都不调。中英切换不调 `getConfig()`（它会扫描主题目录并逐个解析 theme.conf）。
- **一次算好、一次校验**：`fcitx_pad_hint_label` 按全角空格（U+3000）数量一次补齐，随后只做一次最终宽度测量，差一点再补一个；完整的「中」「英」补宽结果按字体、分辨率、装饰宽度缓存；重复切换不再创建 Pango 对象或最终重测，装饰宽度变化则重新测量。
- **分辨率对齐 classicui 实际排版**：Wayland 且 `ForceWaylandDPI > 0` 时取它，否则取 `pango_cairo_font_map_new()` 的默认分辨率（不硬编码 96）。**5.1.23 的 classicui 不读主题里的 `ScaleWithDPI`**（全源码与 `libclassicui.so` 都没有该键），`Xft.dpi`/`PerScreenDPI` 只设 cairo device scale，不改 yoga 布局的逻辑尺寸——这与评论 r4203158955 的机制描述不一致。
- **构建开关**：`pkg_check_modules(MSIME_HINT_FONT QUIET pangocairo)`，缺它时整段不编译，提示保持原样（与 `MSIME_FCITX5_MODE_BADGE` 同一策略）。

## Alternatives considered

- **按 `Xft.dpi`/`PerScreenDPI` 再乘一次逻辑宽度** — 符合评论里「classicui 按自己的 DPI 缩放逻辑布局」的假设；但 5.1.23 实测只改物理像素，乘了会把提示补宽 1.5×（可见回归）。按实测行为只对齐 `ForceWaylandDPI`。
- **每次中英切换重读 theme.conf / 装饰 PNG 头** — 最简单、不会过期；但那是按键热路径，`getConfig()` 会解析所有主题，代价按评论可达 1024 次重排。
- **请求 classicui 暴露活动主题/有效 DPI 的公共接口** — 5.1.23 的 `classicui_public.h` 只导出 `labelIcon`/`preferTextIcon`/`showLayoutNameInIcon`，没有 getter，改动落在上游；宿主只能从 `addon("classicui")->getConfig()` 与环境推出。

## Consequences

- **收益**：带装饰的水杉主题下「中」「英」都被补到能放下装饰的宽度，装饰不再被 `OverlayClipMargin` 从两侧切掉；中英切换不读文件、不做长循环重排；第三方主题、无装饰、kimpanel 保持普通提示。
- **代价与已知上限**：缓存只在主题/字体/系统明暗同步时刷新，用户在 fcitx5-configtool 改主题后不做焦点变化就切中英，会用上一次同步的判据，下一次同步纠正。只有 PNG 装饰读得出宽度，非 PNG/超限/退回原图的装饰不补宽（装饰仍可能被裁，见几何笔记）。kimpanel 分支只有代码审查，没有测试覆盖；`ForceWaylandDPI > 0` 的 Wayland 路径没有设备验证。

## Verification

`platforms/linux/fcitx5/tests/native.cpp::candidateThemeHint`（`--theme-hint`，`#ifdef MSIME_FCITX5_HINT_FONT`）：`fcitx_draws_candidate_theme` 四种组合、`fcitx_overlay_panel_width`/`fcitx_pad_hint_label` 纯几何、读回 theme.conf+PNG 的宽度、真实 classicui 端到端（带装饰补宽且实测 ≥ 目标、删掉 `themes/msime` 目录仍返回同样结果、深色第三方 DarkTheme 不补宽、关闭跟随系统后重新补宽、无装饰内置主题与第三方主题不补宽、主动接管后的第一次提示立即补宽、退出后选回同一皮肤重新初始化、仅换 `Theme`／`DarkTheme` 就停止补宽、第三方主题下连续更改字体均刷新缓存、外部还原自带主题不被夺回、外部换回水杉主题恢复补宽）。`classicuiThemeTicks` 也在该无资源用例中实际执行，计数断言配置变化只重读一次、下一拍不再扫描，不重写已应用主题，写失败仍按 10 秒重试；默认 `DarkTheme` 被 fcitx 写成注释时按缺省值检查。链接包装 `pango_cairo_font_map_new` 统计测试程序分配：预热后重复 20 组中英切换没有新增字体映射。ctest 注册 `fcitx5-candidate-theme-hint`（`MSIME_FCITX5_CLASSICUI` 与 `MSIME_HINT_FONT_FOUND` 双守卫，缺依赖时 skip 77）。

已安装插件的独立原生 Fcitx5 daemon + 真实聚焦 GTK 编辑器验证使用合成 PNG：候选及两种内部模式提示的装饰边框均为 85×112 像素，四个角完整（X11，设备缩放 1；该设备证据对应同步前 `ad5a21c92`，不是最新 develop 基线）。隔离 daemon 的默认全局 Shift 会切到键盘输入法，因此用已启用的 Ctrl+Alt+Space 走水杉内部切换，并断言当前输入法仍为 `msime`；这不覆盖 Fcitx5 自带的全局输入法提示。实际桌面的快捷键依用户原配置，不为测试改写。
