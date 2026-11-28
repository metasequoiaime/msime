# Agent Note: 默认上屏也记一次使用，抬到更短编码的那一组之上

Status: implemented

## Problem

#11955：小鹤双拼下用 `ciku` 上屏「词库」后，下一次打 `cik` 时首选仍是「此刻」，而不是刚用过的「词库」。

根因有两层：

- `session/learning.rs` 的 `learn_ranked_candidate` 在 `index == 0`（以及混输末尾跟着英文/日文行的那条拼音首选）直接返回 `None`，默认上屏什么也不记；`user_dictionary/ranking.rs` 的 `adjust_candidate_ranking` 在 `rank == 0` 时也没有可挪的座位。默认上屏的一行本来就已经在自己完整编码里排第一，调频那套「换座位」的算术对它无事可做。
- 用户接着打的 `cik` 不是完整编码。它落在两张表的重叠处：末段只有声母 `k` 时，范围查询按 `key >= 'ci'k' AND key < 'ci'k{'` 取整组（`ci'ke`、`ci'ku`、`ci'kuan`……）并按词重降序，双拼还会把末键读成辅助码、先按整组词重的顺序命中一遍。这条组里的顺序由词重决定，而「词库」在 `ci'ku` 上的词重（10 万级）比「此刻」在 `ci'ke` 上的低，所以永远排在第三。

## Decision

默认上屏在拼音、双拼下也记一次使用，记录方式是**把这一行抬到「末音节换成它的首字母」那一组的最高词重之上**（`ci'ku` → `ci'k`）：

- 入口在 `learn_ranked_candidate`：`index == 0` 与 `default_behind_leaders` 不再直接 `None`，改走新的 `promote_default_candidate`；非默认上屏仍走原来的 `adjust_candidate_frequency`。
- 主体是 `user_dictionary/ranking.rs` 的 `promote_default_above_prefix_group`：用 `build_key_like_pattern` + `key_prefix_upper_bound` 构造同一张拼音表里的范围（`pinyin_table(entry_key)` 与组切分同表），`SELECT MAX(weight)` 取组内最高，把这一行抬到 `max + REBALANCE_GAP`（1000）。
- 先用只读连接判断要不要写：这一行已经是组内最高（`current >= ceiling`）时直接返回，不写日志、不开写事务。空格上屏远多于真正的调频，不能让每次默认上屏都追加一条记录。
- 写库仍走 `update_ranked_weight`（先日志后主库，`ranking_timestamp` 记时间），所以个人词库日志、重放和学习同步照旧看得见这次使用；写入成功后 `log` 侧 `engine.reset_cache()` 让候选重新查询。
- 只认 `PersonalDictionaryKind::Pinyin`。五笔（含混输里的五笔行、临时拼音反查）不参与：五笔的自学习另有规则（#11904），`is_wubi()`、`local_mode != None`、`dedicated_english` 都提前返回。
- 单音节不参与：单音节的「那一组」是首字母表，一行首字母覆盖成百上千个单字，抬到它之上等于让每次单字母查询的首选都跟着最近一次上屏变，不是这次要改的行为（`segment_count < 2` 直接返回）。
- 默认的「一次置前」（`frequency_adjustment.mode = promote`、`trigger_count = 1`）下即生效；`Disabled` 仍是完全关闭，不写任何东西。

## Alternatives considered

- **新增一张「最近上屏」记录表，在候选排序里按它置顶。** 最强理由是它不动词重、语义最干净，也不会有词重膨胀。但这要新建日志表、改重放、改学习同步的合并规则、并在六个平台的宿主可见排序里各加一次判断；而候选顺序今天已经由词重驱动，记词重是既有机制的唯一自然落点。
- **抬到整个父前缀组之上（`ci'` 下的全部词）。** 最强理由是 `cik` 在双拼里既可能被读成 `ci'k` 的范围，也可能被读成「`ci` + 辅助码 `k`」，后者是在整条 `ci` 前缀里按词重命中。但那样每选一个 `ci…` 的词，下次打 `ci` 的首选都跟着变，改动面远超这个 issue 说的「少最后一个韵母」。
- **给默认上屏也计 `candidate_selection_state` 的命中次数，让「一次置前」在第二次上屏时再动。** 最强理由是它完全复用现有调频语义。但报告者正是要求第一次上屏之后就生效，且范围查询按词重而非命中次数排序，计数不改词重就没有效果。
- **每次默认上屏都无条件抬 1000（不比较组内最高）。** 最强理由是不用先读一次库。但连续空格上屏同一个词会把词重推到上限，也会让已排第一的词每上屏一次就写一条日志。

## Consequences

- 六个平台的候选顺序都会出现可见变化：在拼音/双拼下用空格上屏一个两音节以上的词之后，下一次用「末音节只打首字母」的短编码，这个词会是首选。
- 词重会升高到「组内最高 + 1000」，仍在 `clamp_managed_weight` 的 1..=1 亿之内；已经是组内最高的词不再写入，词重不会无限增长。
- `Disabled` 的用户、五笔用户、单音节上屏、临时英文和超级简拼路径的行为不变；候选身份、固定位置和删除的规则不受影响（这次只写词重）。
- 没有新增设置项，也没有改任何默认值。

## Testing

- `crates/engine/src/session/learning.rs` 新增 `a_default_pick_is_promoted_above_its_shorter_code`：小鹤双拼 `cik` 先给「此刻」首位，`ciku` 上屏「词库」后日志里的 `ci'ku` 词重从 10 万抬到 501000，再打 `cik` 首位是「词库」；再上屏一次不再改写；新会话照旧生效。夹具里的 `ci` 单字行让双拼的辅助码路径真的跑起来（与报告者的词库一致）。
- 新增 `disabled_frequency_does_not_promote_default_picks`（`Disabled` 下不写日志）和 `a_single_syllable_pick_is_not_promoted`（单音节上屏不写）。
- 既有回归照常：`the_trigger_count_and_the_leader`（单音节索引 0 仍不写）、`shuangpin_and_wubi_persist_their_picks`（五笔按编码学习不变）。
- `rbuild -- bash -lc 'ulimit -n 4096; cargo test -p msime-engine --lib'`：3307 passed，0 failed（合并前基线同一命令跑出的 209 个失败是 rbuild 的 SSH 会话文件句柄软上限 256 造成的，`scripts/verify-local.sh` 里为此抬到 4096）。
