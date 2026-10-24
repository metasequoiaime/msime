# Agent Note: 打字统计契约与纯计算独立于页面

Status: implemented

## Problem

打字统计的宿主客户端契约、活动指标、小时基线和按日明细定义在大型 JSX 页面中，概览组件的类型还反向引用页面。纯计算已有回归，但无法从独立模型模块使用，页面与数据规则的职责纠缠。

## Decision

现有类型、保留期选项和四组纯计算原样归入 `packages/ui/src/settings/typing-statistics-model.ts`。页面与概览直接依赖模型，页面旧路径保留重导出，包入口的公开符号集合保持不变。异步读取、轮询、摘要与本地日期刷新归入 `use-typing-statistics-state.ts`（见[读取状态边界](../simplification/2026-10-23-typing-statistics-state.md)）；日历与按键热力图视图归入 `statistics-heatmaps.tsx`，共用选择按钮归入 `statistics-chart-button.tsx`（见[热力图视图边界](../simplification/2026-10-23-statistics-heatmaps.md)）；分布扇区构造归入纯 `statistics-distribution-model.ts`，分布图与候选命中位置视图归入 `statistics-distributions.tsx`（见[分布模型与视图边界](../simplification/2026-10-23-statistics-distributions.md)）；每日趋势的柱图与手机折线、速度和小时对比归入 `statistics-trends.tsx`，内部保留几何计算与日均说明（见[趋势视图边界](../simplification/2026-10-23-statistics-trends.md)）；按日明细的标题、九列表格、空态和说明归入 `statistics-daily-details.tsx`，只接收已有 `DailyDetailRow[]`，页面继续调用 `dailyDetailRows` 并决定显示条件；手机与 HarmonyOS 摘要外观共用的溢出菜单归入 `statistics-options-menu.tsx`，只接收开关、忙碌状态、可选保留期及动作回调；页面保留 `update` 调用、清空确认和宿主能力判断。提示文案仍在页面。皮肤徽章的 `recordSkin` 仍是可选的后台记录：共享 UI 通过 `recordSkinSafely` 捕获同步抛出和异步拒绝，换肤动作不因统计存储失败而中断或产生未处理异常。

保留 60 秒速度门槛、并列时最早日期、未测活跃时间的 `null`、完整 24 桶小时基线和所有现有过滤行为；不更改存储、宿主契约或 Windows C++ 实现。Windows 的来源注释同步新路径。按日明细保留「汉字 / 字母」列名、千分位与时长格式、速度四舍五入、未知值 `null` 与已测量零值的区别；模型负责最新 30 个记录日和分类折叠，视图按传入顺序显示，不重做筛选或排序。

手机摘要的概览、习惯和按键面板分别归入 `mobile-statistics-overview.tsx`、`mobile-statistics-habits.tsx`、`mobile-statistics-keys.tsx`，只接收摘要、按键计数与卡片 class；共用卡片、图例部件归入 `mobile-statistics-parts.tsx`，外观类型及样式归入 `mobile-statistics-style.ts`。页面保留标签状态、最近七天按键汇总、成就组合和隐私页脚，旧路径继续重导出 `statisticsCardClass` 与 `StatisticsLook`。按键面板内部保留自动布局与手选状态；习惯面板保留 84 格补齐、24 小时桶和构成规则，数字未知值与零仍按原摘要区分。

## Alternatives considered

- 手机摘要面板与 `statistics-heatmaps.tsx` 合并能集中相似图形，但手机的 `phoneHeatmapRows` 描述 HarmonyOS 手机键位，摘要日历也固定为 84 格，和桌面及普通触屏统计的布局、时间范围不同；按既有[热力图边界](../simplification/2026-10-23-statistics-heatmaps.md)保留独立面板。共用部件只在手机摘要内部复用，不把摘要或宿主读取塞入通用图表。

- 菜单直接接收整个宿主客户端和 `update` 能减少几个回调参数，但会把展示组件绑定到统计读取协议；用显式动作回调保留页面的请求去重、错误处理和清空确认边界，菜单不持有异步状态。普通手机模式不传保留期动作，HarmonyOS 摘要外观仅在宿主提供 `setRetention` 时传入；保留原 `details/summary`、样式与忙碌语义。

- 按日表继续内嵌页面可以少一个文件，但九列格式、历史未知值和空态无法脱离宿主读取单独组合；沿用其他统计视图的边界，仅移出完整展示，不把模型筛选规则再次实现到组件中。

- 把计算统一进 Rust 能消除跨语言实现漂移，但需要同时改宿主接口和 React 数据流，超出单片抽取的边界；沿用 [Windows 统计页](../../implemented/feature/2026-10-11-windows-settings-typing-statistics.md) 的取舍。
- 继续从页面导出无需迁移，但概览仍反向依赖页面，纯数据层没有独立入口；保留旧重导出仅作为兼容层。
- 让每个皮肤按钮各自 `void recordSkin()`：改动看似最小，但每个宿主入口都会重新暴露未处理拒绝；统一由模型层的安全包装器接住同步和异步失败，调用方只表达“记录一次”。

