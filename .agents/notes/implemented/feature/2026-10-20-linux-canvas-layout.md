# Agent Note: Linux 原生画布消费皮肤字体与候选布局

Status: implemented

## Problem

[原生 X11 画布](../../implemented/architecture/2026-10-20-linux-native-canvas-x11.md)和[图片动画](../../implemented/feature/2026-10-20-linux-canvas-animation.md)已有真实 GTK 验收，但布局仍写死内边距、候选高度、序号字号与间距。共享 catalog 已解析这些字段，装上同一份 msime-windows 皮肤仍画不出指定布局。固定参考 `msime-windows@dd018f79858314ed542a47c528608752b13bd261` 的 `candidate_skin_tokens.cpp`、`candidate_skin_layers.cpp` 与 `candidate_skin_scene.cpp`。

## Decision

沿用共享 catalog 的已校验输出，按公共 `geometry`、当前排列覆盖表的顺序消费布局。Pango 保留用户字号，皮肤 `fontFamily` 排在用户字体及其回退链之前；按实际 DIP 字号换算 em 并取整，不能把 Pango 的 point 直接当作 DIP。测量、绘制、点击共用同一份布局与字体，不能只改变绘制偏移。

消费 `padding`、`item_gap`、`item_extra_height`、`number_inset`、`item_padding_right`、`item_min_width`、`number_gap`、`number_scale`、`preedit_margin_dip`、`justify_rows` 和 `min_width_dip`。显式 content insets 仍按每边与九宫格边宽取最大值，未指定时才使用皮肤 padding；`line_gap_dip` 优先于拼音底部 margin。没有相关字段时保留既有布局。当前原生横排只有一行，均分只作用于该行真实存在的候选。`number_scale = 0` 隐藏序号字形，但仍保留来源的最小 9 DIP 序号栏与 `number_gap`，避免把合法零值误当作移除整个标签。隐藏拼音时本来就不留行距，`trim_hidden_preedit_gap` 无额外动作。`fit_content` 只改变横排显式间距的额外 6 DIP 及内容尾部留白，现有宽度本来就随内容收缩。

### 列表内翻页

