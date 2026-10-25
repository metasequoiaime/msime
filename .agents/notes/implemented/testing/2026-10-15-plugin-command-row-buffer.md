# Agent Note: 插件指令回包行缓冲复用

Status: implemented

## Problem

`Runtime::run_command_provider` 每次询问插件指令时都创建新的 `Vec<(String, String)>`，插件提供者把回包行写入后，这个行容器只存活到本次刷新结束。插件字符串本身仍需要按提供者接口写入和转移，但短命的容器容量可以在同一 runtime 的连续查询之间复用。

本决定与[插件指令回包固定文本槽位](2026-10-15-plugin-command-text-buffer.md)及[插件缓存行直接插入候选列表](2026-10-15-plugin-row-cache-splice.md)分别负责过滤阶段、候选重建阶段的容器分配；[插件指令行架构](../architecture/2026-10-13-script-plugin-command-rows.md)继续持有查询失效、缓存和宿主协议边界。

## Decision

`PluginState` 持有一个 `command_rows` 行缓冲。每次调用提供者前清空它，提供者把行追加到这个缓冲；`NotRun` 和引擎拒绝过期回包时清空并保留容量。引擎接受回包后用 `drain(..)` 把文本和注释的所有权移入 `command_notes`，因此行字符串的所有权边界和注释去重规则保持不变，同时保留缓冲的已分配容量。

提供者仍然只能通过 `CommandProvider::provide` 写入拥有型行；本决定不改变 `CommandOutcome` 语义、查询匹配、引擎候选缓存或插件注释的生命周期。

## Alternatives considered

- **继续为每次调用创建局部 `Vec`**：代码最直接，但同一 runtime 的每次插件查询都会重复申请行容器，分配与插件回包频率成正比。
- **让提供者返回拥有型 `Vec`**：可以把容器责任移到提供者，但会改变现有 `CommandProvider` 接口，并迫使所有宿主实现调整，不能作为局部性能切片。
- **保留行字符串在缓冲中并复制到注释表**：能复用容器，但会额外复制插件文本和注释；使用 `drain(..)` 直接转移字符串，避免引入新的字符串分配。

## Verification

新增 `repeated_command_queries_reuse_the_output_rows_buffer`，先完成一次真实 `/uuid` 查询预热 runtime，再连续触发两次相同查询。旧实现两次各为 50 次分配；复用缓冲后两次各为 49 次，当时的测试固定断言预热后的查询预算并确认提供者仍被调用。还原旧实现时该测试按预期失败。[插件候选注释直接写入截断缓冲](2026-10-15-plugin-annotation-buffer.md)随后移除注释拼接和收集的中间分配，该切片完成后的总预算为 46 次；共享候选字符串合入后总数变为 49 次，当前测试改在[容量申请边界](2026-10-24-plugin-row-buffer-allocation-boundary.md)计数，冷调用 1 次、两次热调用均为 0，并用每次丢弃容量的变异验证失败。这里的 50 → 49 保留为行容器切片的历史独立验证。插件指令相关测试还覆盖 `NotRun`、零行、暂停、缓存和注释行为。

## Consequences

连续插件指令查询省去行容器的重复申请，首次查询仍按需要扩容；`command_rows` 会保留峰值容量，因此 runtime 生命周期内会保留最近一次回包的容器存储。成功回包后字符串移入注释表，失败和 `NotRun` 路径只释放元素、不释放容量。公开接口、候选顺序、缓存失效和插件行语义不变。
