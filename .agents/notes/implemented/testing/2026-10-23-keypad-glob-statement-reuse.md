# Agent Note: 分组英文查询避免无效的 GLOB 重新编译

Status: implemented

## Problem

固定 `e60da08cb07196e3008a0c83fdb8e4f6961dcc0f`。旧九键采样中英文调用链的 1683 个独占样本有 1025 个位于 SQLite prepare/reprepare 之下。分组英文已经用显式上下界限定索引范围，但 `word GLOB ?3` 仍让 SQLite 把绑定值变化视作规划变化。调用者生成的模式以字符组 `[` 开头，不能提供额外的字面前缀索引范围。

## Decision

右侧已绑定的文本模式通过 `CAST(?3 AS TEXT)` 成为表达式，避免无效的 GLOB 参数失效。生产接口的模式类型为 `&str`，仍以 TEXT 绑定；完整 GLOB、上下界、排序、过滤、限额、解码与失败恢复保持。模式继续参数绑定，不拼接进 SQL。该改变不阻止 schema 变化触发必要的重新编译。

## Historical audit

检索 `GLOB`、`RePrepare`、`sqlite3Reprepare`、参数重新规划及英文语句缓存。既有 [英文页直接追加](../../implemented/testing/2026-10-23-nine-key-english-page-append.md)只处理容器，[完整码先筛选](../../implemented/bug-fix/2026-10-21-keypad-english-filter-before-limit.md)持有查询的候选语义，均部分重叠并保持。`pinyin::push_limit` 的字面量 LIMIT 是同仓库避免参数触发重新编译的先例，非本片机制。语言阅读查询缓冲只复用 SQL 字符串，与 SQLite 规划无关；没有同机制实现或过时提案。

SQLite [查询优化说明](https://www.sqlite.org/optoverview.html#the_like_optimization)明确记录 LIKE/GLOB 直接绑定参数触发重新编译，以及字符组模式没有字面前缀的条件；[语句计数](https://www.sqlite.org/c3ref/c_stmtstatus_counter.html)定义 REPREPARE。产品固定的 rusqlite 0.40.2 暴露该计数，验证使用产品 bundled SQLite。

## Alternatives considered

- **把模式拼成 SQL 字面量**：可让规划固定，但不同数字串不断新增语句文本，扩张缓存且要额外处理转义；保留绑定接口。
- **提高 prepared statement 缓存容量**：能减少 LRU 淘汰，但本问题是同一已命中语句被 SQLite 内部重新编译，容量不能阻止它。
- **合并为数字词网格**：能处理更多重复解码，但已有原型丢失候选、存在模式切换回归；本片不改变搜索空间。

## Verification

生产入口的有效红测执行 8 次、重新编译 8 次；修改后同样 8 次为 0，独立冻结旧 SQL 的同进程对照仍为 8。12 组 EXPLAIN QUERY PLAN 与旧 SQL 相同，继续使用 `word>? AND word<?` 的显式索引范围。另一连接修改 schema 并插入合成行后，原读取连接重新编译一次并看见新行。

完整英文入口的 448 次合成冷热对照与 168 次发布词库冷热对照保持全部字段、顺序和容量；原有行解码、空页及逐页分配测试继续通过。完整测试通过：引擎 2490、golden 31、运行时 362，0 失败；引擎 55 项沿用忽略。发布库、完整验证和单次 ABBA 的可复核结果保存在[证据目录](../../../../docs/measurements/2026-10-23-keypad-glob-statement-reuse/README.md)。

严格 Clippy、fmt、笔记和完整 quick 复核通过。首轮 quick 的 Swift 生命周期构建及语音目录 Cargo 命令超出 120 秒；保留该失败，两项原限制下单独复核和串行契约的完整 quick 均通过。Linux 原生 111 项测试全过；各平台编译与跳过范围见证据目录，不声明设备或 GUI 验收。

## Integration

PR #9400 创建后，`develop` 的 #9385 新增前缀缓存填充查询。以 `5f452db36da91ce49afa7f6e2f66b1a286976c59` 合入时保留缓存，同时把 `KEYPAD_PREFIX_ROWS_SQL` 的模式也转为 TEXT 表达式。新增入口独立冻结旧 SQL：有效红测 8 次执行、8 次重新编译，旧对照同为 8；schema 变更用例扩展覆盖两条查询。专项 4 项绿测通过：原有限额查询与缓存填充查询各执行 8 次，重新编译均为 0；独立旧对照均为 8。两条查询均在 schema 变更后重新编译一次并看到新行，见 `integration-green.log.gz`。

集成后的完整回归通过：引擎 2494 项、golden 31 项、运行时 364 项，零失败；引擎 56 项沿用忽略。固定发布英文词库的 168 次冷热对照和页追加专项 4 项再次通过。严格 engine/runtime all-targets Clippy 也通过。见 `integration-full-tests.log.gz`、`integration-published.log.gz`、`integration-clippy.log.gz`。

集成后的完整 quick 退出 0，包含 Windows 本机 176 项、Linux 原生套件 111 项零失败、macOS 原生宿主与 Apple bridge 构建；各项环境跳过保留在 `integration-quick.log.gz`，Linux 详细结果见 `integration-linux-native.log.gz`。没有设备或 GUI 验收。

原程序哈希、质量和 ABBA 记录仍只对应原固定基线及候选 `e34ab4b31051733def428e296176a3b00a8e1de2`，不充当集成后程序的计时或质量报告；没有重试 ABBA。

## Consequences

同一已缓存查询不再因为不提供额外索引范围的字符组模式重复解析和规划。CAST 在八次合成查询中增加 8 条 VM 指令（716 → 724）；这是指令计数代价，不能当作 CPU 收益。表达式阻止 GLOB 自动提供额外范围，后续若改成字面前缀模式，需重新审视这一决定。固定发布词库只有 `sqlite_stat1`，没有 `sqlite_stat4`；不承诺任意带 STAT4 的自定义库都不发生其它参数相关规划。

不增加缓存、依赖、unsafe、文件写入或平台资源；没有设备或 GUI 验收声明。局部重新编译消除不等于整体预算通过。单次 180 秒启动等待没有满足连续 30 秒条件，`timed=false`，未执行任何计时臂；不重试，不推断净耗时收益或默认九键 16 ms 达标。
