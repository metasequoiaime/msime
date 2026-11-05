# Agent Note: 关掉「候选显示剩余编码」后引擎不再附五笔完整码

Status: implemented

## Problem

#10790：macOS 五笔方案下关掉「候选显示剩余编码」（共享偏好 `wubi_code_hint`），候选旁仍然显示五笔编码，只是从剩余编码 `(vb)` 换成了完整编码 `(wqvb)`。

原因在两层之间。`crates/engine/src/host/session.rs` 的快照在五笔方案里无条件给普通候选的 `annotation` 附上完整五笔码（反查码，混输拼音的候选也附），不看这个偏好。macOS、Windows、Android、iOS、鸿蒙宿主都只在剩余编码非空时用它替换注释，开关关掉后剩余编码为空，就退回显示引擎注释，于是完整码露出来。只有 Linux 两个宿主在开关关掉时整段隐藏五笔方案的注释（`/`、`@` 模式除外，见 [插件门控与宿主侧五笔表模式](2026-10-09-plugin-gates-and-wubi-table-modes.md)），所以 Linux 上没有这个问题。

## Decision

- `EngineOptions` 新增 `wubi_reverse_code`，`prepare_options` 给 true，出厂行为不变。host-api 建会话（`into_engine_options`）和改偏好后重建 Engine（`apply_pending`）两条路都把它填成 `preferences.wubi_code_hint`。改偏好本来就会在空闲时重建 Engine，不需要新的运行期开关。
- 快照只在 `wubi_reverse_code` 为 true 时给五笔方案的普通候选附完整码。候选的 `code` 字段不受影响，开关打开时宿主照旧用它算剩余编码。纠错提示、辅助码这些注释里的其他内容不受影响；`/`、`@` 等本地模式本来就不附反查码，也不受影响。
- `z` 引导的临时拼音反查例外，开关关掉也照旧附完整码：用户按 `z` 就是为了看这个字的五笔码，这个模式只为它存在。判断用核心会话的 `wubi_pinyin_lookup_active`，不用快照里的 `answered_by_pinyin_fallback`，后者在五笔码打不出、整页都是混输拼音候选时也为真，那种情况属于普通候选。
- 五个宿主都不改：它们退回显示的注释里已经没有完整码。

## Alternatives considered

- **在每个宿主里关掉开关时隐藏注释，照 Linux 的做法。** 不需要碰引擎；但要改 macOS、Windows、Android、iOS、鸿蒙五份宿主代码，每份还要各自分辨注释里哪部分是五笔码、哪部分是纠错提示，整段隐藏会把纠错提示一起藏掉。引擎是附码的那一层，在那里不附最直接，五个宿主一次覆盖。
- **开关关掉时连 `z` 反查的编码也不附，与 Linux 宿主完全一致。** 规则最简单；但关掉剩余编码的用户按 `z` 反查时会看不到任何编码，这个模式就没有用处了。
- **用 `answered_by_pinyin_fallback` 识别反查。** 快照里现成就有，不用加访问器；但它分不出 `z` 反查和普通的混输拼音回退，见上。
- **只对五笔原生候选（`candidate.scheme` 为五笔）不附，混输拼音候选照旧附码。** [五笔反查](../feature/2026-10-11-wubi-reverse-lookup.md)把混输里直接打拼音看编码当作和 `z` 反查并列的查码路径，而混输打开时 `z` 是拼音字母、没有 `z` 反查，按本条规则关掉开关的混输用户在打字时就没有任何地方能看到五笔码，只剩设置页的「五笔查码」。没有采用的理由是开关关掉后拼音候选旁仍会出现编码，用户同样会觉得「关了还显示」，而 Linux 宿主在开关关掉时也不显示它们；这是取舍而不是事实判断，留给维护者决定。

## Consequences

关掉开关后，macOS、Windows、Android、iOS、鸿蒙上的五笔候选旁不再出现编码，与 Linux 一致；打开开关时行为不变。

代价：Linux 宿主仍按自己的规则在开关关掉时整段隐藏五笔方案的注释，所以 `z` 反查在 Linux 上关掉开关后看不到编码，其他平台看得到。这是 Linux 宿主早已有的行为，这次没有改。另一个代价是开关打开、已经打完整码的候选：剩余编码为空，宿主照旧退回显示完整码，和设置页「已经打完整码的候选不标」的说明不一致，这次也没有改。

代价：[五笔反查](../feature/2026-10-11-wubi-reverse-lookup.md)里「混输打开的用户直接打拼音就能看到编码」现在只在开关打开时成立。混输打开时 `z` 是拼音字母、`z` 反查不可用，所以混输用户关掉开关后，在 macOS、Windows、Android、iOS、鸿蒙上也和 Linux 一样，打字时看不到任何五笔码。

## Testing

- `crates/engine/src/host/tests.rs`：`wubi_reverse_codes_are_left_out_when_the_code_hint_is_off` 钉住选项关掉后五笔候选和混输拼音候选的注释都不带完整码、`candidate_codes` 仍是完整码（去掉门控时它失败）；`the_pinyin_lookup_keeps_wubi_codes_when_the_code_hint_is_off` 钉住 `z` 反查照旧附码。
- `crates/host-api/src/tests.rs`：`wubi_code_hint_reaches_engine_options` 钉住建会话和改偏好两条路都把偏好映射到 `wubi_reverse_code`，切回打开后恢复。
- 没有在真机或宿主里看候选窗；宿主代码没有改。