固定参考同一上游的 `CandidateRowWithPager`、`PagerArrows.cpp` 和 `candidate_skin_scene.cpp`，来源和 GPL 许可记入[第三方清单](../../../../docs/third-party.md#linux-原生画布翻页)。原生画布消费 `pageArrows`、公共及排列覆盖的 `pageArrowStyle`；折线箭头用序号色，圆角三角用候选文字色，尺寸跟随候选 DIP 字号。横排在列表右侧留出整列，箭头位于最后一行底部，折线按基础候选行高抬起；三角在箭头前显示 `page_arrow_divider`。竖排另起矮行，上一页字形左端对齐序号左缘。辅助信息单独占行，不再把翻页塞在其尾部；均分只分配箭头之外的候选区域。

`pageArrows` 缺省关闭有明确适用边界：`load_native_canvas` 只接管 `canvas = true` 的包，上游 `canvas` 开关沿用 `fluent`，其出厂默认关闭。非 canvas 基底仍由已有桌面面板处理，不在 Linux 复制所有基底的开关表。显式 `false` 隐藏箭头，横排的展开开关优先于翻页开关，竖排不受展开开关影响；展开面板自身仍待补齐。开关不改变运行时是否可翻页，也不改变已有鼠标滚轮偏好。

两侧箭头始终占位，不可用的一侧以来源透明度绘制，吞点击但不高亮、不调用翻页。命中、绘制与 X11 输入形状共享矩形；指针状态不修改键盘选中项，翻页复用原 `CandidateList` 快照，配置／候选刷新清掉旧按下状态。菜单没有候选翻页区。共享主题接口增加可选规范化颜色 `candidate_details.page_arrow_divider`，缺省不输出，保留其他宿主的读取兼容。

### 旧笔记审计

- 原生 X11 画布、图片动画：部分重叠，保留窗口所有权和计时规则，互链说明本次补充的布局范围。
- [Windows 布局消费](../../implemented/feature/2026-10-19-windows-candidate-skin-layout-keys.md)：其他宿主的独立实现，参考其固定上游逐键语义，不引入 Windows 依赖。
- [跨平台清单](../../implemented/feature/2026-10-19-windows-skin-schema-parity.md)：保留共享解析契约，本次只改 Linux 原生消费，不增加一套清单校验。
- 静态 classicui、其他平台及生命周期笔记：不改变其决定；Wayland、IBus 与其他桌面 UI 不由本次覆盖。

## Alternatives considered

直接复用 Windows 的布局头可以减少规则重复，但它包含 Windows 卡片自有的 logo、首行和翻译行语义，不适合移植整个尺寸模型。Linux 使用共享已校验字段，Pango 实测自身字体与文字，并按来源语义映射到现有原生窗口。

继续在辅助行末尾画文本箭头最容易保留旧窗口结构，但不能表达皮肤开关、横排独立列、禁用状态与三角分隔线，因此翻页是参与测量的独立区域。

只在绘制阶段偏移文字最容易保持窗口尺寸，却会导致文字越界、点击区域与高亮错位，因此测量和绘制使用同一份解析结果。

## Testing

列表内翻页的原生像素测试覆盖公共／排列覆盖顺序、字体 DIP 尺寸、横竖排槽位、均分避让、辅助行分离、禁用字形透明度、禁用命中、悬停／按下、菜单关闭及横排展开优先级。共享主题 26 项测试与 `client-services --all-targets` Clippy（`-D warnings`）通过，规范化分隔线色通过 JSON 输出。

构建机 run `20261020-093731` 通过候选行与普通／2 倍缩放／合成器的横排三组、普通／2 倍缩放的竖排两组；最后一组遇到 Docker daemon 停止，以 `unexpected EOF` 中断。恢复同一服务后，run `20261020-094214` 的竖排合成器与最终原生像素测试均通过。六组真实 GTK 均检查折线与三角两种样式、禁用点击不穿透、不误上屏、按下后拖出取消、真实下一页／上一页、按下时关闭开关不调用旧快照，以及字号／皮肤恢复。横排和竖排截图已实际检查。夹具从启动起显式设置合成字号，恢复的是同一显式字号，不把原来借用的桌面默认字体当成用户选过的字体；候选行高度以清单规定的字号公式断言，不依赖中英字体回退的固定差值。

同一 run `20261020-094214` 的完整 quick 通过：Linux 原生 93/93、Rust 工作区、Android target／Java 宿主、WebAssembly、Linux 设置壳、macOS 原生宿主和共享 Apple 桥接均通过。Windows 本机可运行策略测试 171 项通过、2 项按宿主差异排除、63 项需 Windows 构建；Windows 原生／MinGW／pipe 与 HarmonyOS HAP 因工具链或 staged 输入缺失而跳过。macOS Tauri 工作区项因未暂存 bundle 跳过，Linux 设置壳单独构建通过；不把这些跳过项算作设备验收。最终 Rust 格式、Python 语法、笔记与差异检查通过。

原生测试覆盖横竖排覆盖、DIP/em、独立序号字号、边距与最小宽度；真实 GTK 验收检查配置热更新改变候选几何、文本仍在点击区域内，恢复默认后像素布局恢复。Debian 12 专用容器 run `20261020-055251` 中的 35 项原生、真实 GTK、图片动画与 Wayland 静态回退全部执行通过，无失败或跳过；其中六项普通／2 倍缩放／合成器横竖排窗口同时检查布局与字体热更新、恢复原布局，以及点击新增行高底部正确上屏。

零字号边界另用实际 Cairo 像素验证：序号栏保持空白，候选文字位于保留的 9 DIP 栏宽与 `number_gap` 之后。

额外用父提交 `c8e8c6c99f` 的真实绘制实现作临时对照，三组 point／px 字号乘横竖排共六组未指定布局场景，窗口尺寸和逐像素绘制均一致；不把旧实现复制进长期测试。

完整 quick（构建机 run `20261020-055747`）通过：Linux 原生 93/93、full/wubi 构建与安装隔离、共享 Rust、Android target／Java 宿主、WebAssembly、Linux 设置、macOS 原生及 Apple 桥接均通过。HarmonyOS HAP 因缺打包前置跳过；Windows 原生／MinGW 和 pipe 因工具链缺失跳过，当前宿主可运行的 Windows 策略测试为 171 项通过、2 项按宿主差异排除、62 项需 Windows 构建；参考仓库对照、词库再生成和部分可选检查按原脚本跳过。不将编译当作其他平台设备验收。

Rust 格式、Python 语法、笔记门禁与差异检查通过。

## Consequences

收益是清单中的字体、序号与候选几何进入真实原生绘制，点击复用新的矩形；代价是需要同时维护 Pango 字体单位、分离序号测量和布局覆盖顺序。多行候选按真实文字高度撑高，横排同一行取最大高度。

选中竖条、逐角圆角与预编辑细分颜色由[原生画布外观](../../implemented/feature/2026-10-20-linux-canvas-appearance.md)补充；[释义分段](2026-10-20-candidate-translation-runs.md)补充共享范围和实际分段配色；材质、长文本换行与展开面板仍有缺口，不能因此宣称完整 Linux 或全平台视觉兼容。字体按系统字体解析，不添加本地字体文件加载。未指定字段仍使用 Linux 原生默认值，未补齐来源各 base 的出厂几何。
