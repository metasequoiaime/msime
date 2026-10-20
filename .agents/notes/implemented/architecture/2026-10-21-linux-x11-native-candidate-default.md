# Agent Note: X11 原生画布成为 Fcitx5 的默认候选窗，与 macOS 共用候选排版规则

Status: implemented

## Problem

macOS 候选窗的每个像素都由水杉自己画：首行有 logo、半粗体拼音和插入光标、「1 / 3」、28pt 的 ‹ › 与展开键，候选行按 `CandidateItemLayout` 排，释义固定 12pt，悬停有底色和手形指针，还有展开网格、皮肤动效和装饰图待机动效。Linux 上默认的 Fcitx5 路径把候选交给 classicui 的生成主题，IBus 交给桌面面板，这些细节在两条路径里都无从表达；只有选了 canvas 皮肤包、又在 X11 上时，插件才用自己的原生窗口画（[原生画布](2026-10-20-linux-native-canvas-x11.md)）。另一面，macOS 的排版、网格、滚轮和定位规则写在 `platforms/macos/src/candidate` 里，Linux 要对齐只能再抄一份，两份会漂移。

产品决策是：Linux 的 `system` 候选主题保留 Adwaita 配色，几何、字号、排版和交互对齐 macOS；画不出来的路径优雅退化，不能让 Wayland 和 IBus 变差。

## Decision

- **规则只有一份，放在 `shared/candidate`。** 不碰 AppKit 的 `CandidateItemLayout.h`、`CandidateExpandedGrid.h`、`CandidateWheelRouting.h` 从 macOS 目录移过去，命名空间改成 `msime::candidate`，macOS 的调用处改名引用；新加的 `CandidatePlacement.h`（`CandidateOrigin`、记住最高一页与最宽一张、选屏）、`CandidateDecoration.h`（`DecorationPlacement`、`kDecorationMotionMargin`）、`CandidateChromeMetrics.h`（首行、行、释义、光标的尺寸常量）和 `CandidateMotionRules.h`（动效时长、幅度、曲线和逐帧采样）由两边共用：macOS 的 `CandidatePlacement.h`、`CandidateChrome.h`、`CandidateTypography.h`、`CandidateSkin.h` 和 `CandidateMotion.mm` 只把 NSRect 换成共享结构或直接引用常量。坐标系照 AppKit（y 朝上），Linux 用 `CandidateFlipY` 翻过去再翻回来。
- **X11 上原生画布是 Fcitx5 的默认画法。** 水杉接管了 classicui 主题、当前 UI 是 classicui、输入上下文是 X11 时，所有主题都由插件自己的窗口画：canvas 包仍按清单的 Windows 版式（`measure_native_canvas_skin`），其余主题合成 `NativeCanvasAssets::chrome` 资产（`native_canvas_theme_assets`：用户样式算好的内边距、圆角、行圆角、描边与不透明度，皮肤包的 `resolvedLayout`、字体、`[animation]`、装饰图和待机动效），配色是共享层解析结果盖在 Adwaita token 上（`native_canvas_theme_palette`），按 macOS 结构画（`NativeCanvasChrome.h`）。Wayland、kimpanel、IBus 和第三方 classicui 主题不变；`MSIME_FCITX5_CANDIDATE_RENDERER=classicui` 是整套退回 classicui 的逃生口；原生窗口准备失败（工作区放不下、X 连接失败）时那一帧仍由 classicui 画。
- **macOS 结构的内容。** 首行、行与释义、底部的本地模式与连击行、悬停与手形指针、按下与选中、选中条、顶带装饰、宽度上限与下限、横排宽度记忆、竖排翻转记忆、光标下方 4pt 的定位，都逐条对照 `renderCandidates` 与 `candidatePageGeometry`，数值取自 `shared/candidate`。动效只播清单显式写了的预设，进场和退场要合成器，桌面关掉动画（GNOME 的 `enable-animations`、KDE 的 `AnimationDurationFactor=0`）时什么都不播、帧动画停在首帧，DPMS 熄屏时不再定时唤醒。
- **展开网格。** `expand_candidates` 打开、横排、不止一页时，↓、Tab 或展开键打开网格；状态在 `FcitxState`（`expanded_*`），键盘规则是共享的 `StepExpandedGrid`，候选经 `msime_client_all_candidates` 复制、`msime_client_select_any_candidate` 选择，与 macOS 相同。网格只在原生画布上存在。
- **设置页的能力位由宿主报告。** Fcitx5 在 X11 会话里画 macOS 结构时，`candidate-panel.json` 带 `"renderer":"native_canvas"`；client-core 的 `linux_native_canvas_renderer` 读它，Tauri 外壳据此调用 `HostCapabilities::apply_linux_native_canvas`，给出展开候选、拼音字号和选中外观三项。按平台给的能力位不变：IBus 和 Wayland 用户看不到画不出来的开关。
- **classicui 和 IBus 也向 macOS 靠一步。** 页码统一写成「1 / 3」（`candidate_page_indicator`）；classicui 画 canvas 面板时，整个落在角切片里的自由挂件画进面板图，Wayland 上也看得到。

## Alternatives considered

