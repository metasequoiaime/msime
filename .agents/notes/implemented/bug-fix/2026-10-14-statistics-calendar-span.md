# Agent Note: 手机统计趋势按日历字段计算跨度

Status: implemented

## Problem

`mobileTrendLength` 用最早记录到本地今天午夜的毫秒差除以 24 小时，再向下取整。跨进入夏令时的 23 小时日或半小时偏移后，结果少一天；`recentDays` 据此生成手机趋势范围，最早有统计的日期被漏掉。记录本身按本地 `YYYY-MM-DD` 保存，计算的是日期个数，不是已经经过多少个完整的 24 小时。

## Decision

先取用户本地今天的日期字段，和最早记录一起用 UTC 午夜坐标计算日期差。UTC 只作为日历字段的算术坐标，不能把今天改为当前 UTC 日期；图表日期和标签继续由 `recentDays` 按本地日期生成。30～366 天的范围限制及无记录时的默认值保持原有契约。

## Alternatives considered

把向下取整换成四舍五入，是一行改动，也能补上常见的半小时或一小时差。但跨度依然取决于本地午夜的时区偏移，不能表达纯日期计数的约束；直接比较日期字段更明确。

按 `addDays` 一天一天推进，能沿用已有的纯日历计算。但为一个跨度枚举日期还需要迭代及停止条件；两个日期的 UTC 坐标相减即可完成同一计算。

## Consequences

UTC 算术不能替代本地日期标签生成，否则接近 UTC 换日的用户会看到错误的今天。测试必须同时覆盖进入、退出夏令时、半小时偏移，以及本地日期与 UTC 日期不同的瞬间。

跨夏令时的趋势能包含最早有记录的一天，范围计数不依赖午夜的时区偏移。实现依赖标准日期时间字符串的 UTC 解析，与已有 `addDays` 相同；本地日期字段和 UTC 算术坐标仍有不同职责，后续改动必须继续保留这个区别。

## Verification

`statistics-calendar-span.test.ts` 固定时区和时钟，检查实际 `mobileTrendLength` 与 `recentDays` 生成的范围。旧实现九项中五项失败：纽约、柏林、悉尼、Lord Howe 和纽约晚间的范围都漏掉最早一天；退出夏令时、东京及默认范围四项通过。修复后九项全部通过。

与统计页、节奏和日期格式测试一起运行，四个文件共 48 项通过。测试既检查第一个和最后一个日期，也检查 46 天跨度，以及默认 30 天和最长 366 天的边界；每项结束后恢复时区和时钟。验证层级为共享 JavaScript 日期模型及现有 React/jsdom 统计页回归。

前端类型检查与生产构建、改动文件 lint 和格式检查、`pnpm run verify-notes`、`git diff --check`、`bash scripts/verify-local.sh --quick` 通过。

## 历史检索与参考

已按 `mobileTrendLength`、夏令时、`DST`、趋势天数和本地午夜检索活跃笔记，没有持有这条跨度决定的笔记。`crates/client-core/src/calendar.rs` 与共享 UI 的 `addDays` 已采用按日历字段运算的边界。[MDN 的 Date 文档](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Date#date_components_and_time_zones)说明本地偏移随日期和夏令时变化；这里沿用项目的 UTC 日期坐标方式。
