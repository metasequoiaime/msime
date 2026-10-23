# Agent Note: 统计分布模型与视图独立于页面

Status: implemented

## Problem

`typing-statistics.tsx` 同时持有字符、来源和语言模式的分组映射、环形图及排行绘制、候选命中位置视图与整页编排。分布只需要已选范围的计数，却必须加载统计页才能组合与验证；表格、趋势和生命周期与这些职责没有共同状态。

## Decision

把既有扇区类型、调色板、标签和符号映射、三组扇区构造移入纯 `statistics-distribution-model.ts`，让 `statisticsDistributionSlices` 接收既有 `TypingBreakdown`。把 `Distribution`、私有 `ShapeChart` 和 `CandidateRanks` 移入 `statistics-distributions.tsx`。两种排行共用原样迁移的行样式。页面继续持有范围选择和 `scopedBreakdown`，只调用模型、组合视图；模型不引用 React 或页面。不增加包入口导出。

保持现有分组、颜色、符号、顺序、缺省值、空态、百分比与全部 JSX；来源按计数排序，候选位置始终按位置排序，两种排行不合并数据规则。环形图几何继续复用 `typing-chart.ts`。手机摘要的 `summaryComposition` 与 `summaryMethods` 有不同分组，不在搬移时合并。

## 旧笔记审计

按 `typing-statistics`、`typing-chart`、候选命中位置和分布图检索活跃笔记。[统计纯模型](../../implemented/architecture/2026-10-15-typing-statistics-model.md)部分重叠，更新剩余图表归属并互链；[热力图视图](../../implemented/simplification/2026-10-23-statistics-heatmaps.md)与[读取状态](../../implemented/simplification/2026-10-23-typing-statistics-state.md)保留独立边界。[Windows 统计页](../../implemented/feature/2026-10-11-windows-settings-typing-statistics.md)、各语言新增来源、云上传和午夜刷新笔记持有其他实现或规则，不由本篇接管。没有相同范围的过时提案。

## Alternatives considered

- 把分布、热力图和趋势合成一个图表模块：可以减少文件数量，但无共同状态的视图仍捆绑加载与验证，已有热力图边界失去意义；按分布模型与视图分开。
- 把标签表统一成摘要手机的构成模型：可以减少两份分类代码，但手机把符号合并、把输入方式按键盘形态归类，语义不同；保留既有映射。
- 只移视图、继续让页面拼扇区：迁移更小，但页面仍持有大量静态映射和纯构造逻辑；让纯模型接收已有分解结果，避免向视图传整份宿主统计。

## Verification

迁移前 3 文件、63 项测试通过；新增两文件先因待抽取模块不存在失败，新增实现后 7 项独立测试通过；页面接入后相同基线及新测试合计 5 文件、70 项通过。分组类型与常量、三组构造、分布/形状/候选视图及样式排除注释、空白和导出修饰符后与基点一致，页面函数除三组构造的委托外一致，包入口未改。桌面构建和类型检查、Harmony 类型检查、变更文件 lint、格式通过；笔记门禁通过。独立审查确认生产实现无阻断问题；按反馈把不变性断言改为检查实际传入数组，修正后独立两文件 7 项再次通过。首次 quick 的 Swift 豆包测试冷编译超过原有 120 秒期限；随后因构建盘空间不足终止该轮，将 worktree 迁至容量充足的开发目录。重建路径缓存后，两文件 7 项及 Swift 豆包 17 项再次通过；从新位置完整运行 quick 成功，Rust 工作区、Android 目标、Linux 桌面外壳和 Apple 共享桥接编译通过。原脚本按范围跳过无关平台阶段；缺少 ArkTS 构建输入、Windows 原生工具链和 macOS 原生资源的检查仍跳过，韩文词典再生成、GNU 校验工具和大小写敏感文件系统回归也未在本机执行。没有调整超时或跳过钩子。

CI 首轮在与更新后的 `develop` 组合时暴露插件作者 SDK 缺少 `grid_limit` 常量；补齐 C/Rust 两份模板并同步[九宫格上限笔记](../feature/2026-10-23-nine-key-digit-limit-pick.md)，既有 SDK 测试本地先复现失败、修复后两项通过。共享 UI 首轮另有既有皮肤编辑器测试抢读 `ColorRow` 的副作用同步前色值；最终色值断言改为等待实际目标颜色，修正后该文件 11 项测试、lint 和格式检查通过。

同步 `develop` 并修正两处 CI 问题后，完整全范围 quick 通过：Windows 本机 164 项测试、Linux 原生 111 项测试与两版安装检查、Rust 工作区、Android 目标、wasm 编译及 clippy、Android Java、Linux 桌面外壳、macOS 原生与 Apple 共享桥接编译均通过。ArkTS 构建、Windows 交叉编译和 pipe-only 仍缺少本机输入或工具链；macOS 桌面 Rust 外壳仍因规定位置缺少原生资源而未编译。

## Consequences

页面移出 423 行内嵌实现，纯分布构造和无状态视图可以独立使用，增加两个内部模块。两种排行共享样式，数据顺序各自保持；手机摘要继续使用自己的分组模型。搬移遗漏颜色、语言分组或零未知项的显隐会改变图表，纯模型与既有页面同时覆盖；不声称覆盖原生宿主装机或其他平台原生统计实现。
