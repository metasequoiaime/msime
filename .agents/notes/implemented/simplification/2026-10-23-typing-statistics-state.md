# Agent Note: 打字统计读取与摘要刷新独立维护

Status: implemented

## Problem

`typing-statistics.tsx` 已有独立纯模型，但统计读取、可见性轮询、窗口恢复、摘要合并重读和写入状态仍嵌在图表页面里。复用这些生命周期需要同时理解桌面与手机的全部 JSX，测试异步边界也必须渲染图表。

## Decision

在 `use-typing-statistics-state.ts` 提供 `useTypingStatisticsState(client, look)`，持有统计快照、本地日期、请求占位与时间戳、摘要快照和失败状态，并提供原有 `update` 与打开数据目录的动作。打开目录仍只调用宿主方法并更新同一个错误提示，不触发统计重读或全页忙碌。页面保留标签、所选日期、上传说明、趋势派生值、确认对话框和 JSX。继续复用 `useMountedRef`、`useAsyncGeneration`、`subscribeToWindowResume` 和 `subscribeToInterval`，不新增通用请求框架。

保留 5 秒轮询、1 秒最短读取间隔、超过 15 秒可替换在途读取、隐藏窗口不读取、同签名不替换快照、跨午夜独立推进本地日期，以及摘要读取中再次变脏时完成后补读。`look` 的原值参与 effect 依赖；它只作为摘要展示标识，hook 不依赖手机 UI 的类型或实现。旧请求只在挂载、客户端代次和请求身份都匹配时写回。原页面和包入口的公开 API 保持不变。

## 旧笔记审计

按 `typing-statistics`、`TypingStatisticsPage`、`overviewPollMs`、`summaryDirtyRef`、统计刷新和午夜检索活跃笔记。[纯模型归属](../../implemented/architecture/2026-10-15-typing-statistics-model.md)与本片部分重叠，仅同步异步状态的新位置；[午夜刷新](../../implemented/bug-fix/2026-10-14-statistics-midnight-refresh.md)继续持有日期推进语义，同步 hook 归属并互链；[公共间隔订阅](../../implemented/simplification/2026-10-15-shared-interval-hook.md)继续持有定时器策略，同步统计调用方位置。Windows 原生统计、上传存储串行化、夏令时日历和统计后端笔记属于独立实现或模型算法，无需吸收；没有同范围的过时提案。

## Alternatives considered

- **把状态全部装进一个统计页 hook**：页面可进一步缩短，但会把 UI 标签、图表选择和确认弹窗与宿主读取绑在一起，限制无图表调用方，保留这些状态在页面。
- **把 `look` 变为布尔摘要开关**：接口更窄，但 `harmony` 与 `hm2` 之间切换将不再重建现有 effect，改变轮询相位，故继续传入原字符串标识。
- **借抽取改成统一请求队列或新的 interval hook**：能压缩局部逻辑，但会改变超过 15 秒的请求替换、摘要补读或客户端切换时序；本片仅移动现有实现。

## Verification

迁移前 5 个页面与生命周期测试文件、55 项通过。模块边界检查先因状态模块缺失失败；独立 hook 的 7 项行为测试覆盖读取间隔、在途替换、防重、旧客户端回答、摘要补读、失败重试、日期推进、外观切换与打开目录失败重试。迁移后 10 个相关文件、95 项测试全部通过；独立审查未发现需修复问题，另跑 4 个文件、45 项测试通过。刷新、摘要、写入与目录动作与基点逐字比对一致，全部 JSX 仅目录按钮的动作绑定改变；桌面构建及类型检查、Harmony 类型检查、变更文件 lint 与格式、笔记校验和 `git diff --check` 通过。本地 `bash scripts/verify-local.sh --quick` 通过，涵盖仓库契约、Rust 工作区、Android 目标、Linux 桌面外壳与共享 Apple 桥接；Windows 原生工具链与 Harmony 原生打包环境未就绪，对应阶段跳过，其他无关平台按改动范围跳过。

## Consequences

页面只组合统计读取状态与既有视图，独立测试不再需要加载图表；增加一个专属 hook 模块及其导入。读取 effect 捕获的请求代次与摘要闭包属于已有时序，不应因增加 memo、包装函数或把 `look` 布尔化而改变。每个 hook 实例持有自己的状态，不能引入跨页面共享快照。抽取方式遵循 [React 自定义 Hook 文档](https://react.dev/learn/reusing-logic-with-custom-hooks)中的状态逻辑复用边界。
