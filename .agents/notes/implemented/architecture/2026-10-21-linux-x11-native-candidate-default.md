# Agent Note: X11 原生画布成为 Fcitx5 的默认候选窗，与 macOS 共用候选排版规则

Status: implemented

候选卡片显隐与高亮的动效由[所有平台的候选立即呈现](../simplification/2026-10-30-candidate-immediate-presentation.md)取代；本篇的布局、定位、指针与装饰图帧动画决定继续成立。

## Problem

macOS 候选窗的每个像素都由水杉自己画：首行有 logo、半粗体拼音和插入光标、「1 / 3」、28pt 的 ‹ › 与展开键，候选行按 `CandidateItemLayout` 排，释义固定 12pt，悬停有底色和手形指针，还有展开网格、皮肤动效和装饰图待机动效。Linux 上默认的 Fcitx5 路径把候选交给 classicui 的生成主题，IBus 交给桌面面板，这些细节在两条路径里都无从表达；只有选了 canvas 皮肤包、又在 X11 上时，插件才用自己的原生窗口画（[原生画布](2026-10-20-linux-native-canvas-x11.md)）。另一面，macOS 的排版、网格、滚轮和定位规则写在 `platforms/macos/src/candidate` 里，Linux 要对齐只能再抄一份，两份会漂移。

产品决策是：Linux 的 `system` 候选主题保留 Adwaita 配色，几何、字号、排版和交互对齐 macOS；画不出来的路径优雅退化，不能让 Wayland 和 IBus 变差。

## Decision

