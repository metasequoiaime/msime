# Agent Note: Windows 选词按文字在宿主会话里定位，不再套用候选窗的下标

Status: implemented

## Problem

打 `bushang`，候选窗高亮第 1 项「补上」，按空格上屏的却是「不伤」——那是候选窗里的第 4 项。数字键 1 同样上屏「不伤」。用 SendInput 往记事本里复现，稳定重现 5 次；`ceshi` 则正常上屏「测试」。界面上完全看不出异常：高亮一直停在「补上」，等 2.5 秒列表也不变，云候选占位符始终没返回。

原因是候选身份在两个引擎会话之间被当成了同一个下标。候选窗由 Server 进程绘制（`src/candidate/CandidateWindow.cpp`），而 TIP 进程里另有一个自己的宿主会话（`EngineSessionAdapter`，`msime_client_create`）。[空格和点选也在 TIP 的宿主会话里选词](2026-10-16-windows-space-selects-in-host-session.md)让空格、数字、点选都经 `_SelectHighlightedHostCandidate` 在宿主会话里选中高亮候选，它取候选窗高亮项的 `_EngineIndex`，直接交给 `host->select`。两边唯一的校验是 `MatchesEngineView`，而它只比对 session 和 generation，**不比对列表内容**。

那篇笔记自己记下了这个风险：代价一节写着「两个会话的状态一致时两者相同」，重访信号写着「如果宿主会话和 Server 选出的候选出现分歧，就要重新评估由哪边决定上屏文字」。这次就是那个信号。两个会话都带着 `learning = true` 各自演化，同一个下标于是指向不同的词；而每错一次又把错的那个喂给宿主会话的学习，下次它更靠前。

## Decision

`_SelectHighlightedHostCandidate` 按**文字**在宿主会话当前列表里唯一匹配，用匹配到的候选自己的下标调 `host->select`，不再使用候选窗带来的 `_EngineIndex`。同形候选在同一页上不止一个时 **never** 猜，和找不到一样返回 `S_FALSE`，由调用方既有的兜底接手（写 Server 的文字，再 `_CancelHostComposition`）。`MatchesEngineView` 的 session/generation 前置校验保留。

这不是新规则，是这份代码库已有的规则。`common/HostCandidateContinuation.h` 开头就写着「候选必须在本地当前列表中唯一匹配，不能套用 Server 的身份」，续接候选一直这么做；`_SelectHighlightedHostCandidate` 是漏网的那一处。

`tests/input/space_host_selection_wiring.cpp` 相应改为核对新接线：select 用的是匹配到的下标而不是 `displayed._EngineIndex`，文字比对在 select 之前，匹配不唯一时提前返回。

## Alternatives considered

- **上屏改回用 Server 的回复，宿主会话只跟随** — 显示与上屏天然同源，不可能分叉。否决：[2026-10-16](2026-10-16-windows-space-selects-in-host-session.md) 正是为了离开这条路——宿主会话拿不到选择就不前进，空格上屏过的拼音在里面越积越长，Shift 切英文时整串吐出来（「你好世界nihaoshijie」），分段选词的剩余拼音也会丢。那篇笔记否决过的反方向在这里同样成立。
- **让 TIP 的宿主会话不学习、降级成纯组字镜像** — 砍掉一个分叉来源。否决：两个会话仍然各自独立，配置、在线候选的应用时机、分段选择的历史都还能让顺序分开；它降低了概率而没有消除下标跨会话这个前提。按文字定位则与两边是否同步无关。
- **只加一次内容对账，发现不一致就退回 Server 的文字** — 改动更小。否决：那是检测分叉而不是消除它，对账一旦遗漏就复发；而且每次分叉都要丢掉宿主会话的组字，分段选词的剩余拼音又会丢。按文字定位在分叉时**仍然能选中用户看见的那个词**，组字照常前进。
- **同步两个会话的配置**（`config.toml` 与 `runtime-options.json` 的 `cloud_candidates` 确实相反） — 调查中发现的另一处不一致。否决：两个宿主读的是同一份 `runtime-options.json`，那处矛盾不是本缺陷的成因；它值得单独处理，但修掉它也不会让下标重新可靠。

## Consequences

- **收益**：用户看见哪个候选，上屏就是哪个，空格、数字键、点选三条路同时修好；宿主会话仍随之前进，2026-10-16 那次改动的收益不受影响。
- **代价**：同一页上出现两个同形候选时这条路会拒绝，落回兜底，那种情形下分段选词剩余的拼音不保留——与候选窗和宿主会话对不上时的既有行为相同。每次选择多一次列表线性扫描，页内候选个位数，可忽略。
- **重访信号**：如果以后候选需要按身份（而不是文字）区分同形项——例如同一个词来自不同来源要分别计数——这条唯一匹配会开始拒绝合法选择，那时需要的是让两个会话共享候选身份，而不是放宽匹配。
- **未处理**：`runtime-options.json` 里 `preferences.cloud_candidates = false` 与 `config.toml` 的 `true` 相反。文档里的 preferences 是准备那一刻的副本（`lib.rs` 的 `HostOptions::from_document`），`refresh_host_options` 只刷新词库代次。本次不动它。

## Verification

- `msime-tsf-space-host-selection-wiring` 与 `msime-tsf-gloss-column-wiring` 在改动后的源码上通过（Visual Studio 2022 生成工具 17.14，RelWithDebInfo）。这两项是源码接线检查，证明的是接线而不是运行时行为。
- 真机：装上本次构建后，在记事本里打 `bushang` 按空格，以及按数字键 1，都要上屏候选窗高亮的那一项。修复前这两条路都上屏「不伤」。
