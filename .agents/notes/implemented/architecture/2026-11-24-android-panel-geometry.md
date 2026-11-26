# Agent Note: Android 键盘面板高度集中到 PanelGeometry 和 ImePanelLayout

Status: implemented

## Problem

Android 键盘面板的高度没有一处算出来，是三处分别写进视图、再由测量加出来的：

- 顶栏（读音行、候选行、工具栏那一行和补齐空白）在 `updateCandidateViewportHeight` 里算，每次 `render()` 都写；
- 键区各排挂 `KeyboardHeightRole` 标签，由 `ImeStyler.applyKeyboardHeight` 套用几何时按标签算；
- 键盘列的底部内边距和键盘底栏的底边距在 `ImeBottomBar.apply` 里算。

三处的整数运算都夹在读视图、写视图的代码中间，「某个布局、某组设置下面板多高」只能上模拟器量。刷新路径上还有两处重复工作：每次 `render()` 都对读音和读音行里每个文字子视图取一次 `FontMetricsInt`；`CandidateHintRows` 每次刷新都把整页候选重新注释一遍，只为数出提示最多占几行（[14/18 键尺寸与卡顿](../bug-fix/2026-11-17-android-grid-key-metrics.md)）。

## Decision

**算法在 `keyboard/PanelGeometry`，写入在 `core/ImePanelLayout`。**

- `PanelGeometry` 是纯类，只做整数运算，不读视图、不读输入法服务：
  - `topBand(TopBandInputs)`：顶栏各行和补齐空白；
  - `keyRowPx(role, adjustmentDp, nineKeyGridDrawn, density, halfSpacingPx)`：键区一行或一块；
  - `bottomInset(…)`：底部内边距和底栏的底边距；
  - `headroomPx(…)`：API 29+ 停靠时面板上方的透明高度（`HEADROOM_DP`，见[面板之上的层](2026-11-26-android-panel-overlay-layers.md)）。

  数值和原来三处完全相同，`tests/keyboard/PanelGeometrySmoke.java` 钉住：各状态的顶栏、全部高度调整范围内 26 键、九宫格和带底行整块的键区总高、底栏和浮动时的底部留白。
- `KeyboardHeightRole` 从 `MSIMEInputService` 的内部类移到 `keyboard/`，内容不变。
- `ImePanelLayout` 是**顶栏各行、键区各排、键盘列底部留白这三组视图**唯一写高度的地方：`applyTopBand`、`applyKeyRows`、`applyBottomInset`。
  - 服务里的 `updateCandidateViewportHeight` 只剩算预留行数，然后调 `applyTopBand`；
  - `ImeStyler.applyKeyboardHeight` 删掉，调用方改调 `applyKeyRows`；
  - `ImeBottomBar` 仍决定底栏画不画，两次写入改调 `applyBottomInset`。
  - 写入时机和值不变，外观不变。
- **读音行的字形按标签缓存**：记下读音和读音行里每个文字子视图的字号、字体、是否带字体留白、上下内边距，以及设计高度和读音行内边距。这些都没变时直接用上次量出的高度。
- **提示行数按页记下**：`CandidateHintRows` 记住上次数的那一页，引擎给的 `view`、读音表 `candidatePronunciations` 和影响注释行数的开关（五笔编码提示、释义、读音、韩文汉字的训音，按位拼成一个整数）都没变时，直接用上次的行数。数法不变。

## Unchanged

