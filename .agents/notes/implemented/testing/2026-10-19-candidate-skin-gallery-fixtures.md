# Agent Note: 候选皮肤验证图库（提交在仓库里的验证皮肤包）

Status: implemented

## Problem

共享 catalog 已经按 msime-windows `c0a9d371` 解析版式表、外框、阴影、材质、翻页箭头、收紧宽度、卷轴展开、`canvas` 九宫格与挂件、v2 动效和全部候选颜色写法（见 [跨平台候选皮肤清单与 msime-windows 对齐](../feature/2026-10-19-windows-skin-schema-parity.md)），Windows 宿主也开始逐项绘制它们（见 [Windows 候选窗绘制皮肤的版式、外框、阴影、翻页箭头与收紧宽度](../feature/2026-10-19-windows-candidate-skin-layout-keys.md) 和 [Windows 候选窗绘制皮肤的卷轴展开](../feature/2026-10-19-windows-candidate-skin-expand-panel.md)）。但 catalog 的单测都是在临时目录里现写的最小清单和 1 字节或纯黑的图片，只证明规则，不是一个真实宿主能装上、画出来、截图对照的包；各宿主要做视觉验收时，只能各自临时拼皮肤，或者借用 msime-windows 的出厂皮肤，而后者不能复制进本仓。

## Decision

`crates/client-services/tests/fixtures/candidate-skins/<id>/` 放一批提交在仓库里的验证皮肤，每个包是一套完整、配色可读的设计，专门演示一组键，id 一律以 `verify-` 开头：

- `verify-layout-em`：版式全部用 em 长度，横排、竖排表各自覆盖几项，含序号缩放、序号缩进、选中竖条宽高与位置、贴角高亮、拼音行外边距（四值、两值、一值三种写法）。
- `verify-frame-strong` / `verify-frame-none`：3 DIP 外框加 `strong` 阴影；无外框、无阴影加 `fit_content`。
- `verify-arrows-chevron`（`microsoft` 外观基底，颜色和圆角由外观补齐）/ `verify-arrows-triangle`（带 `page_arrow_divider`）：列表翻页箭头的两种样式。
- `verify-expand-panel`：`wechat` 外观基底、只支持横排的卷轴展开，`expand_rows = 3`。
- `verify-canvas`：`canvas` 基底的九宫格（深浅两张）、内容内边距、外留白、行距、三个自由挂件（其中一个是 6 帧 APNG）和 `[animation]`。
- `verify-v2-motion-system`：`system` 基底上的 v2，三个动效预设都写出，装饰图是 8 帧 APNG 并带 `motion = "float"`。
- `verify-color-syntax`：catalog 接受的每一个候选颜色键、右键菜单和工具栏颜色键，七种颜色写法混用，配 `backdrop = "acrylic"` 和两个材质参数。
- `verify-backdrop-mica`：`backdrop = "mica"`，深浅两套半透明底色和材质参数，验证 Windows 的云母（见 [Windows 候选窗按皮肤垫系统材质](../feature/2026-10-19-windows-candidate-backdrop-material.md)）。
- `verify-canvas-keys-ignored`：`system` 基底上写了五个画布专属键，引用的图片故意不在包里，证明它们不校验、只给一条提示，动效照常。

图片由同目录的 `generate.py` 生成：只用 Python 标准库（`zlib`、`struct`、`tomllib`），自己编码 PNG 和 APNG，没有随机数，重跑逐字节相同；预览图的颜色和几何直接读各包的 `skin.toml`，改了清单重跑一次即可。图片总共一百多 KB，单张都在几 KB 到十几 KB。

