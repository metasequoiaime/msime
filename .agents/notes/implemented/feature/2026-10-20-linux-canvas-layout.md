# Agent Note: Linux 原生画布消费皮肤字体与候选布局

Status: implemented

## Problem

[原生 X11 画布](../../implemented/architecture/2026-10-20-linux-native-canvas-x11.md)和[图片动画](../../implemented/feature/2026-10-20-linux-canvas-animation.md)已有真实 GTK 验收，但布局仍写死内边距、候选高度、序号字号与间距。共享 catalog 已解析这些字段，装上同一份 msime-windows 皮肤仍画不出指定布局。固定参考 `msime-windows@dd018f79858314ed542a47c528608752b13bd261` 的 `candidate_skin_tokens.cpp`、`candidate_skin_layers.cpp`、`candidate_skin_scene.cpp` 与 `CandidateList.cpp`。长正文和释义此前没有宽度约束，整个候选窗超出工作区后只能退回桌面面板，无法保持画布皮肤。

## Decision

沿用共享 catalog 的已校验输出，按公共 `geometry`、当前排列覆盖表的顺序消费布局。Pango 保留用户字号，皮肤 `fontFamily` 排在用户字体及其回退链之前；按实际 DIP 字号换算 em 并取整，不能把 Pango 的 point 直接当作 DIP。测量、绘制、点击共用同一份布局与字体，不能只改变绘制偏移。

消费 `padding`、`item_gap`、`item_extra_height`、`number_inset`、`item_padding_right`、`item_min_width`、`number_gap`、`number_scale`、`preedit_margin_dip`、`justify_rows` 和 `min_width_dip`。显式 content insets 仍按每边与九宫格边宽取最大值，未指定时才使用皮肤 padding；`line_gap_dip` 优先于拼音底部 margin。没有相关字段时保留既有布局。横排在可用宽度不足时折到下一行，每行独立撑高与均分。`number_scale = 0` 隐藏序号字形，但仍保留来源的最小 9 DIP 序号栏与 `number_gap`，避免把合法零值误当作移除整个标签。隐藏拼音时本来就不留行距，`trim_hidden_preedit_gap` 无额外动作。`fit_content` 只改变横排显式间距的额外 6 DIP 及内容尾部留白，现有宽度本来就随内容收缩。

长文本遵循固定上游 `CandidateList.cpp` 的可用宽度语义：正文和释义完整换行，竖排只有短正文与整段释义都能放下时才并列，否则释义移到下方。Pango 使用词边界与字符回退换行，保留原文及 UTF-8 分段范围。布局保存每项文字的测量宽度；均分只扩展背景和命中，不让绘制擅自改变断行。预编辑、辅助行和预编辑光标也共用受限布局。短预编辑和辅助行保留原有最小行高，长文本只向下增高，避免 ASCII 与中文字体回退的行高差异改变短文本布局。

X11 在测量前取得光标所在工作区并按缩放换算 DIP；九宫格、outer insets 与挂件参与最终窗口尺寸复核，必要时收紧面板宽度后重测。超高、最小序号栏也放不下或装饰无法容纳时仍回退桌面面板，不裁掉候选文字。默认无宽度限制的测量入口继续供离屏测试使用。候选显示后的独立光标矩形更新也会复核工作区尺寸，变窄或变矮时重新测量并取消旧指针格位；失败返回宿主重建桌面面板，不能只隐藏原生窗而留下空面板。

### 列表内翻页