- **规则只有一份，放在 `shared/candidate`。** 不碰 AppKit 的 `CandidateItemLayout.h`、`CandidateExpandedGrid.h`、`CandidateWheelRouting.h` 从 macOS 目录移过去，命名空间改成 `msime::candidate`，macOS 的调用处改名引用；新加的 `CandidatePlacement.h`（`CandidateOrigin`、记住最高一页与最宽一张、选屏）、`CandidateDecoration.h`（`DecorationPlacement`、`kDecorationMotionMargin`）、`CandidateChromeMetrics.h`（首行、行、释义、光标的尺寸常量）和 `CandidateMotionRules.h`（动效时长、幅度、曲线和逐帧采样）由两边共用：macOS 的 `CandidatePlacement.h`、`CandidateChrome.h`、`CandidateTypography.h`、`CandidateSkin.h` 和 `CandidateMotion.mm` 只把 NSRect 换成共享结构或直接引用常量。坐标系照 AppKit（y 朝上），Linux 用 `CandidateFlipY` 翻过去再翻回来。
- **X11 上原生画布是 Fcitx5 的默认画法。** 水杉接管了 classicui 主题、当前 UI 是 classicui、输入上下文是 X11 时，所有主题都由插件自己的窗口画：canvas 包仍按清单的 Windows 版式（`measure_native_canvas_skin`），其余主题合成 `NativeCanvasAssets::chrome` 资产（`native_canvas_theme_assets`：用户样式算好的内边距、圆角、行圆角、描边与不透明度，皮肤包的 `resolvedLayout`、字体、`[animation]`、装饰图和待机动效），配色是共享层解析结果盖在 Adwaita token 上（`native_canvas_theme_palette`），按 macOS 结构画（`NativeCanvasChrome.h`）。Wayland、kimpanel、IBus 和第三方 classicui 主题不变；Wayland 会话里走 XWayland 的程序也不合成 chrome 资产，水杉主题照旧由 classicui 画（canvas 包在那里本来就由原生画布画），会话判断与能力位报告是同一个 `candidate_session_is_x11`（`XDG_SESSION_TYPE` 为 `x11`／`wayland` 时照它，`tty` 等其他值或没设时看 `DISPLAY` 与 `WAYLAND_DISPLAY`），画的和设置页给的开关不会对不上；`MSIME_FCITX5_CANDIDATE_RENDERER=classicui` 是整套退回 classicui 的逃生口；原生窗口准备失败（工作区放不下、X 连接失败）时那一帧仍由 classicui 画。
- **macOS 结构的内容。** 首行、行与释义、底部的本地模式与连击行、悬停与手形指针、按下与选中、选中条（没设颜色时跟随主题 accent）、顶带装饰、皮肤背景图（cover／contain／stretch，裁在圆角里）、有合成器时的卡片投影（取 classicui 主题那一层，窗口四周按投影边距放大，定位时贴光标的仍是卡片）、宽度上限与下限、横排宽度记忆、竖排翻转记忆、光标下方 4pt 的定位，都逐条对照 `renderCandidates` 与 `candidatePageGeometry`，数值取自 `shared/candidate`。滚轮翻不翻页照 classicui 落盘的 `WheelForPaging`（缺省为开），与原先 classicui 画候选窗时一样：水杉的「鼠标滚轮」开关动过就会写进这一项，从没动过就沿用桌面原有设置；只照水杉的开关会让从没碰过它的 X11 用户升级后滚轮翻页悄悄关掉，设置页「从未开启过时沿用 Fcitx5 原有设置」的说明也随之不对。动效只播清单显式写了的预设，进场和退场要合成器，桌面关掉动画（GNOME 的 `enable-animations`、KDE 的 `AnimationDurationFactor=0`）时什么都不播、帧动画停在首帧，DPMS 熄屏时不再定时唤醒。
- **展开网格。** `expand_candidates` 打开、横排、不止一页时，↓、Tab（不看 Tab 翻页开没开，与 macOS 相同）或展开键打开网格，只有高亮所在的行画序号，序号列却每行都按「1」…「n」量（与 macOS 相同），高亮换行时文字不左右跳；一次看得见的行数取皮肤共享版式的 `expandRows`（2–8）加 1，没有时 5 行；网格里的 - = 在日语里仍是输入；状态在 `FcitxState`（`expanded_*`，与 canvas 包的卷轴共用），键盘规则是共享的 `StepExpandedGrid`，候选经 `msime_client_all_candidates` 复制、`msime_client_select_any_candidate` 选择，与 macOS 相同；复制会推进代次时先重读当前页再核对。网格只在原生画布上存在。
- **设置页的能力位由宿主报告。** Fcitx5 在 X11 会话里画 macOS 结构时，`candidate-panel.json` 带 `"renderer":"native_canvas"`；client-core 的 `linux_native_canvas_renderer` 读它，Tauri 外壳据此调用 `HostCapabilities::apply_linux_native_canvas`，给出展开候选、拼音字号和选中外观三项。原生画布画编号色、强调色和固定候选的颜色，所以选中外观打开时，编号和强调色取色器下面不再写「此项仅对 IBus 生效」。按平台给的能力位不变：IBus 和 Wayland 用户看不到画不出来的开关。
- **classicui 和 IBus 也向 macOS 靠一步。** 页码统一写成「1 / 3」（`candidate_page_indicator`）；classicui 画 canvas 面板时，整个落在角切片里的自由挂件画进面板图，Wayland 上也看得到。

## Alternatives considered