## Verification

手机摘要面板迁移前，页面、保留期和手机摘要三文件 48 项通过。新增独立面板测试先因缺少目标模块失败；接入后连同状态边界五文件 52 项通过，覆盖未知与零、摘要与卡片更新、固定日历与小时桶、构成空态及自动布局和手选状态跨更新保留。31 个原声明含页面组合排除导出、改名、注释和格式后与基点一致；旧路径重导出和包入口保持兼容。桌面类型检查与构建、Harmony 类型检查、变更文件 lint、格式、笔记与差异空白检查通过，独立审查未发现问题。本地 `bash scripts/verify-local.sh --quick` 通过，包括 Rust workspace、Android Rust 目标、Linux 桌面外壳与共享 Apple bridge。主机桌面 Rust 包缺少预构建 macOS bundle，Harmony 原生 staging 与 Windows 工具链未准备，对应阶段跳过，其余原生阶段按范围跳过；本片只移动共享 React 展示，没有原生装机验收。

溢出菜单迁移前，页面、保留期和手机摘要三文件 48 项测试通过。独立菜单测试先因缺少目标模块失败；接入后四文件 51 项通过，覆盖受控开关、保留期缺省与能力移除、忙碌禁用及展开状态保留。菜单三组样式常量、全部 className/ARIA/label/role 属性和页面重导出与基点一致。桌面类型检查与构建、Harmony 类型检查、变更文件 lint 和格式、笔记校验与差异空白检查通过，独立审查未发现问题。本地 `bash scripts/verify-local.sh --quick` 通过，包含 Rust workspace、Android Rust 目标、Linux 桌面外壳及共享 Apple bridge；主机桌面 Rust 包缺少预构建 macOS bundle，Harmony 原生 staging 与 Windows 工具链未准备，对应阶段跳过，其他原生阶段按范围跳过。本片没有原生装机验收。首轮 CI 的 5105 项共享 UI 测试中，唯一失败是弹层登记还引用旧页面路径，本地同样复现；把同一条登记迁到新组件路径，原理由和扫描断言保持不变。修正后弹层与菜单两文件 7 项通过，独立审查确认无其他旧路径依赖，本地 quick 再次通过。

按日明细视图迁移前，页面节奏卡与纯活动模型两文件 26 项测试通过；独立组件测试先因缺少待抽取模块失败，接入后三文件 28 项通过。列常量与完整 JSX 排除注释、空白、导出和函数改名后与基点一致；页面全部重导出声明不变。桌面类型检查与构建、Harmony 类型检查、变更文件 lint 和格式、差异空白检查通过。独立审查未发现阻断问题，本地 `bash scripts/verify-local.sh --quick` 通过，包括 Rust workspace、Android Rust 目标、Linux 桌面外壳和共享 Apple bridge 编译。macOS 桌面 Rust 包因未构建输入法 bundle 未在主机检查中编译；Harmony 原生 staging 与 Windows 工具链未准备，对应阶段跳过，其余平台原生阶段按改动范围跳过；本片未做原生装机验收。

迁移前后相同的 9 个统计测试文件、91 项测试通过。纯计算测试直接加载模型，其他页面测试继续覆盖旧入口。迁移实现与旧模块逐字比对（排除注释与空白）一致；临时类型与运行时探针确认旧页面、包入口与模型的类型和函数引用一致，可选摘要及皮肤记录方法保留。新增合成 `recordSkin` 拒绝回归，确认皮肤网格仍完成应用且测试进程没有未处理异常。桌面构建、HarmonyOS 类型检查、偏好字段检查、格式、lint、笔记门禁和本地 quick 是本片的验证门禁。

## Consequences

手机摘要各面板可以脱离页面与宿主读取独立组合，代价是增加显式的内部模块依赖。包入口不增加公开符号，调用旧页面路径的外观类型与卡片函数保持兼容。手机摘要样式只被该组面板复用，不向其他统计外观强加同一布局。

菜单现在可以脱离统计读取和图表渲染单独组合，代价是页面需要显式适配动作回调；原有宿主和包入口的公开契约不增加符号。菜单保留浏览器自己的展开状态，忙碌重绘不会重新创建弹层；不要在菜单内重复保存或执行清空确认。源码扫描的弹层登记必须随菜单归属迁移，继续登记同一个手机/HarmonyOS 菜单，不增加豁免范围。

类型及常量有旧调用方，必须保留原引用与可选方法；纯模块仅依赖纯函数以及类型导入，避免加载 React 或键盘布局。增加模块边界会保留一层兼容重导出，但数据规则获得独立归属。统计存储暂时仍是尽力记录：失败不会提示设置页错误，也不会回滚已经应用的皮肤，因为徽章数据不改变用户选择。
