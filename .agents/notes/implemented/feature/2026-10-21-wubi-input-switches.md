# Agent Note: 补齐五笔 z 通配与第五码顶字开关，四码自动上屏判定抽成纯函数

Status: implemented

## Problem

来源 v0.11.1 的五笔在 `[input]` 段有三个策略键，新架构对不上：

| 来源键 | 出厂值 | 来源语义 | 新架构此前 |
| --- | --- | --- | --- |
| `wubi_z_mode`（`"off"` / `"wildcard"`） | `"off"` | 通配打开时 `z` 匹配码表该位置的任意一个字母（含首位） | **没有**：`z` 只用来打开临时拼音反查 |
| `wubi_four_code_auto_commit` | `true` | 四码唯一时第四键直接上屏 | **已有**：`wubi_auto_commit_unique`，判定写在 `Runtime::wubi_should_auto_commit` 里 |
| `wubi_fifth_code_top_commit` | `true` | 完整四码后再敲字母，上屏首选并把该字母留作下一组开头 | **写死常开**：`Runtime::dispatch` 里没有任何开关 |

来源依据（`/tmp/msime-audit/msime-windows` @ `dd018f79`）：

- 出厂值与注释：`installer/default_config/config.default.toml:315-327`。
- 读配置与非法值回落：`server/src/config/ime_config.cpp:496-499`（`z_mode` 只认 `wildcard`，其余一律 `off`）。
- 设置页文案：`server/src/settings/native/native_page_input.cpp:1644-1652`（「z 键通配」「四码唯一时自动上屏」「第五码将首选上屏」）。
- 纯判定：`server/src/ipc/input_key_policy.h:110-131`。顶字那条是 `(top_commit_enabled || !mixed_pinyin_enabled) && four_code_is_complete && key_is_letter && caret_at_end && !creating_word_active`——**关掉顶字只在混输开启时生效**。
- 方案层：`engine/schemes/wubi_scheme.cpp:7-27,89-105,145-148`；provider 的通配语句 `engine/providers/wubi_candidate_provider.cpp:43-62,180-186`。
- 四码唯一与四码完整的引擎事实：`engine/core/input_session_composition.cpp:746-784`，两者都用 `raw_input.find('z')` 挡住带 `z` 的猜码。

三键里只有 z 通配和五码顶字是真缺口；四码自动上屏新架构已有，缺的是「判定抽成纯函数」这一层可测性。

## Decision

### z 通配

- 共享偏好新增 `wubi_z_wildcard: bool`，`#[serde(default)]`，缺省关，对齐来源 `wubi_z_mode = "off"`。新架构用布尔而不是来源的三态字符串：来源只剩 `off` / `wildcard` 两态，第三态在 `ime_config.cpp` 里本来就被归一到 `off`。
- `WubiScheme` 收 `z_wildcard`；`QueryRequest` 新增 `wubi_z_wildcard`，由方案按「通配打开且码里确实有 `z`」算出，provider 只在它为真时改走 GLOB。
- 通配语句：`z` → `?`，GLOB 元字符（`*` `?` `[` `]`）按字面转义，末尾接 `*` 收入更长编码的前缀行；排序改按权重（模式里的 `?` 永远不会等于真实 key 的那个字节，没有精确行可优先）。
- **与临时拼音反查互斥**：三条路都要占用 `z`——混输里它是拼音字母、通配里它是未知码位、两者都关时它打开反查。`ImeSession::pinyin_lookup_allowed()` 一处判定，`bind_wubi_scheme` 和 `raw_request_through_scheme` 都读它。
- 带 `z` 的码不算四码唯一或可顶字的完整四码：共享 runtime 的 `wubi_four_code_is_complete` 与引擎采用相同守卫，猜码后按第五键也不提交首选。`InputSession::wubi_unique_four_code` 加 `raw_input.contains('z')` 守卫，与来源逐字一致。猜码后面那个「唯一」候选与好几个码只差一个字母，直接上屏就夺走了这个键存在的理由。

### 五码顶字