- **继续用 classicui，只把主题做得更像 macOS：** 最省事，Wayland 与 X11 一套代码，桌面行为由 Fcitx5 负责。classicui 的主题只有一行辅助文字、一种文字颜色、固定切片的九宫格，没有悬停、手形指针、首行布局、两行行文字、网格和动效的入口，上一批已经把能调的都调了（[样式对齐](../feature/2026-10-20-linux-candidate-style-mac-parity.md)），剩下的差距只能自己画。
- **canvas 包也改用 macOS 结构：** macOS 对 canvas 包同样画自己的首行，统一后只剩一种画法。canvas 包的清单声明了 Windows 的翻页样式、翻页位置和行距，现有验收按这些像素检查；改掉等于让这些皮肤在 Linux 上换一种排法，作者的声明失效，所以保留清单版式，差异写进 README。
- **macOS 的头文件原地保留、Linux 抄一份：** 不动 macOS，风险最小。两份规则迟早漂移，这次要对齐的正是这些数值，所以移到共享目录；macOS 的改动只是改名和引用，`candidate-*` 单测与产品目标都在 macOS 上编译通过。
- **在 macOS 目录留转发头文件：** 调用处一行不改。仓库里已经有转发的先例（`JapaneseConversion.h`），但这里的符号带 `msime::mac::` 前缀散在几十处，转发会让两个命名空间长期并存，读代码的人不知道规则归谁；直接改名一次说清楚。
- **逃生口做成共享偏好：** 用户在设置页就能切换。偏好要进 client-core、设置页、各平台的能力位，而它只是原生画布出问题时的退路；环境变量够用，不增加一个大多数人永远不该碰的开关。
- **XWayland 程序也用 chrome 画法：** 同一会话里 X11 与 XWayland 程序看到一样的候选窗。但设置页按会话类型报告能力位，Wayland 会话里不给展开候选、拼音字号和选中外观；画的和设置页给的开关会对不上（打开过的展开候选在那里仍会响应 ↓ 和 Tab），所以 Wayland 会话保持原来的 classicui 画法。
- **投影交给窗口管理器或合成器：** 少画一层。Mutter 等不给 ARGB 的 override-redirect 窗口加投影，classicui 主题原本自己画了这一层，原生画布不画就是退步；只在有合成器时画，无合成器的 Shape 窗口画不出半透明。
- **Wayland 也开原生窗口：** 体验一致。输入法不能自己放置 popup，普通顶层窗口会被合成器当成应用窗口，定位和焦点都不可靠，前一篇笔记已经否掉。

## Consequences