`crates/client-services/src/skin/catalog/gallery_tests.rs` 用真实的 `scan` / `load_package` 读这个目录：目录里的包必须与登记的列表一致、全部无问题地加载；逐个核对每个包要演示的解析结果（em 与 DIP 两种长度形式和它们的 JSON、横竖排覆盖、外框阴影材质、翻页箭头、收紧宽度、展开、九宫格与挂件与内边距、动效预设和 `declared`、装饰图动效与帧数、`canvas` 的回退配色与 `panelCandidate`、颜色原文与规范化结果、提示文字）；APNG 逐帧完整解码。同一个文件还把社区打包（`candidate_community::pack_as`，公开发布）和 MCP 创建（`create_candidate_skin` 调用的 `candidate_community::install`，传清单原文和它引用的全部图片）对每个包的结果写成断言：除 `verify-canvas` 外都接受；`verify-canvas` 打包被 `SERVER_UNSUPPORTED_BASES` 拒绝（`candidate_skin_package`），创建因引用 6 张图超过 3 张的上限被拒绝（`candidate_skin_file_path`）。msime-cloud 接受 `canvas` 之后，这张表要跟着改。

Windows 的截图工具渲染的也是这批包，这是它们放在共享 crate 的 fixtures 下、而不是放在某个平台目录里的原因：同一批包既是 catalog 的解析断言输入，也是各宿主视觉验收的输入。

## Alternatives considered

- **沿用临时目录里现写清单的做法，只在单测里加断言**：最强的理由是自包含，不往仓库里放二进制文件，现有 catalog 测试就是这样写的。不采用：临时目录里的包用完即删，宿主截图工具拿不到；1 字节或纯黑的图片也不能拿来看渲染结果。
- **用 Pillow 之类的图像库画图，或直接提交手绘图片**：画起来更省事，效果也可以更精细。不采用：Pillow 不在仓库的工具链里，让一个测试 fixture 多一个 Python 依赖不划算；手绘图片没法按清单颜色重新生成，改一次配色就要重画一次。标准库自己编码 PNG / APNG 只要几十行。
- **把这些用例加进共享用例表 `client_dialect.json`**：那张表也被 msime-cloud 和 msime-windows 同步使用。不采用：它只核对接受与否和拒绝原因，生成的文件都是 1 字节，表达不了解析出的具体值和真实图片；而且改它会牵动另外两个仓库的同步。

## Consequences

收益：每个兼容 msime-windows 的皮肤键都有一个真实、可安装、看得出效果的包，catalog 解析结果和宿主绘制结果可以对着同一个包核对；社区打包与 MCP 创建对 `canvas` 的限制有了一条会在规则变化时失败的断言。

代价：仓库里多了十二个包和一百多 KB 的生成图片；改清单或图片时，`gallery_tests.rs` 的断言要一起改，新增包要登记进 `GALLERY`。预览图只是示意（候选字画成笔画块、拼音和序号是点阵字、1 DIP 按 1 像素），不代表任何宿主的逐像素外观。

验证：`rbuild cargo test --locked -p msime-client-services skin::` 通过（277 项，其中本图库 13 项）；`rbuild cargo clippy --locked -p msime-client-services --all-targets -- -D warnings` 无警告；`cargo fmt --all -- --check` 通过；`generate.py` 重跑后所有图片的哈希不变。

Android 交叉编译的测试程序带着构建主机的 `CARGO_MANIFEST_DIR`，设备没有那个源码目录。2026-10-19 的 API 28/35 整套 services 回归因此有 13 个图库用例找不到夹具。该容量分支最初由设备 runner 复制图库，再通过 `MSIME_CANDIDATE_SKIN_FIXTURES` 传入测试路径。合入 `develop` 的[社区画布资源实现](../feature/2026-10-19-community-canvas-resources.md)后，沿用上游的 `include_bytes!` 嵌入夹具并写入临时目录；移除重复复制和环境变量约定。测试程序自带素材，不再要求 runner 同步图库，也不依赖设备上存在构建机路径；图库用例全部保留。

容量分支补入当时 `develop` 后，API 28、35 的 services 整套单测均为 1167 项通过、9 项忽略、4 项按既有硬链接规则过滤；图库 15 项实际执行通过。此结果包含社区画布扩展后的用例，详见[容量验证记录](../bug-fix/2026-10-19-dictionary-collection-capacity.md)。
