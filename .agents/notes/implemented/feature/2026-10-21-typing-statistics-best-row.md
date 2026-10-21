# Agent Note: macOS 打字统计概览补上「最好成绩」一行

Status: implemented

## Problem

共享桌面设置页的「统计概览」在六格汇总下面只有一行方法说明，缺了 Windows 原生页的一行成绩。

- Windows 原生设置在 `platforms/windows/settings/main.cpp:14944-14958` 用两列 `Grid` 并排画 `typing::overview_details(activity).summary`（左）和 `.best`（右），后者右对齐。
- `best` 的措辞在 `platforms/windows/settings/TypingStatisticsModel.h:610-619`：有记录时是「最多 M月D日，N 字符」，有最快日时追加「 · 最快 M月D日，S 字 / 分钟」；一条记录都没有时是「还没有记录」。
- 共享页这边，`packages/ui/src/settings/typing-statistics-overview.tsx:102-109` 的 `statisticsOverviewDetails` 早就把 `summary` 和 `best` 都算了出来，但 `packages/ui/src/settings/typing-statistics.tsx` 的桌面概览只用了 `.summary`（放进「累计输入」一格的 `note`），`.best` 没有任何 JSX 引用，macOS 因此看不到 Windows 那一行。

需要说清的是，单日最多和单日最快的**事实**在 macOS 上并非完全不可见：「今日输入」和「今日速度」两格的 `note` 分别写了「单日最多 N 字符 · 日」和「单日最快 S 字 / 分钟 · 日」。缺的是 Windows 把这两件事合并成一行、放在六格正下方的那条 `best`。

## Decision

- 桌面概览在六格和方法说明之间加一行：右对齐、`11px`、`--p-sub` 的 `<p>`，内容直接取 `statisticsOverviewDetails(activity).best`。日期格式、千分位和四舍五入都沿用现有纯函数，逐字对齐 Windows 的 `overview_details`。
- `typing-statistics.tsx` 从 `./typing-statistics-overview` 显式 `import` `statisticsOverviewDetails`：它原先只作为包入口的重导出存在，模块作用域里没有绑定。
- 改动只落在共享桌面分支，`mobile` 为真且带 `look` 的 HarmonyOS 摘要版（`MobileStatistics`）不受影响；macOS、Linux 等渲染这个桌面分支的宿主一并补上这一行。

## Alternatives considered

- **照 Windows 画完整两列，`summary` 靠左、`best` 靠右，并把「累计输入」格里的 `summary` 说明删掉**：这是最贴近 Windows 版式的做法，也避免同一句「日均…天有记录」出现两次。没有采用，因为本任务只要求补 `best`，删格内说明会改动 `statisticsOverviewMetrics` 的输出和它已有的单测断言，超出「补一行」的范围。
- **把「今日输入」「今日速度」两格里的单日最多 / 最快 `note` 删掉，只留这一行**：信息不再重复，也更接近 Windows 的六格定义（Windows 这两格没有 `note`）。没有采用，理由同上：那是重排现有信息，不是补充缺失信息，会让已有回归断言失效。
- **不渲染 `best`，认定格内 `note` 已经够用**：最省事，且用户确实能从两格看到单日最多和单日最快。没有采用，因为 Windows 有一行独立的最好成绩，macOS 缺它，属于已确证的功能缺口；`statisticsOverviewDetails` 的 `best` 长期没有消费者本身就是信号。

## Consequences

- macOS（以及同样走共享桌面分支的 Linux）概览下方多一行最好成绩；单日最多 / 最快的合并写法与 Windows 一致。
- 「今日输入」「今日速度」两格的 `note` 仍在，这一行与它们是同一批事实的两种写法，桌面上会各出现一次。日后若决定只保留一处，按上面的备选方案重排即可，本行和纯函数都不用动。
- 无记录的文档显示「还没有记录」，与 Windows 的空状态一致；只有 `bestDay` 而没有 `fastestDay` 时只显示「最多…」半句，与 Windows 相同。
- 没有新增计算，只是把一个已有字符串接到了 JSX 上。

## Verification

- `apps/desktop/tests/settings/typing-statistics.test.tsx` 新增用例「desktop statistics render the best day and fastest day line」：构造昨天 100 字 / 2 分钟、前天 20 字 / 2 秒的合成统计，断言页面出现「最多 &lt;昨天&gt;，100 字符 · 最快 &lt;昨天&gt;，50 字 / 分钟」。改前该用例以 `TestingLibraryElementError: Unable to find an element` 失败，改后通过。
- `pnpm --filter @msime/desktop test` 全绿。
- `pnpm run format:check` 通过。

## Related notes audit

- [Windows 原生设置窗口自己画「打字统计」页](2026-10-11-windows-settings-typing-statistics.md)：`TypingStatisticsModel.h` 逐条移植共享页的派生数字，本篇补回它移植过、而共享页后来没渲染的 `best`。部分重叠，互链。
- [打字统计契约与纯计算独立于页面](../architecture/2026-10-15-typing-statistics-model.md)：`statisticsOverviewDetails` 留在概览组件里，不随纯计算迁入模型；本篇不改这个归属。决定不变。