- **继续用 classicui，只把主题做得更像 macOS：** 最省事，Wayland 与 X11 一套代码，桌面行为由 Fcitx5 负责。classicui 的主题只有一行辅助文字、一种文字颜色、固定切片的九宫格，没有悬停、手形指针、首行布局、两行行文字、网格和动效的入口，上一批已经把能调的都调了（[样式对齐](../feature/2026-10-20-linux-candidate-style-mac-parity.md)），剩下的差距只能自己画。
- **canvas 包也改用 macOS 结构：** macOS 对 canvas 包同样画自己的首行，统一后只剩一种画法。canvas 包的清单声明了 Windows 的翻页样式、翻页位置和行距，现有验收按这些像素检查；改掉等于让这些皮肤在 Linux 上换一种排法，作者的声明失效，所以保留清单版式，差异写进 README。
- **macOS 的头文件原地保留、Linux 抄一份：** 不动 macOS，风险最小。两份规则迟早漂移，这次要对齐的正是这些数值，所以移到共享目录；macOS 的改动只是改名和引用，`candidate-*` 单测与产品目标都在 macOS 上编译通过。
- **在 macOS 目录留转发头文件：** 调用处一行不改。仓库里已经有转发的先例（`JapaneseConversion.h`），但这里的符号带 `msime::mac::` 前缀散在几十处，转发会让两个命名空间长期并存，读代码的人不知道规则归谁；直接改名一次说清楚。
- **逃生口做成共享偏好：** 用户在设置页就能切换。偏好要进 client-core、设置页、各平台的能力位，而它只是原生画布出问题时的退路；环境变量够用，不增加一个大多数人永远不该碰的开关。
- **Wayland 也开原生窗口：** 体验一致。输入法不能自己放置 popup，普通顶层窗口会被合成器当成应用窗口，定位和焦点都不可靠，前一篇笔记已经否掉。

## Consequences

- **收益：** X11 上每个 Fcitx5 用户的候选窗在结构、排版、交互和动效上与 macOS 一致，配色仍是 Adwaita；排版、网格、滚轮、定位、装饰摆放和动效数值两边各只有一份；设置页在 Linux 上只给画得出来的开关。
- **代价：** 插件的原生窗口从 canvas 包的小众路径变成默认路径，指针、动效计时和定位的缺陷会影响所有 X11 用户；逃生口要用户自己设环境变量。没有合成器时进场和退场不播。macOS 的悬停提示（tooltip）、读屏朗读和逐行读音（pronunciation）还没有接入：前两者需要 X11 上单独的提示窗和 AT-SPI，后者需要 Linux 先有读音的数据通路。canvas 包的 Windows 展开面板仍未接入。
- **行为变化：** X11 上候选窗离光标多了 4pt 间隙，有装饰带时卡片顶边（而不是窗口顶边）对着光标；页码从「1/3」变成「1 / 3」；底部辅助行只剩本地模式和连击，页码和拼音移到首行。
- **旧笔记审计：** [原生画布](2026-10-20-linux-native-canvas-x11.md) 的「只为 canvas 包创建原生窗口」被本篇部分取代，其余生命周期与回退规则仍成立，两篇互链；[样式对齐](../feature/2026-10-20-linux-candidate-style-mac-parity.md) 是本篇的前一批，留给本篇的事项（原生画布做默认、菜单与折行）在这里落地；[原生画布布局](../feature/2026-10-20-linux-canvas-layout.md)、[原生画布外观](../feature/2026-10-20-linux-canvas-appearance.md) 与 [canvas 动画](../feature/2026-10-20-linux-canvas-animation.md) 仍描述 canvas 包那条版式，部分重叠；[macOS 展开网格](../feature/2026-10-12-macos-expanded-candidate-grid.md)、[macOS 横排宽度](../bug-fix/2026-10-20-macos-steady-candidate-width.md)、[声明式动效](../feature/2026-10-16-macos-candidate-declared-motion.md) 与 [装饰图待机动效](../feature/2026-10-16-skin-decoration-idle-motion-and-apng.md) 的路径事实已就地改成共享目录。

## Testing

- macOS：`target/macos-isolated` 编译 `MSIMEClientInputMethod` 与 `candidate-*` 单测，`candidate-item-layout`、`candidate-pagination`、`candidate-expanded-grid`、`candidate-placement`、`candidate-row-layout`、`candidate-panel`、`candidate-canvas`、`candidate-hover` 通过。
- Linux 原生：`platforms/linux/build-container.sh` 在 Debian 容器里编译 IBus、Fcitx5 和全部单测，ctest 全部通过；新增的 `linux-candidate-native-canvas-chrome` 在内存里检查首行、行与释义、网格、缩放、装饰带、调色板、定位和动效采样，`linux-candidate-fcitx-canvas` 检查角上挂件进面板图，`linux-candidate-panel-status` 检查 `renderer` 字段。
- 真实窗口：`fcitx5-native-chrome-{horizontal,vertical}` 在独立 Xvfb、D-Bus 和真实 GTK 编辑器里用 `system` 主题检查原生窗口出现、首行翻页、悬停、展开网格和点选上屏；原有的 canvas 包与 classicui 验收照旧运行，classicui 的译文验收用逃生口关掉原生画布。
