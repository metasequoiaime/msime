# Agent Note: 权重为 0 的个人词条可以删除和调权重

Status: implemented

## Problem

iOS 个人词库列出的部分五笔词权重为 0，例如「还不饿」98 五笔 `ddqn`。滑动删除后请求进入队列，键盘应用时失败，显示「dictionary edit rejected」，词条仍然留在列表里。

这些行来自[五笔新词学习](../../implemented/feature/2026-10-18-dictionary-completion.md)。用户确认拼音自造词后，`wubi_learning::learn` 按两版五笔取码，并以同码已有词的最小权重减一作为新词权重，结果限制在 0 到 9。同码已有权重为 1 的词时，派生词就会存成 0，并以 `user_inserted=1` 写进日志，所以列表会原样返回 0。这个 0 是正常的存储值，不是坏数据。

拒绝发生在两层，任意一层都足以让删除失败：

- host-api 的直接编辑路径（iOS 键盘应用自己的队列、桌面设置页、鸿蒙）会对旧行调用 `Entry::normalized_for_engine`，把权重从 0 抬到新词下限 1。Engine 要求旧行的权重与日志完全一致，找不到权重为 1 的行，于是返回 `ENTRY_CHANGED`，宿主再把它折叠成「dictionary edit rejected」。
- Engine 的 `validate_existing_entry` 与 host-api 的 `validate_previous_entry` 只取消了旧行的权重上限，下限仍是 1。走 Rust 队列的宿主（Android 和桌面）在入队时由 `personal_from_entry` 按新词规则拒绝，在应用时由 `dictionary_validate_previous` 拒绝。MCP 的 `edit_dictionary_word` 直接用存储行编辑，也会被拒绝。

## Decision

旧行是列表返回的存储行，校验它时只要求权重在存储可能出现的范围内，不再套用新词的取值范围。

- Engine 的 `validate_existing_entry` 接受 `0..=i64::MAX`。`validate_personal_dictionary_entry` 对新词仍要求 `1..=MAX_ENTRY_WEIGHT`。
- host-api 的直接编辑路径对旧行只调用 `Entry::with_folded_code` 折叠编码，权重原样交给 Engine。`normalized_for_engine` 只用于新写入的替换行，仍会把 0 抬到 1。
- host-api 的 `validate_previous_entry` 接受 `0..=i64::MAX`。队列入队时，旧行改由 `personal_from_stored_entry` 处理，它按 `dictionary_validate_previous` 规范旧行；替换行仍走 `personal_from_entry`。
- 已核对所有会写入 `user_inserted=1` 行的路径：五笔学习限制在 0 以上，英文学习使用 `weight.max(0)`，状态恢复要求 `0..=MAX_STAGED_WEIGHT`，手动编辑和导入至少为 1。存储里不会出现负权重，所以负权重的旧行仍按越界拒绝。client-services 队列的 `PersonalWord::validate` 本来就允许 `weight >= 0`。
- iOS 同步状态页显示失败原因时，会把认得的 host-api 英文原因换成共享设置页 `dictionaryErrorMessage` 的措辞。「dictionary edit rejected」按请求类型显示「删除失败，请稍后重试。」或「保存失败，请稍后重试。」，与桌面这个错误码回落到的提示一致；认不出的原因原样显示。

## Existing notes audit

- [词库持续补全与五笔新词学习](../../implemented/feature/2026-10-18-dictionary-completion.md)：权重为 0 的行就是这篇决定写入的。两篇部分重叠，本篇保留低权重派生规则，只修复编辑路径，并在这里互链。
- 英文学习词超过权重上限后仍可编辑的修复没有单独的笔记，相关理由写在 `validate_existing_entry` 的文档注释和测试里。本篇按同一原则补上下限。
- 其余命中的笔记只是在别的场景提到「权重 0」，与本篇无关。

## Alternatives considered

- 让五笔学习至少写 1。这样以后不会再生成 0，但同码已有权重为 1 的派生词会和原有词并列，违背「不挤占已有同码候选」的设计；设备上已有的 0 权重行仍然删不掉。因此不采用。
- 只改 host-api，让旧行权重原样传给 Engine。这样能修复 iOS 直接编辑路径，但 Engine 的下限仍会在 Android 队列和 MCP 路径上拒绝同一行，所以两层一起改。
- 保留旧行的权重折叠，并在 Engine 里把旧行权重 0 当作 1 匹配。这样能保住「用新建时的写法删除」这个便利，但需要在比对日志时引入例外，还会让真实权重为 0 和 1 的两行无法区分。因此改为让旧行原样到达 Engine，示例程序 `dictionary_requests.rs` 改用存储时的权重删除。

## Consequences

列表返回的任何一行都能删除；替换行权重至少为 1 时，也能调整权重。所有宿主共用这段 Rust 代码，因此 iOS、Android、macOS、Windows、Linux 和鸿蒙同时生效。代价是，宿主如果拿新建时提交的表单删除，而不是拿列表行删除，表单里的权重 0 不再被折叠成 1，会以「行已变化」被拒绝。代码库内没有宿主这样做，只有示例程序依赖过这一点，已同步修改。

调权重时如果把新权重写成 0，仍会被抬到 1 或被拒绝，所以 0 只能由学习路径产生。iOS 的中文提示只覆盖三类已知原因，别的英文原因仍会原样显示。

## Verification

Engine 新增测试 `a_derived_wubi_word_with_weight_zero_stays_editable`：先通过真实的 `wubi_learning::learn` 生成一行权重为 0 的派生词，再按列表行把它调到 5 并删除，同时确认新权重 0 仍被拒绝、负权重的旧行仍被拒绝。修复前，这个测试在调权重一步失败，错误为 `Weight must be between 1 and 100000000`。

host-api 新增测试 `a_listed_wubi_word_at_weight_zero_can_be_removed_directly_and_through_the_queue`：一行按直接编辑删除，另一行经 `queue_edit` 和 `msime_client_personal_dictionary_sync` 删除。换回修复前的源码后，测试得到 `{"error":"dictionary edit rejected","ok":false}`，与设备上看到的现象一致。只修复 Engine、不改 host-api 时，结果相同，说明宿主对旧行权重的折叠本身就足以造成故障。

修复后 `cargo test -p msime-engine` 全部通过（2,893 项），`cargo test -p msime-host-api` 全部通过（1,074 项），`cargo clippy -p msime-engine -p msime-host-api --all-targets -- -D warnings` 通过。iOS 改动只在本机做了 `swiftc -parse`，类型检查交给 PR 上的 iOS CI，真机删除需要在设备上验收。