- 新增写这三组视图高度的代码 must 经过 `ImePanelLayout`，算式 must 放进 `PanelGeometry`；`PanelGeometry` never 读视图或服务。
- 读音行缓存只比较标签的字号、字体、字体留白和内边距。读音行加了新的文字子视图时，标签数变了，缓存自动作废；新增一种会改变字形高度、却不改这几项的手段（例如字距、行距）时，must 把它加进比较。
- 提示行数的记忆依赖一条不变式：引擎回复、释义到达时 `view` 整体换成新对象，读音到达时读音表整体换成新表，二者都不就地修改。改成就地修改 `view` 或读音表时，must 同时让 `CandidateHintRows` 失效。新增影响注释行数的开关 must 加进 `flags`。
- 「唯一写入处」只指上面三组视图。还有几处面板几何不经过它：
  - 工具栏打开的面板（功能、候选展开、表情、符号、剪贴板等）的顶边由 `alignOverlaysBelowTopRow` 跟着顶栏实测的底边，`post` 出去，晚一帧；
  - `PanelSurface.onMeasure` 改写非 `fullBleed` 子视图的底边距；
  - `diagnosticView` 按内容包裹高度，显示时替换读音行，不在模型里；
  - 浮动拖动条和 `keyboardSurface` 自身的尺寸由 `ImeStyler.applyKeyboardSurfaceGeometry` 写；面板上方的透明高度（面板的上外边距）由它在套布局参数时调 `ImePanelLayout.applyHeadroom` 写。

## Alternatives considered

- **一步把整块面板做成一个值，工具栏面板也按算出的矩形摆放。** 最强的理由是彻底去掉晚一帧的布局回调，「面板多高」在一处回答完。没这样做：诊断行没有固定高度，浮动拖动条和键盘列上内边距也要进模型，任何一处算错，面板就一直错位，而不只是错一帧。要做对，面板得改成按高度定好的几个容器，触及每个工具栏面板的父视图，改动面大，单独讨论（#11865）。
- **保留面板总高 `panelPx`。** 它只是四个数相加，没有生产代码调用，只有自己的测试。等键区有了按布局声明的总高再加。
- **`CandidateHintRows` 做成只收候选列表的纯函数，加 JVM 测试。** 这样可以在 JVM 上钉住行数。没这样做：数行要调服务的注释方法（释义、读音、五笔编码和训音各有来源），拆成纯函数得先把注释器拆出来，超出这次的范围。改为按页记忆，数法原样保留。
- **只加缓存，不搬算法。** 改动更小，也能去掉重复的字形测量。但高度仍然只能上设备量，`KeyboardHeightRole` 和三处写入照样散在服务、`ImeStyler` 和 `ImeBottomBar` 里。

## Consequences

- **收益**：
  - 面板高度在 JVM 上有测试；
  - 服务净少约 100 行，`ImeStyler` 少 24 行；
  - 打字时每次刷新只比较几个数：读音行不再取字形，同一页候选不再重新注释。
- **代价**：
  - 多了两个类和一层调用；
  - 提示行数的记忆依赖「整体替换、不就地修改」，这一点没有测试钉住，只靠上面的不变式和代码审查；
  - 工具栏面板的位置仍晚一帧（见不变式最后一条）。
- **验证**：
  - `-Werror` 编译、全部 JVM 冒烟、设备测试包编译通过。
  - 模拟器 API 35、29、28 上对比改前改后：14 种布局、10 组设置，空闲和组字，外加 API 35 横屏，共 136 组，`keyboardSurface` 下每个可见视图的纵向位置和高度逐像素相同。
  - 每次 `render()` 耗时的中位数，打「beijingshi」加空格，两版交替各测 3 轮：API 35 1760 → 1641 µs，API 29 2535 → 2322 µs，API 28 2675 → 2718 µs。API 28 那一项在改前自己各轮的波动范围（2446–2839 µs）之内。
  - 没覆盖到：真正的释义行（需要翻译目标）、手势导航下的键盘底栏（模拟器用三键导航）、分离键盘、设置之间的组合、API 29 和 28 的横屏。没有上真机。
- **相关笔记**：读音行高度和字体留白的规则见 [读音字号](../bug-fix/2026-10-09-android-reading-font-size.md)；高度调整的封顶见 [键盘高度范围](../feature/2026-10-08-android-keyboard-height-range.md)；底行分摊见 [底行等高](../feature/2026-10-10-android-equal-bottom-row.md)。