固定参考同一上游的 `CandidateRowWithPager`、`PagerArrows.cpp` 和 `candidate_skin_scene.cpp`，来源和 GPL 许可记入[第三方清单](../../../../docs/third-party.md#linux-原生画布翻页)。原生画布消费 `pageArrows`、公共及排列覆盖的 `pageArrowStyle`；折线箭头用序号色，圆角三角用候选文字色，尺寸跟随候选 DIP 字号。横排在列表右侧留出整列，箭头位于最后一行底部，折线按基础候选行高抬起；三角在箭头前显示 `page_arrow_divider`。竖排另起矮行，上一页字形左端对齐序号左缘。辅助信息单独占行，不再把翻页塞在其尾部；均分只分配箭头之外的候选区域，长文本折行也只用翻页列左侧的宽度。

`pageArrows` 缺省关闭有明确适用边界：`load_native_canvas` 只接管 `canvas = true` 的包，上游 `canvas` 开关沿用 `fluent`，其出厂默认关闭。非 canvas 基底仍由已有桌面面板处理，不在 Linux 复制所有基底的开关表。显式 `false` 隐藏箭头，横排的展开开关优先于翻页开关，竖排不受展开开关影响；可展开的普通横排通过独立按钮进入下述卷轴；释义页没有展开按钮，仍使用分页。收起时滚轮继续遵守原有偏好。

两侧箭头始终占位，不可用的一侧以来源透明度绘制，吞点击但不高亮、不调用翻页。命中、绘制与 X11 输入形状共享矩形；指针状态不修改键盘选中项，翻页复用原 `CandidateList` 快照，配置／候选刷新清掉旧按下状态。菜单没有候选翻页区。共享主题接口增加可选规范化颜色 `candidate_details.page_arrow_divider`，缺省不输出，保留其他宿主的读取兼容。

### 横排卷轴

对照 `msime-windows@c0a9d371f00b7b309c07e78bc0fd27d3a0d4f5b1` 的卷轴语义，原生 X11 画布在横排右侧绘制固定的展开／收起按钮。首行仍为当前页，后续网格来自 `msime_client_all_candidates`，不拿当前页冒充完整列表。网格采用至少 2.6em 的等宽列，长候选跨格，超过可用宽度时文字继续换行。`expandRows` 从共享 `resolvedLayout` 顶层读取，是首行之外的 2–8 行，缺省 4；窗口宽高受光标所在工作区的一半限制，容不下完整首行或装饰时继续回退。

打开时才请求完整候选；`all_candidates` 可能释放缓存尾部并推进代次，所以先重新读 `msime_client_view`，再构造当前页和网格。`NativeCanvasSnapshot.h` 逐项检查 session、generation 和严格递增的非负整数 index，拒绝错误类型、混代和重复身份。网格只显示当前页之后的候选，跳过占位行，不显示序号与释义；显示文本沿用既有简繁、辅助码与脚本注释投影。选择使用完整列表提供的全局身份调用 `select_any_candidate`，并在宿主再次核对焦点、会话与代次；不通过翻页或按显示文字寻找替代项。

首行与网格在同一视口内滚动，按钮固定。展开时滚轮按候选行高滚动，不受滚轮翻页偏好影响；细滚动条支持拖动与轨道点击。首批测量 300 个后续候选，接近底部时增加 300 个，完整列表的其余项目保留在同一快照中。绘制与命中都裁到视口，滚出去的候选不接收点击；网格不提供当前页专用的右键菜单。继续输入保留展开（与 Windows 相同），继续输入、翻页或简繁投影变化后回到顶部；后台换代（云候选、释义到达）按新的一代重载完整列表，滚动留在原位、正在拖的滑块继续拖。焦点丢失、上屏、关闭窗口、换皮肤、竖排、释义页或韩文汉字、注音的候选列表收起；展开后画不下时收起并由原生画布画普通的一页。卷轴由清单的 `expandPanel` 打开，不受设置页「展开候选」与「显示展开键」控制，也不接管键盘。该实现仍只覆盖 Fcitx5 classicui 的 X11 canvas 所有权路径，不能外推到 Wayland 或 IBus。

### 与 macOS 结构网格合流

本切片写成时，X11 原生画布还只画 canvas 包；develop 随后由[X11 原生画布成为默认](../architecture/2026-10-21-linux-x11-native-candidate-default.md)让其余主题按 macOS 结构画，并带来键盘驱动的展开网格。合并时两者不并行保留两套机制，而是共用一份宿主状态：`FcitxState::expanded_*` 记完整候选、会话、代次、组字、皮肤资产和样式（`ExpandStyle::grid` 或 `scroll`），`expandedEntries` 是唯一的取数与核对入口（先 `all_candidates`，再重读当前页，用 `native_canvas_snapshot_current` 整份核对），`selectExpanded` 是唯一的按全局身份选择入口，`expandedStillApplies` 在 `render()` 开头统一决定收起或按新的一代重载，两种样式的展开／收起键是同一个回调。画布侧 `NativeCanvasContent` 的 `expandable`／`expanded`／`expand` 与 `NativeCanvasLayout::expand` 两种画法共用，命中码共用 `kNativeCanvasHitExpand`，卷轴另有 `kNativeCanvasHitScrollbar`；网格项不再有单独的 `select_grid`，命中下标接在当前页后面，由同一个 `select` 处理。`expandRows` 由 `native_canvas_expand_rows` 读，两种资产共用，缺省都是首行之后 4 行。

保留的差异是有意的：网格跟着一段组字（组字变了就收起）、接管键盘、按行滚动、只在 `expand_candidates` 打开且不止一页时提供；卷轴是清单声明的开关，继续输入时留着、不接管键盘、按像素滚动，窗口受工作区一半限制。这样 chrome 主题的行为与 macOS 一致，canvas 包的行为与 Windows 一致。合流顺带修了网格的一处缺陷：develop 原来按调用前的当前页核对完整候选的代次，而 `all_candidates` 放出尾部时会推进代次，首次展开会悄悄失败、首行点选被当成旧身份拒绝，直到下一次按键；现在网格也走重读当前页的那条路。

不采用的做法：让 canvas 包也改用 macOS 网格（作者在清单里声明的是 Windows 卷轴，会让同一个皮肤在 Linux 上换一种交互）；卷轴也接管键盘（Windows 来源没有，键盘会与普通候选窗的选词、翻页规则冲突）；保留卷轴自己的快照与刷新函数（两份代次核对和收起规则迟早不一致，正是这次合并要消除的）。

### 验收截图与热更新同步

GTK 验收将原始截图与整屏颜色计数分开：几何、局部取色和存图只获取 `Pixbuf`，只有颜色数量断言构造整屏 `Counter`。2 倍缩放下冗余整屏统计会挤占原有 90 秒预算，因此保留断言与超时上限，减少不使用的计算。

翻页两种箭头样式具有相同行高，不能只靠高度确认皮肤热更新。每轮使用不同的合成预编辑背景色，等该颜色实际绘出后再检查高度、定位和点击，避免沿用上一轮槽位而误点候选。不增加固定等待时间。

### 旧笔记审计

- 原生 X11 画布、图片动画：部分重叠，保留窗口所有权和计时规则，互链说明本次补充的布局范围。
- [Windows 布局消费](../../implemented/feature/2026-10-19-windows-candidate-skin-layout-keys.md)：其他宿主的独立实现，参考其固定上游逐键语义，不引入 Windows 依赖。
- [跨平台清单](../../implemented/feature/2026-10-19-windows-skin-schema-parity.md)：保留共享解析契约，本次只改 Linux 原生消费，不增加一套清单校验。
- 静态 classicui、其他平台及生命周期笔记：不改变其决定；Wayland、IBus 与其他桌面 UI 不由本次覆盖。

## Alternatives considered

直接复用 Windows 的布局头可以减少规则重复，但它包含 Windows 卡片自有的 logo、首行和翻译行语义，不适合移植整个尺寸模型。Linux 使用共享已校验字段，Pango 实测自身字体与文字，并按来源语义映射到现有原生窗口。

继续在辅助行末尾画文本箭头最容易保留旧窗口结构，但不能表达皮肤开关、横排独立列、禁用状态与三角分隔线，因此翻页是参与测量的独立区域。

只在绘制阶段偏移文字最容易保持窗口尺寸，却会导致文字越界、点击区域与高亮错位，因此测量和绘制使用同一份解析结果。

在窗口提交前裁切或省略超长释义能避免重测，却会丢掉上游承诺显示的内容；使用 Pango 的受限布局并把新增高度传入命中矩形。

## Testing

合入 develop（#9048、#9049）后，构建机 run `20261021-131912` 的 `build-container.sh` 通过：IBus、Fcitx5 full／wubi 与全部单测编译，ctest 107 项零失败，`linux-ibus-demand-activation` 条件跳过；`linux-candidate-native-canvas` 补了 `expandRows` 缺省（首行之后 4 行，与网格一致）、超范围夹到 8 和非整数当作没写。Fcitx5 验收镜像里 run `20261021-132455` 跑完 `fcitx5-container.sh` 的 34 项：30 项通过，`fcitx5-native-context` 与 `fcitx5-native-ai` 照旧停在「Disabled expression mode must pass through」（与本篇无关，[X11 原生画布成为默认](../architecture/2026-10-21-linux-x11-native-candidate-default.md)已记录）；`fcitx5-native-canvas-horizontal` 与 `-scale2` 在卷轴验收通过之后、置顶检查里因只读连接读到 `database is locked` 退出，复跑时 `-scale2` 又在翻页样式热更新里因两次读取之间窗口撤下而退出。两处都是验收脚本把读取失败当成异常抛出 `wait`，改成只读一次、locked 时接着等。之后 run `20261021-134139` 的原生画布与 chrome 横竖排（普通、2 倍缩放、合成器）及两项像素测试 12/12 通过；`native_chrome.py` 新增的网格滚轮、首行展开键与收起键检查一并通过。卷轴展开、滚动截图与网格滚轮截图已实际检查。

同步 `develop` 后，run `20261020-230802` 的完整 quick 通过，实际执行 Windows 策略 171 项、Linux 原生 94/94、Rust 工作区、Android 目标与 Java 宿主、WebAssembly、Linux 设置壳、macOS 和 Apple 桥接；Windows 原生／pipe 及 HarmonyOS HAP 因工具或 staged 输入不足跳过。额外矩阵 run `20261020-231840` 为 6/7，竖排 2 倍缩放触发原有 90 秒上限。拆分截图后 run `20261020-232628` 暴露同高样式热更新未完成便点击的旧槽位问题；补充实际像素同步后，单项 run `20261020-232904` 在 65.26 秒通过，超时上限不变。最终整组 run `20261020-233303` 通过 7/7，无跳过，总计 212.54 秒；六组真实 GTK 的横竖排普通／2 倍缩放／合成器及原生像素测试全部执行，其中竖排 2 倍缩放为 32.64 秒。修复后 run `20261020-233720` 的 quick 通过：仓库契约、笔记、Rust 工作区、Linux 设置壳、full／wubi 构建安装隔离和 Apple 桥接通过；Linux 原生套件共 94 项、零失败，其中 `linux-ibus-demand-activation` 条件跳过。其余平台按本次 Linux 改动范围跳过。新一轮展开与滚动截图已实际检查，Python 语法及差异检查通过。

卷轴切片的构建机 run `20261020-212421` 通过原生像素测试和普通／2 倍缩放／合成器下横竖排六组真实 GTK 验收，共 7/7。像素测试覆盖 2／4／8 附加行、300 项分批布局、滚动边界、视口外命中排除、固定按钮、滑块末端、短工作区、竖排与菜单排除，以及完整候选的整数身份、混代与重复拒绝。GTK 横排在关闭滚轮翻页的情况下检查展开／收起、滚轮、拖动滑块、轨道回顶和真实候选上屏，并核对上屏后收起与重新展开后的首行身份；参考会话通过 `prepare_host` 初始化独立合成状态目录并明确聚焦，不共享被测输入法的用户数据。展开与滚动截图已实际检查。

最终 run `20261020-212917` 在三组横排 GTK 中补验包含挂件和外留白的半屏宽高上限，并连同原生像素测试通过 4/4；同一源码的 quick 门禁通过，执行仓库契约、笔记、Rust 工作区、Linux 设置壳、Linux 原生 94/94、full／wubi 构建安装隔离和 Apple 桥接。其他平台阶段按改动范围跳过，不算作其他宿主设备验收。Python 语法与差异检查通过。

列表内翻页的原生像素测试覆盖公共／排列覆盖顺序、字体 DIP 尺寸、横竖排槽位、均分避让、辅助行分离、禁用字形透明度、禁用命中、悬停／按下、菜单关闭及横排展开优先级。共享主题 26 项测试与 `client-services --all-targets` Clippy（`-D warnings`）通过，规范化分隔线色通过 JSON 输出。

构建机 run `20261020-093731` 通过候选行与普通／2 倍缩放／合成器的横排三组、普通／2 倍缩放的竖排两组；最后一组遇到 Docker daemon 停止，以 `unexpected EOF` 中断。恢复同一服务后，run `20261020-094214` 的竖排合成器与最终原生像素测试均通过。六组真实 GTK 均检查折线与三角两种样式、禁用点击不穿透、不误上屏、按下后拖出取消、真实下一页／上一页、按下时关闭开关不调用旧快照，以及字号／皮肤恢复。横排和竖排截图已实际检查。夹具从启动起显式设置合成字号，恢复的是同一显式字号，不把原来借用的桌面默认字体当成用户选过的字体；候选行高度以清单规定的字号公式断言，不依赖中英字体回退的固定差值。

同一 run `20261020-094214` 的完整 quick 通过：Linux 原生 93/93、Rust 工作区、Android target／Java 宿主、WebAssembly、Linux 设置壳、macOS 原生宿主和共享 Apple 桥接均通过。Windows 本机可运行策略测试 171 项通过、2 项按宿主差异排除、63 项需 Windows 构建；Windows 原生／MinGW／pipe 与 HarmonyOS HAP 因工具链或 staged 输入缺失而跳过。macOS Tauri 工作区项因未暂存 bundle 跳过，Linux 设置壳单独构建通过；不把这些跳过项算作设备验收。最终 Rust 格式、Python 语法、笔记与差异检查通过。

原生测试覆盖横竖排覆盖、DIP/em、独立序号字号、边距与最小宽度；真实 GTK 验收检查配置热更新改变候选几何、文本仍在点击区域内，恢复默认后像素布局恢复。Debian 12 专用容器 run `20261020-055251` 中的 35 项原生、真实 GTK、图片动画与 Wayland 静态回退全部执行通过，无失败或跳过；其中六项普通／2 倍缩放／合成器横竖排窗口同时检查布局与字体热更新、恢复原布局，以及点击新增行高底部正确上屏。

零字号边界另用实际 Cairo 像素验证：序号栏保持空白，候选文字位于保留的 9 DIP 栏宽与 `number_gap` 之后。

额外用父提交 `c8e8c6c99f` 的真实绘制实现作临时对照，三组 point／px 字号乘横竖排共六组未指定布局场景，窗口尺寸和逐像素绘制均一致；不把旧实现复制进长期测试。

完整 quick（构建机 run `20261020-055747`）通过：Linux 原生 93/93、full/wubi 构建与安装隔离、共享 Rust、Android target／Java 宿主、WebAssembly、Linux 设置、macOS 原生及 Apple 桥接均通过。HarmonyOS HAP 因缺打包前置跳过；Windows 原生／MinGW 和 pipe 因工具链缺失跳过，当前宿主可运行的 Windows 策略测试为 171 项通过、2 项按宿主差异排除、62 项需 Windows 构建；参考仓库对照、词库再生成和部分可选检查按原脚本跳过。不将编译当作其他平台设备验收。

Rust 格式、Python 语法、笔记门禁与差异检查通过。

长文本与动态工作区切片在构建机 run `20261020-091209` 通过 8/8：原生像素、候选行，以及普通／2 倍缩放／合成器下横竖排六组真实 GTK。像素覆盖中文、组合字符、emoji、无空格长串、长释义分段颜色、逐行撑高与均分、预编辑换行光标及辅助行翻页留白；GTK 通过真实 socket 异步返回合成长释义，窗口保持在屏幕内，点击新增行高底部仅上屏正文，恢复原配置后基线像素复原。验收同时发现并修复 ASCII 预编辑实际行高小于中文回退字体导致的短布局偏移，原有最小行高保留。

同一 run 通过补充短行高、逐行高亮和动态工作区断言后的原生像素测试、笔记与差异检查，以及 `bash scripts/verify-local.sh --quick`（脚本按改动范围选择阶段）：Linux 原生 93/93、Rust 工作区、Android 目标、Linux 设置壳与 Apple 桥接通过。脚本按本次改动范围跳过 Windows 策略、WebAssembly、Android Java、macOS 和 pipe 阶段；Windows 原生缺 MinGW，HarmonyOS HAP 缺 staged 输入而跳过。此结果只证明实际执行的范围，不外推为其他平台设备验收。

独立光标更新的回归用 GTK 改变输入框文字对齐触发，不输入按键、不重建候选；专用 Xvfb 先收窄工作区，再恢复宽度，并验证过矮工作区恢复桌面候选。固定旧提交 `a10b69c42d` 的三个窗口／宿主文件作临时对照，run `20261020-090428` 在宽度断言处按预期失败；当前实现同一断言与工作区恢复、过矮回退检查在最终六组 GTK 中全部通过。对照只使用本分支已提交文件，结束后恢复当前实现，不保留旧实现副本。

## Consequences

收益是清单中的字体、序号与候选几何进入真实原生绘制，点击复用新的矩形；代价是需要同时维护 Pango 字体单位、分离序号测量和布局覆盖顺序。多行候选按真实文字高度撑高，横排同一行取最大高度。

选中竖条、逐角圆角与预编辑细分颜色由[原生画布外观](../../implemented/feature/2026-10-20-linux-canvas-appearance.md)补充；[释义分段](2026-10-20-candidate-translation-runs.md)补充共享范围和实际分段配色；材质以及 Wayland／IBus 的完整布局消费仍有缺口，不能因此宣称完整 Linux 或全平台视觉兼容。字体按系统字体解析，不添加本地字体文件加载。Windows 基底的出厂几何由[共享出厂布局](2026-10-20-skin-factory-layout.md)提供；原生画布优先读取合并后的横竖排结果，旧 catalog 与普通主题仍沿用既有宿主默认。受限布局增加逐项测量成本；完整列表增加一次显式运行时查询与视图同步，继续遵守候选身份、上屏和异步释义的代次围栏。