- 共享偏好新增 `wubi_fifth_code_top_commit: bool`，`#[serde(default = "enabled_by_default")]`，缺省开：对齐来源出厂，也是接入开关之前唯一可能的行为。
- 判定抽成纯函数 `wubi_top_commit_on_next_key(&WubiTopCommitRequest)`，五项输入显式列出；`Runtime::dispatch` 只负责把 `self.wubi_fifth_code_top_commit`、`engine.wubi_mixed_pinyin()`、四码完整性、无暂存词段、读音是否续输填进去。
- **关掉只在混输开启时生效**：`(top_commit_enabled || !mixed_pinyin)`。混输关掉时引擎把码裁回四位，字母无处容纳；宿主已经按下的键不能丢，所以仍按顶字上屏。这正是来源 `ShouldCommitCompleteWubiCodeOnNextKey` 的兜底分支。
- 为了让 runtime 知道混输状态，`InputEngine` 加 `wubi_mixed_pinyin()`（默认 `false`），`Session` 从 `WubiInputOptions::mixed_pinyin` 转发。runtime 不复制引擎的读音规则。
- 宿主状态而不是 Engine 状态：改偏好不必等 Engine 空闲重建，下一键生效，与 `wubi_auto_commit_unique` 同一条规矩。
- 顶下来的首选立即上屏，开着 `phrase_preedit` 的宿主（六个宿主都开）也不进暂存词段，与 14 键顶字一致。此前这里记的是「和组字中途的选词一样暂存、下一段选词时一起上屏」，那样会挡住之后每次顶字与四码唯一自动上屏，续打的码被截在四码外丢掉，Esc 还连已顶的字一起丢；改动与理由见[五笔顶字立即上屏，不进暂存词段](../bug-fix/2026-11-01-wubi-top-commit-skips-phrase-hold.md)。安卓设备验收「26 键五笔第五码顶字」与运行时回归 `a_fifth_letter_after_wqiy_commits_the_topped_word_even_with_phrase_preedit` 钉住这条。

### 四码唯一自动上屏

行为不变，只把 `Runtime::wubi_should_auto_commit` 的五项条件搬进纯函数 `wubi_auto_commit_on_fourth_key(&WubiAutoCommitRequest)`：开关、字符键、快照有效期、四码唯一、无暂存词段。抽出来的目的是让判定能脱离窗口和会话单测，与另外两个键的判定形状一致。

### 设置页与同步

- 共享设置页五笔组加「z 键通配」「第五码将首选上屏」两行，文案与来源设置页一致（来源把顶字的「只在混输开启时生效」写进说明，这里照抄）。
- 账号同步的 `input.wubi_fifth_code_top_commit` 加进导出与应用两份键表；`wubi_z_wildcard` 不进——它改变码表查询语义，来源的账号字段表里也没有它。
- MCP 偏好视图与 `update_preferences` 各加两个字段，与同组的 `wubi_auto_commit_unique` 一致。

## Alternatives considered

- **把 `wubi_z_mode` 照抄成三态枚举**：与来源键名逐字一致，迁移最直接。没有采用：来源的第三态在自己的配置读取里就被归一到 `off`，枚举会多出一个永远读不到的值，还要同步到六个宿主的设置界面与账号字段表。布尔的两态就是它实际的两态。
- **z 通配与临时拼音反查同时打开，按 `z` 的位置分流**：首字母走反查、中间走通配，看起来两边都保住。没有采用：来源两条路互斥，混输打开时反查本来就关；再叠一条按位置的规则，用户按同一个键会得到两种结果，而界面只有一个开关能解释它。
- **顶字开关做成「关掉就一概不顶字」**：语义最直白。没有采用：混输关掉时第五个字母无处容纳，引擎会把码裁回四位，宿主已经显示了这个字母——一概不顶字就是丢按键。来源为此专门写了 `(top_commit_enabled || !mixed_pinyin_enabled)` 的兜底。
- **把顶字判定放进 Engine**：偏好流经现成管道，不用动 `InputEngine` trait。没有采用：上不上屏是宿主行为，Engine 只回答「这个码是不是完整四码、这个字母还能不能续拼音」；把提交与否塞回 Engine 会让「Engine 报告事实、宿主决定动作」的分工反过来，而且 Engine 重建要等组字空闲，开关的生效会滞后。
- **只在 macOS 接开关**：改动最小。没有采用：三键都是共享偏好，判定在共享 runtime 与共享 Engine 里，只接一端会让两端行为分叉。

## Consequences

收益：

- 三个键在共享层有唯一落点，六个宿主读同一份文档、走同一条判定。
- 顶字与自动上屏的判定都是纯函数，窗口外的单测能覆盖真值表；z 通配的查询语义由 provider 单测直接钉住 GLOB 行为。
- 通配打开后 `z` 的第三种语义（反查）自动让位，不存在一个键两种行为。

代价：

- 已经习惯「组字开头 `z` 必开反查」的五笔用户在打开通配后会失去反查；两个功能抢同一个键，这是来源的既定取舍，界面说明里写清了通配含首位。
- 顶字关掉只在混输开启时能观察到差别。混输关掉的用户打开这个开关看不到任何变化，说明文字必须解释这一点，否则会被当成无效开关。
- 通配查询在首位是 `z` 时整表扫描。来源同样如此（`?bc*` 无法用索引），代价是首码不确定时一次全表 GLOB + 权重排序。

