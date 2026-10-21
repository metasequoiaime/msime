# Agent Note: Fcitx5 主题切片避开标识并允许关闭自动生成

Status: implemented

## Problem

#8968：classicui 生成背景的左、上切线只包住标识，没有留纯色采样区；1.25／1.5 倍显示缩放下，标识像素可能进入可拉伸条带。主题输入变化或插件重置又会重建用户数据目录中的主题，手工修过的边距和图片可能丢失。

## Decision

普通形状背景的左、上固定切片向上对齐到 4 px，包含标识的左上切片越过标识边界后再留 `max(2, ceil(2 * candidate_scale))` px。右、下切片保持只容下阴影和圆角：classicui 绘制九宫格时右、下切线位于「按内容决定的面板宽高减去边距」处，宽高本身不是 4 的倍数，对齐边距不能让这两条切线落在整设备像素上，只会抬高面板最小尺寸（带标识时上下合计从 60 变 64），让短横排面板的角块更容易重叠、不透明度低于 100% 时出现更暗的条带。描边、圆角仍完整位于固定切片中。内容留白、标识坐标、阴影边距不跟着切片扩张；外部 canvas 包自己的切片不受影响。

`MSIME_FCITX5_THEME_AUTOGENERATE=0` 让 Fcitx5 宿主退出主题生成与接管：不写或清理主题及图片，不创建缺失目录，不改 `Theme`／`DarkTheme`，不以原生 X11 画布覆盖人工主题。未设置、`1` 和其他值保留现有自动同步。字体同步是独立功能，继续按偏好生效。此开关只从 Fcitx5 进程环境读取（`fcitx_theme_autogenerate_enabled`），不增加共享偏好或其他平台设置。关闭时经典界面画的是人工主题，宿主向设置页报告 `fcitx_theme` 限制，即使 `Theme` 仍是水杉，颜色和皮肤设置也如实标为不生效。

卸载、AUR `msime`／`msime-bin` 互换、`.deb` 替换和 RPM Obsoletes 都在用户的 systemd 实例里运行 `msime-linux-setup --unregister`，那里看不到 Fcitx5 进程的环境变量。所以 `restore_panel_settings` 按目录内容判断主题归属：`theme.conf` 仍带生成器的 `Description=Generated from the MSIME candidate settings` 说明，且目录里只有 `theme.conf` 与 `shape-`、`decoration-`、`background-`、`canvas-` 前缀（及 `theme.conf.tmp-` 临时文件）的普通文件时，才删除目录并把仍指向它的 `Theme`／`DarkTheme` 改回自带主题；否则目录与这两项都保留。

关闭前先生成一份主题，随后由用户编辑、重载；要在卸载和换包后保留，改掉说明行或放入自己命名的图片。恢复自动生成前需备份定制文件。README 记录启动方式、恢复代价和实际重建时机：每次 Fcitx5 启动后的第一次同步都会重建，不只是首次接管。

## Related decisions

- [装饰与描边几何](2026-10-07-fcitx5-overlay-geometry.md)：部分重叠；高度缩放和整条描边约束保留，原先左上切片止于标识边界的部分由本篇取代。
- [主题同步缓存](2026-10-08-fcitx5-theme-sync-tick.md)：部分重叠；默认缓存、第三方主题所有权和失败重试保留，本篇增加明确退出入口。
- [静态 canvas 面板](../feature/2026-10-19-linux-fcitx-canvas-panel.md)：部分重叠；包的切片与图像准备规则保留，关闭生成时也不再生成该中间主题。
- [原生 X11 画布](../architecture/2026-10-20-linux-native-canvas-x11.md)：部分重叠；正常绘制规则保留，关闭自动主题时退出原生所有权，人工 classicui 主题才实际可见。

检索活跃笔记的 `8968`、`write_fcitx_candidate_theme`、主题自动生成及切线，没有需要删除的完全重复决定。

## Alternatives considered

- **随包安装一份静态主题**：发行版最容易直接 patch，也无需运行期生成。但当前主题随任意用户配色、尺寸、皮肤与明暗组合变化，单个静态主题不能保留这些功能；本次采用 issue 同样接受的显式关闭入口。
- **只在主题缺失时生成**：逻辑最少，已有定制天然保留。但用户之后换肤或修改配色都不会生效，会改变所有人的正常行为。
- **检测人工修改后自动保留**：无需用户设置环境变量，也可同时支持自动换肤。可靠判断需要保存此前整套配置和图片的生成指纹、处理旧安装无指纹及部分写入；这里选择边界明确的显式退出，避免误认人工或自动文件。
- **只把报告中的 Top 固定改为 40**：直接匹配报告的尺寸。不同候选缩放、内边距和圆角会改变标识位置，必须按实际几何留白；左、上切线的整像素对齐也需要同时处理。

## Verification

`linux-candidate-fcitx-theme` 覆盖 5 种整体大小、3 种内边距与 3 种圆角下的左上对齐、右下保持原值、安全带和 1x／@2x 实际像素，原有描边与透明度测试继续保留。旧实现新增几何回归断言失败。

`classicuiThemeTicks` 驱动真实宿主与记录配置写入的经典界面替身：关闭生成后经历明暗和配色变化、主动选择、缺失目录，主题与所有图片的内容、inode 和时间戳不变；恢复生成后又能按偏好更新。真实 classicui 加载入口也执行该用例，旧实现因改写定制文件而失败。

`linux-setup-panel-restore`（`tests/core/setup_panel_restore.py`）覆盖生成原样、改了说明行、多出文件或子目录、符号链接四种主题目录，以及 Fcitx5 运行与否两条恢复路径；改动前的脚本通不过。`fcitx_theme_autogenerate_enabled` 的取值表在 `linux-candidate-fcitx-theme` 中。

远端固定 Linux 容器构建 IBus/Fcitx5 及全部 provider，107 项 CTest 通过（评审修正后重跑同样通过，`linux-ibus-demand-activation` 照旧跳过），并完成 full／wubi 分开安装、五笔库链接和日文语言登记检查。完整 `scripts/verify-local.sh --quick` 通过，包含 Rust 工作区、Linux 设置外壳和共享 Apple 桥接层检查；本机 `pnpm run verify-notes` 与 `git diff --check` 通过。未运行报告人的 niri／Wayland 分数缩放环境。

## Consequences

切线留在标识外的底色中，左、上切线在常见分数缩放下保持整数像素。用户和发行版可以持久维护生成主题，不必复制到另一个名字才能避开覆写。

左、上固定切片对齐会各补 0–3 px，左上另含标识安全带，源 PNG 最小尺寸增加；右、下不变。按内容判断主题归属是启发式：保留说明行、只改已有文件内容的人工主题仍会在卸载时删除，README 写明了保留的方法。关闭生成后主题与候选偏好不会自动同步，需要用户重载；重新开启会覆盖人工定制。此改动不证明上游 classicui 的所有分数缩放采样问题已解决，也不能替代报告人的 niri／Wayland 实际验收。
