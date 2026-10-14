# Agent Note: 全拼相邻重复键优先按连按误触纠错

Status: implemented

## Problem

全拼纠错把「正确音节中插入一个字母」统一按 `INSERTION_WEIGHT = 12` 计价。用户连按同一个键时，生成的错误串同时可能被切成多个音节，并由较便宜的普通删除纠错（权重 11）抢先。例如 `shannghai` 的直接读法是 `shang'hai`，旧排序却选择 `sha'nang'hai`；`shangghao` 也会被拆成 `shang'gao`。这使一个常见的物理键盘误触改变了音节边界。

## Decision

纠错边在运行时根据原始错误串和目标音节识别「只多出一个、且与相邻正确字母相同」的插入。该类边使用独立的 `REPEATED_INSERTION_WEIGHT = 10`，低于普通删除和普通插入，与转置纠错同级；其它插入、删除、邻键替换和转置仍沿用原有权重。识别只作用于已经启用的插入纠错表，不改变纠错表的生成内容或开关语义。

## Alternatives considered

- **把所有插入纠错的权重都降到 10**：实现最简单，但会让任意漏按字母的纠错都压过删除纠错，扩大行为变化范围，无法表达 issue 只针对连按误触的意图，因此不采用。
- **在切分阶段硬编码 `shannghai` 一类词形**：可以快速通过单个例子，但会复制输入规则并随音节表增长，无法覆盖不同位置和不同字母的重复键，因此不采用。
- **修改纠错表生成器，把重复键行移到独立表**：能在静态表层表达权重类别，但会改变现有表文件契约和生成计数，且重复属性只影响排序，不影响表成员；当前在查询边上判定更小、更容易保持兼容，因此不采用。

## Consequences

收益：`shannghai`、`shangghao` 等相邻重复键会优先保留自然音节边界；普通插入和其它纠错的排序保持不变，现有纠错开关仍分别控制各自表。

代价与缺口：重复键权重与转置并列，最终同成本结果仍受既有表顺序和候选频率影响；本地没有配置经过验证的词库目录，无法运行 `convert_eval` 的整句评测，因此需要在具备 `MSIME_EVAL_RESOURCES` 的环境中补跑并核对基线。

## Verification

- 先在旧实现上运行新增的 `repeated_key_at_a_syllable_boundary_prefers_the_dropped_letter`，确认 `shannghai` 实际得到 `sha'nang'hai` 并失败。
- 加入专用重复插入权重后，`cargo test -p msime-engine pinyin::autocorrect::tests::repeated_` 通过（2 项）。
- 全部引擎单元测试通过：`cargo test -p msime-engine --lib`（2052 通过、16 忽略、0 失败）。
- Golden 测试通过：`cargo test -p msime-engine --test golden`（31 通过、0 失败）。
- `convert_eval` 未运行：当前环境没有经过验证的 `MSIME_EVAL_RESOURCES` 和句子模型。
