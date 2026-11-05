# Agent Note: 五笔剩余编码开关同时管引擎注释里的五笔反查码

Status: implemented

## Problem

#10790：五笔方案关掉「候选显示剩余编码」（偏好 `wubi_code_hint`）后，候选后面仍有编码。开关原本只被各端宿主读取，用来算「编码减去已打前缀」的剩余编码；引擎宿主层 `crates/engine/src/host/session.rs` 的 `snapshot()` 却不看这个开关，五笔方案里一律把词条的完整五笔反查码追加进 `candidate_annotations`（#3392 引入，五笔混输拼音的候选同样附上）。宿主在剩余编码为空时退回显示引擎注释：macOS `CandidateAnnotationRun`（`platforms/macos/src/input/InputController.mm`）在 hint 为空时用 `candidate[@"annotation"]`，Windows `with_wubi_code_hints` 关闭时原样保留注释，Android `candidateAnnotation`、iOS `codeHint`、鸿蒙同样是剩余编码优先、引擎注释兜底。于是开关一关，显示的从「剩余编码」变成了「完整编码」，看起来像开关失效。Linux IBus 与 Fcitx5 宿主早已在五笔方案下随 `wubi_code_hint` 整体隐藏注释，因此 Linux 不受影响，也说明「关掉就不显示五笔码」本来就是一端已有的语义。

## Decision

`EngineOptions` 新增 `show_wubi_code`，host-api 在两处构造（新建会话与偏好变化后重建）都按 `Preferences::wubi_code_hint` 填写，缺省为 true，与偏好默认一致。引擎宿主层只在 `show_wubi_code` 为 true 时追加五笔反查码；为 false 时五笔码表候选与混输拼音候选都不再带五笔码，辅助码、纠错等其它注释不受影响。`candidate_codes` 照旧下发，查词、排序、学习都不读注释，行为不变。偏好任何变化都会让 `session_preferences_differ` 判为待应用，空闲时按新偏好重建 Engine，开关切换与辅助码显示开关走同一条生效路径。

## Alternatives considered

- 每个宿主在关闭时自行丢弃五笔方案下的引擎注释（Linux 现在的做法）：不改契约，但要在 macOS、Windows、Android、iOS、鸿蒙五处各加一份规则，而且会连辅助码以外的注释一起丢；在引擎处判断只需一处，五端同时生效，所以选引擎。
- 只在码表候选上去掉反查码、混输拼音候选保留：混输用户可借注释学五笔码，这是 #3392 的初衷；但开关文案是「候选显示剩余编码」，关掉后仍有一类候选带码同样会被报告为开关失效，且两类候选在同一页混排时规则难以解释。采用整体关闭，是否为混输候选单独保留由维护者决定（见 PR 的「需要维护者确认」）。

## Verification

`cargo test -p msime-engine --lib wubi_reverse_codes`：新增 `wubi_reverse_codes_follow_the_wubi_code_hint_switch`，修复前在 `mixed=false shown=false` 处失败（注释仍为 `wqvb`），修复后与既有三个反查用例一起通过。`cargo test -p msime-host-api --lib wubi_code_hint_preference_reaches_engine_options` 验证偏好切换后会话的 `show_wubi_code` 跟着变化。未在真机或 macOS 输入法里截图验证。

## Consequences

关掉剩余编码后，五笔候选不再显示任何五笔编码，包括完整编码等于已打前缀时原本由反查码显示的那一份，以及混输拼音候选的五笔反查码。以后再给五笔方案的注释位增加内容时，要先决定它是否归这个开关管。

## Note audit

检索 `wubi_code_hint`、`五笔反查`、`剩余编码`：[五笔反查编码缓冲复用](../testing/2026-10-08-wubi-reverse-code-buffer.md)只管反查的分配，与本篇无关；[五笔偏好类型](../simplification/2026-10-15-wubi-preferences.md)持有三个开关的缺省规则，本篇沿用剩余编码缺省为开，不改它，互不吸收。
