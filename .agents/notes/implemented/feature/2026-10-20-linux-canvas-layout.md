# Agent Note: Linux 原生画布消费皮肤字体与候选布局

Status: implemented

## Problem

[原生 X11 画布](../../implemented/architecture/2026-10-20-linux-native-canvas-x11.md)和[图片动画](../../implemented/feature/2026-10-20-linux-canvas-animation.md)已有真实 GTK 验收，但布局仍写死内边距、候选高度、序号字号与间距。共享 catalog 已解析这些字段，装上同一份 msime-windows 皮肤仍画不出指定布局。固定参考 `msime-windows@dd018f79858314ed542a47c528608752b13bd261` 的 `candidate_skin_tokens.cpp`、`candidate_skin_layers.cpp`、`candidate_skin_scene.cpp` 与 `CandidateList.cpp`。长正文和释义此前没有宽度约束，整个候选窗超出工作区后只能退回桌面面板，无法保持画布皮肤。

## Decision

沿用共享 catalog 的已校验输出，按公共 `geometry`、当前排列覆盖表的顺序消费布局。Pango 保留用户字号，皮肤 `fontFamily` 排在用户字体及其回退链之前；按实际 DIP 字号换算 em 并取整，不能把 Pango 的 point 直接当作 DIP。测量、绘制、点击共用同一份布局与字体，不能只改变绘制偏移。

消费 `padding`、`item_gap`、`item_extra_height`、`number_inset`、`item_padding_right`、`item_min_width`、`number_gap`、`number_scale`、`preedit_margin_dip`、`justify_rows` 和 `min_width_dip`。显式 content insets 仍按每边与九宫格边宽取最大值，未指定时才使用皮肤 padding；`line_gap_dip` 优先于拼音底部 margin。没有相关字段时保留既有布局。横排在可用宽度不足时折到下一行，每行独立撑高与均分。`number_scale = 0` 隐藏序号字形，但仍保留来源的最小 9 DIP 序号栏与 `number_gap`，避免把合法零值误当作移除整个标签。隐藏拼音时本来就不留行距，`trim_hidden_preedit_gap` 无额外动作。`fit_content` 只改变横排显式间距的额外 6 DIP 及内容尾部留白，现有宽度本来就随内容收缩。

长文本遵循固定上游 `CandidateList.cpp` 的可用宽度语义：正文和释义完整换行，竖排只有短正文与整段释义都能放下时才并列，否则释义移到下方。Pango 使用词边界与字符回退换行，保留原文及 UTF-8 分段范围。布局保存每项文字的测量宽度；均分只扩展背景和命中，不让绘制擅自改变断行。预编辑、辅助行和预编辑光标也共用受限布局。短预编辑和辅助行保留原有最小行高，长文本只向下增高，避免 ASCII 与中文字体回退的行高差异改变短文本布局。

X11 在测量前取得光标所在工作区并按缩放换算 DIP；九宫格、outer insets 与挂件参与最终窗口尺寸复核，必要时收紧面板宽度后重测。超高、最小序号栏也放不下或装饰无法容纳时仍回退桌面面板，不裁掉候选文字。默认无宽度限制的测量入口继续供离屏测试使用。候选显示后的独立光标矩形更新也会复核工作区尺寸，变窄或变矮时重新测量并取消旧指针格位；失败返回宿主重建桌面面板，不能只隐藏原生窗而留下空面板。

### 旧笔记审计

- 原生 X11 画布、图片动画：部分重叠，保留窗口所有权和计时规则，互链说明本次补充的布局范围。
- [Windows 布局消费](../../implemented/feature/2026-10-19-windows-candidate-skin-layout-keys.md)：其他宿主的独立实现，参考其固定上游逐键语义，不引入 Windows 依赖。
- [跨平台清单](../../implemented/feature/2026-10-19-windows-skin-schema-parity.md)：保留共享解析契约，本次只改 Linux 原生消费，不增加一套清单校验。
- 静态 classicui、其他平台及生命周期笔记：不改变其决定；Wayland、IBus 与其他桌面 UI 不由本次覆盖。

## Alternatives considered

直接复用 Windows 的布局头可以减少规则重复，但它包含 Windows 卡片自有的 logo、首行和翻译行语义，不适合移植整个尺寸模型。Linux 使用共享已校验字段，Pango 实测自身字体与文字，并按来源语义映射到现有原生窗口。

只在绘制阶段偏移文字最容易保持窗口尺寸，却会导致文字越界、点击区域与高亮错位，因此测量和绘制使用同一份解析结果。

在窗口提交前裁切或省略超长释义能避免重测，却会丢掉上游承诺显示的内容；使用 Pango 的受限布局并把新增高度传入命中矩形。

## Testing

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

选中竖条、逐角圆角与预编辑细分颜色由[原生画布外观](../../implemented/feature/2026-10-20-linux-canvas-appearance.md)补充；[释义分段](2026-10-20-candidate-translation-runs.md)补充共享范围和实际分段配色；材质、列表内翻页与展开面板仍有缺口，不能因此宣称完整 Linux 或全平台视觉兼容。字体按系统字体解析，不添加本地字体文件加载。未指定字段仍使用 Linux 原生默认值，未补齐来源各 base 的出厂几何。受限布局增加逐项测量成本；不改变候选身份、上屏和异步释义的代次围栏。