## Testing

先失败后通过，逐键留证据：

1. **z 通配**：把 `build_request_into` 里的 `request.wubi_z_wildcard` 强制改回 `false`，`cargo test -p msime-engine --lib wubi_wildcard` 两条会话用例失败（`left: []`，候选为空）；恢复后三条全过。
2. **五码顶字**：把纯函数首行改成 `true &&`，`cargo test -p msime-input-runtime --lib turning_off_the_wubi_top_commit` 的 `turning_off_the_wubi_top_commit_lets_a_mixed_spelling_grow` 失败（`grown.commit` 不是 `None`）；恢复后两条全过。
3. **四码自动上屏**：把纯函数首行改成 `true &&`，`the_wubi_auto_commit_decision_needs_all_five_facts` 失败；恢复后通过。

新增回归：

- `msime-engine`：`a_z_in_the_code_matches_any_one_letter_when_the_wildcard_is_on`、`a_wildcard_query_does_not_leak_glob_metacharacters`、`the_wildcard_only_takes_the_pattern_path_when_the_code_holds_z`、`the_wubi_wildcard_matches_any_letter_and_keeps_guessing_codes_open`、`the_wubi_wildcard_takes_z_from_the_pinyin_lookup`、`turning_the_wubi_wildcard_off_keeps_z_out_of_the_code`、`a_wildcard_code_never_counts_as_a_unique_four_code`。
- `msime-client-core`：`wubi_z_wildcard_defaults_off_and_roundtrips`、`wubi_fifth_code_top_commit_defaults_on_and_roundtrips`（缺键仍读出出厂值、显式值往返保持）。
- `msime-input-runtime`：`the_wubi_top_commit_decision_is_the_reference_truth_table`（来源六项真值表逐条）、`the_wubi_top_commit_tops_a_mixed_letter_that_continues_no_reading`（混输与顶字都开时，续不上读音的字母顶字、能续写的保留组字）、`the_wubi_auto_commit_decision_needs_all_five_facts`、`turning_off_the_wubi_top_commit_lets_a_mixed_spelling_grow`、`turning_off_the_wubi_top_commit_still_tops_with_mixed_input_off`。
- `msime-client-services`：账号同步导出的键清单与逐键往返带上 `input.wubi_fifth_code_top_commit`。
- `apps/desktop`：`wubi-preferences.test.ts` 的缺键默认值、`wubi-section.test.tsx` 的两行控件与各自写回的 patch。

本次审查补充真实 Runtime 会话回归：86 / 98 方案与混输开关的四种组合均输入 `wqzy` 后再按 `e`，确认通配候选不被五码顶字提前提交；纯五笔允许拒绝第五码，已有组字仍保留。Runtime 五笔定向 22 项、Engine 通配定向 15 项、两 crate 全目标严格 Clippy、`scripts/verify-local.sh --quick` 通过，其中 Linux 原生 94 项测试通过。共享设置页定向 8 项、desktop typecheck、build 与笔记校验通过。

原有命令与结果见本次提交（`cargo test -p msime-engine`、`cargo fmt --all --check`、`cargo clippy -p msime-engine --all-targets --locked -- -D warnings`、`cargo check -p msime-host-api -p msime-input-runtime --all-targets`、`pnpm --filter @msime/desktop test`、`pnpm format:check`、`pnpm run verify-notes`）。

设置界面范围：共享 React 页由 macOS、Linux 与 HarmonyOS 承载；Windows、Android、iOS 的产品设置各自原生绘制，本 PR 没有给它们增加这两个开关。Windows 原生设置和 macOS 原生外观面板不会因共享页改动自动出现新控件。真机设置窗口未验收。

## Related notes

- [四码唯一自动上屏接进共享偏好](2026-10-08-wubi-auto-commit-unique-preference.md)：四码那条的落点与「宿主状态而非 Engine 状态」的规矩，本篇的顶字开关照它办。
- [五笔反查：`z` 引导临时拼音与设置页查码](2026-10-11-wubi-reverse-lookup.md)：`z` 反查的原设计与「真有用户要求关掉，再按四码唯一那条路接进共享偏好」的预留；本篇按那条路接的是通配，它们争同一个键。
- [五笔混输开关统一叫「五笔拼音混输」](2026-10-09-wubi-mixed-pinyin-naming.md)：顶字关掉只在混输开启时生效，说明文字里的「混输」指的就是那个开关。
