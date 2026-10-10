# Agent Note: 日期时间模式新增农历关键词 nl / nongli / yinli

Status: implemented

## Problem

#5952：用户想在聊天、写日记时快速输入农历日期。T 模式（Shift+T）早就能出农历，但只排在 `rq` / `riqi` / `date` 列表的最后一行（第 17 行），软键盘上要翻好几页才看得到，也没有任何关键词直接对应它；`/` 命令模式和服务端 `/v1/input/datetime` 同样只认日期、时间、星期三组关键词。

## Decision

- `crates/engine/src/local/date_time.rs` 的 `query_date_time_with_limit` 认 `nl`、`nongli`、`yinli` 三个关键词，给出三行：农历日期（`丙午年六月二十七日`，与 `rq` 列表末行同一个 `lunar_date`），再加上带星期的两种写法（`… 星期日`、`… 周日`）。日历换算不了的日期（超出 lunar-lite 覆盖的农历 1850–2150 年、零时钟、不存在的日期）三行都不给，不出只有星期的残行，与 `rq` 列表丢掉农历行的做法一致。
- 同一组关键词同步到另外两处白名单：`backend/local_modes.rs` 的 `DATE_TIME_KEYWORDS`（服务端只回答名单里的关键词），`local/command.rs` 的 `BUILTINS`（`/` 模式的内置命令，标题「农历」）。`/` 模式不带字母时也列出农历命令的首行，前缀 `n`、`y` 等按既有规则匹配到它。
- 设置页 `packages/ui/src/settings/local-modes-section.tsx` 的 T 模式说明（桌面和 iOS 两份）和 `/` 模式说明补上新关键词。
- 不做拼音组字里的行内日期候选（打 `riqi` 直接在候选里出日期）：那一层要新增开关、改候选混排和学习排除，需要产品拍板，本次不碰。

## Alternatives considered

- **只给一行农历日期，不加带星期的写法**：改动最小，`nl` 和 `rq` 末行完全相同。但 issue 要的是「日期 + 星期」这种写日记时的常用格式，`rq` 列表里公历已经有带星期的写法，农历没有；多两行成本很低，所以加上。
- **在拼音组字中匹配 `nongli` / `riqi` 并插入行内候选**：最接近搜狗的体验，也是 issue 的原始期望。代价是要动 `session/candidates.rs` 的混排、新增 `LocalModeOptions` 字段和共享偏好键，并保证生成行不进学习和固定位置；这些都需要先确定默认开关和位次，留给第二层。

## Consequences

- **收益**：农历一键可达；T 模式、`/` 模式和服务端三处关键词保持一致。
- **代价与已知上限**：`/` 模式不带字母时的内置命令从三条变成四条（加农历首行），`/y`、`/n` 等前缀会多出农历这一行；这是有意的前缀匹配行为。农历的日用 `chinese_number`（`二十七日` 而不是 `廿七`），沿用 `rq` 列表末行的既有写法。

## Verification

- `cargo test -p msime-engine`：`local::date_time::tests::lunar_aliases_with_weekday_variants`（三个关键词、闰月、限行、换算不了的日期）、`local::command::tests`（裸 `/`、`nongli`、前缀 `yin`、标题）、`backend::tests::date_time_validates_the_supplied_date`（服务端白名单）、`session::tests::a_date_row_commits_and_leaves_date_time_mode`（会话里 Shift+T 打 `nongli` 选带星期的那行上屏并退出模式）。
- `apps/desktop` 的 `vitest run tests/settings/settings.test.tsx tests/settings/plugins-section.test.tsx tests/settings/local-modes-section.test.tsx`。