- **收益：** X11 上每个 Fcitx5 用户的候选窗在结构、排版、交互和动效上与 macOS 一致，配色仍是 Adwaita；排版、网格、滚轮、定位、装饰摆放和动效数值两边各只有一份；设置页在 Linux 上只给画得出来的开关。
- **代价：** 插件的原生窗口从 canvas 包的小众路径变成默认路径，指针、动效计时和定位的缺陷会影响所有 X11 用户；逃生口要用户自己设环境变量。没有合成器时进场（连同逐行错峰）和退场不播，卡片也没有投影；投影图按窗口尺寸缓存，尺寸变了才重算；重算时跳过卡片内部（那里的投影整块被擦掉），只对四周的一圈像素算距离和 erfc。macOS 的悬停提示（tooltip）、读屏朗读和逐行读音（pronunciation）还没有接入：前两者需要 X11 上单独的提示窗和 AT-SPI，后者需要 Linux 先有读音的数据通路。canvas 包的 Windows 卷轴展开由[原生画布布局](../feature/2026-10-20-linux-canvas-layout.md#横排卷轴)接入，与这里的网格共用宿主的展开状态、取数核对和选择入口（见该篇「与 macOS 结构网格合流」）。
- **行为变化：** X11 上候选窗离光标多了 4pt 间隙，有装饰带时卡片顶边（而不是窗口顶边）对着光标；页码从「1/3」变成「1 / 3」；底部辅助行只剩本地模式和连击，页码和拼音移到首行。
- **旧笔记审计：** [原生画布](2026-10-20-linux-native-canvas-x11.md) 的「只为 canvas 包创建原生窗口」被本篇部分取代，其余生命周期与回退规则仍成立，两篇互链；[样式对齐](../feature/2026-10-20-linux-candidate-style-mac-parity.md) 是本篇的前一批，留给本篇的事项（原生画布做默认、菜单与折行）在这里落地；[原生画布布局](../feature/2026-10-20-linux-canvas-layout.md)、[原生画布外观](../feature/2026-10-20-linux-canvas-appearance.md) 与 [canvas 动画](../feature/2026-10-20-linux-canvas-animation.md) 仍描述 canvas 包那条版式，部分重叠；[macOS 展开网格](../feature/2026-10-12-macos-expanded-candidate-grid.md)、[macOS 横排宽度](../bug-fix/2026-10-20-macos-steady-candidate-width.md)、[声明式动效](../feature/2026-10-16-macos-candidate-declared-motion.md) 与 [装饰图待机动效](../feature/2026-10-16-skin-decoration-idle-motion-and-apng.md) 的路径事实已就地改成共享目录。

## Testing

- macOS：最初只在 `target/macos-isolated` 编译了产品目标 `MSIMEClientInputMethod` 和候选相关单测（`candidate-skin`、`external-skin`、`candidate-panel`、`candidate-item-layout`、`candidate-row-layout`、`candidate-canvas`、`candidate-hover`、`candidate-placement`、`candidate-pagination`、`candidate-expanded-grid` 十项通过），没有跑整个 CMake 构建，漏掉了 `input-mode-hud-panel-test`：它定义在 CMakeLists 统一设置 C++17 之前，`InputModeHUDPanel.mm` 经 `CandidatePlacement.h` 引入 `shared/candidate`（嵌套命名空间、`std::optional`），CI 的 AppleClang 按更早的默认标准编译而失败；改成和 `input-mode-identifiers-test` 一样单独指定 `OBJCXX_STANDARD 17`。修好后在构建机上对 `target/macos-isolated` 跑全部目标（Rust 宿主库换成空的占位文件，`make -k` 继续）：没有一个源文件编译失败，失败的只有 45 个要链接 Rust 宿主库的目标；`input-mode-hud-panel`、`input-mode-identifiers` 和其余能链接的 candidate 单测运行通过，要链接宿主库的 `candidate-skin`、`candidate-panel`、`candidate-row-layout` 这次只验证了编译。
- 设置页：`candidate-preview-native-tokens` 原先从 macOS 的 `CandidateChrome.h` 里按字面量读行内边距，常量移到 `shared/candidate/CandidateChromeMetrics.h` 后读不到，改成从共享头文件读数值（仍是 12），并检查 macOS 那一行引用的是共享常量。`host-capabilities` 新增一项：宿主报告原生画布（选中外观打开）时，取色器下不再有「此项仅对 IBus 生效」。
- Linux 原生：`platforms/linux/build-container.sh` 在 Debian 容器里编译 IBus、Fcitx5（full 与 wubi）和全部单测，ctest 96/96 通过；`linux-candidate-native-canvas-chrome` 还检查网格各行的文字起点相同（高亮行与没画序号的行），以及跳过卡片内部的投影与逐像素完整公式的结果一致；新增的 `linux-candidate-native-canvas-chrome` 在内存里检查首行、行与释义、网格、缩放、装饰带、调色板、定位和动效采样，`linux-candidate-fcitx-canvas` 检查角上挂件进面板图，`linux-candidate-panel-status` 检查 `renderer` 字段。
- Rust：桌面检查镜像里 `cargo clippy -p msime-client-core -p msime-desktop --all-targets -D warnings` 通过，`host_surface` 33 项与 `msime-desktop` 的 `candidate_panel` 3 项单测通过。
- 真实窗口：Fcitx5 验收镜像（Xvfb、独立 D-Bus、真实 GTK 编辑器、sway）里跑 `fcitx5-container.sh` 的全部用例加上 `page-number-visibility` 与六项帧动画，42 项中 40 项通过；`fcitx5-native-chrome-{horizontal,vertical}` 用 `system` 主题检查原生窗口出现、首行翻页、悬停、展开网格和点选，以及非 canvas 皮肤包的顶带装饰；Wayland 的六项 canvas 用例照旧通过。没过的 `fcitx5-native-context` 和 `fcitx5-native-ai` 都停在「Disabled expression mode must pass through」，同一镜像里别的分支也是这样，与本篇无关。IBus 的 `ibus-engine-smoke` 通过了「1 / 」页码的检查，停在之后一条同样早已存在的智能标点检查上。
- 验收里修了两处与本篇相关的老问题：`fcitx5-container.sh` 按前缀嵌套的 ctest 正则超出 CMake 正则 10 组括号的上限，一个用例也不跑，改成逐个列出用例名；canvas 翻页验收的两轮行高相同，等待在热更新生效前就会通过，横排改为等翻页分隔线出现或消失再取按钮位置（修之前在构建机负载高时偶发点错位置，修之后连跑六轮通过）。
- 客户端自己画候选面板（`ClientSideInputPanel`）时原生画布让给客户端：`fcitx5-page-number-visibility` 用这种上下文验收，原生画布接管时客户端收不到候选。
