# Agent Note: 快捷短语混输与调频两个共享偏好

Status: implemented

## Problem

Windows 有两个快捷短语偏好，新架构没有对应物：

- `utility.quick_phrase_candidates`（出厂关）：普通组字的编码恰好是某条快捷短语的编码时，把整组短语放进普通候选。新架构里引擎早就有这个能力，但只有 macOS、Linux 两个宿主在选项里写死 `true`，用户关不掉（A3）。
- `utility.quick_phrase_frequency`（出厂开）：常越过整组快捷短语选别的候选时，整组后退，最多退到首页末位；选中组内短语时整组前移。新架构完全没有这套调频（A4）。

## Decision

两个偏好落在共享偏好 `mixed_input` 下，和符号混输、日文混输同一组：`mixed_input.quick_phrase` 管混输，`mixed_input.quick_phrase_frequency` 管调频。

**默认值：`quick_phrase` 新装关（对齐 Windows 出厂），缺键的旧文档读作开。** serde 用 `enabled_by_default` + `skip_serializing_if = "is_true"`：升级用户的文档没有这个键，读作开，保住 macOS/Linux 升级前的行为；`Preferences::default()` 给 false，新装和重置后默认关，与 Windows 出厂一致。这个「旧文档读作开、新装默认关」的双默认和 `cloud_candidates`、`candidate_translations` 是同一个模式。这是行为变更：macOS/Linux 的新装用户不会再自动看到快捷短语混进普通候选，要在设置里打开；升级用户不受影响。

**宿主能力与用户开关相与。** `HostOptions::quick_phrase_candidates`（macOS、Linux 声明）保留为「这个宿主提供这个功能」，`EngineOptions.quick_phrase_candidates = 宿主请求 && mixed_input.quick_phrase`。其他宿主（Windows、iOS、Android、HarmonyOS）不声明能力，开关写了也不生效，和 2026-10-17 笔记里「其他平台保持现状」的决定一致。macOS 的 `InputController.mm` 里的 `@YES` 不再直接等于「开启」，而是「声明能力」。

**A4 移植 Windows 的槽位机制，落位沿用新架构的「追加在末尾」。** journal 新增 `quick_phrase_slots(code, slot)` 表，slot 是「组前面有几个计位候选」；没有记录时组追加在末尾（新架构的默认落位，Windows 是固定在首位），学过之后组按槽位摆放。学习在 `learn_quick_phrase_order`：选中组内短语满 `trigger_count` 次，整组按调频规则前移（`ranking_target` 同一套公式）；越过整组选排在后面的普通候选满 `trigger_count` 次，槽位 +1，封顶 `first_page_rows - 1`（首页末位）。选中组内非首条时那条短语在同码里升到最高权重 + 1，走 `quick_parases` 表和日志重放，和 Windows 一致。槽位计数的候选口径照搬 `counts_toward_quick_phrase_slot`：固定位置的行、快捷短语自己和混输/在线/插件/联系人行都不算。没有记录时的落位后来改成首页尾部，见 [快捷短语混输默认落在首页末位](2026-10-25-quick-phrase-first-page-default.md)；槽位和学习规则不变。

## Alternatives considered

- **对齐 Windows 把组固定在首位**：和 2026-10-17 的笔记冲突，那里明确选了「追加在末尾、不动普通候选排序」。本改动不改落位决定，只加调频。
- **`quick_phrase` 默认开**：一行注释就能改，但新装用户多一个他没要过的候选来源；Windows 出厂关是有意为之（短语插在首位，打扰大）。新架构追加在末尾打扰小，但出厂默认仍对齐 Windows。
- **调频不读槽位、只在内存里挪**：关掉 App 就忘，换设备不同步，且 `quick_phrase_slots` 本来就是 journal 表，加表成本一行 DDL。
- **字段放 `local_modes`**：`quick_phrase`（K 模式）在那里，但混输和调频都不是本地模式，和 `mixed_input` 的「混进普通候选」语义同一族。

## Intended behavior

- macOS、Linux 上设置页「候选与联想 → 更多选项」多出「快捷短语混输」和「快捷短语参与调频」两行；调频行在混输关着时置灰。其他宿主的设置页不显示这两行。
- 混输开着时，编码恰好等于某条快捷短语的编码，整组短语混进普通候选（默认落位见 [快捷短语混输默认落在首页末位](2026-10-25-quick-phrase-first-page-default.md)）；常越过它选别的，它一格一格后退，最多退到首页末位；选中组内短语，整组前移。
- 混输关着时普通候选不出现快捷短语，`Shift+K` 的 K 模式不受影响。
- 升级用户的文档没有这两个键，读作开，行为不变；新装默认关混输、开调频。

## Risks

- macOS/Linux 新装用户失去「快捷短语自动进候选」，这是行为变更，PR 正文要写明。
- `quick_phrase_slots` 是新增的 journal 表，`CREATE TABLE IF NOT EXISTS` 随每次打开执行，老日志文件自动获得这张表，不需要 schema 版本迁移。
- 学下来的槽位只记「组前几个计位候选」，候选数量变化时组可能落不到原意的位置；和 Windows 的槽位语义相同，是有意接受的近似。

## Verification

- `cargo test -p msime-engine quick_phrase`：16 项全过，含新增的 `quick_phrase_frequency_moves_the_group_on_selection`（选中前移、越过逐格后退）和 `the_group_stays_at_the_end_when_quick_phrase_frequency_is_off`（关闭时位置不变）。
- `cargo test -p msime-host-api quick_phrase_candidates`：新增 `the_quick_phrase_candidates_preference_gates_the_host_capability`（偏好关 → 无短语，偏好开 → 有短语）。
- 先失败后通过：A4 关掉 `learn_quick_phrase_order` 调用时 `quick_phrase_frequency_moves_the_group_on_selection` 失败（期望 4，实际 7）；A3 去掉 `&& mixed_input.quick_phrase` 门控时 `the_quick_phrase_candidates_preference_gates_the_host_capability` 失败（关偏好仍出现短语）。
- `cargo fmt --all --check`、`cargo test -p msime-engine -p msime-client-core`、`cargo check -p msime-host-api -p msime-input-runtime -p msime-mcp-server --all-targets`、`pnpm --dir apps/desktop exec tsc --noEmit`、相关 vitest、`scripts/run-checks.sh`、`preference_coverage.py`、`pnpm run verify-notes` 全过。

## Consequences

- 「快捷短语混输」从宿主能力变成用户开关：macOS、Linux 上用户可以在设置里关掉它，新装默认关。
- 「快捷短语参与调频」默认开；它和调频开关共用 `frequency.mode != Disabled` 的门禁，关掉调频时槽位记录保留但不读。
- `quick_phrase_slots` 表进 journal，换词库时随日志重放。

快捷短语的 `${cursor}` 展开与上屏后缀契约见[光标定位笔记](../../implemented/feature/2026-10-31-quick-phrase-cursor.md)；不改变本文原有决定。
