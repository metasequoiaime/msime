# Agent Note: 双拼辅助码补齐直接、间接与句中触发方式

Status: implemented

## Problem

Windows 版水杉（`msime-windows` v0.11.1）的双拼辅助码有三种触发方式，并各有可多选的触发键。出厂配置是 `installer/default_config/config.default.toml` 的 `[helpcode]` 段（`shuangpin_helpcode_direct` 起，见下表），设置页的面板与互斥规则在 `server/src/settings/native/native_page_helpcode.cpp` 的 `BuildHelpcodePage`。

macOS 侧只有四类行：开关、方案、导入、候选窗显示（`packages/ui/src/settings/pages/helpcode-page.tsx` 的 `HelpcodeSettingsGroup`）。`crates/client-core/src/preferences.rs` 的 `HelpcodePreferences` 只有 `enabled` / `schema` / `show_in_candidate_window` 三个字段，没有 `direct`、触发键、`mid_sentence` 这些键。因此 macOS 用户无法选择「不敲触发键就不认辅助码」，也没有反引号 / 分号 / 大写触发键，更没有句中辅助码。

### Windows 的配置组 ↔ macOS 现状

| Windows `[helpcode]` 键 | 默认 | 语义 | macOS 现状 |
| --- | --- | --- | --- |
| `shuangpin_helpcode` | `false` | 双拼辅助码总开关 | 有：`shuangpin_helpcode.enabled` |
| `shuangpin_helpcode_schema` | `"lantian"` | 码表方案 | 有 |
| `show_sp_helpcode_in_candidate_window` | `true` | 候选窗显示辅助码 | 有 |
| `shuangpin_helpcode_direct` | `true` | **末尾直接**：完整音节后直接接辅码，多一键是单码（`ulpbx`），末尾两键带大写是双码（`ulpbxY`）。关闭后改由下面的触发键引导 | **无字段，恒按「直接」处理**。`ShuangpinEngine::active_helpcode_query` 先试 `full_helpcode_query`（两码带大写）再试 `single_helpcode_query`（奇数长度的末位字母），都只在完整音节基座上成立 |
| `shuangpin_helpcode_trigger_backtick` | `false` | **末尾间接**触发键：反引号 | 无 |
| `shuangpin_helpcode_trigger_semicolon` | `true` | **末尾间接**触发键：分号 | 无 |
| `shuangpin_helpcode_trigger_uppercase` | `false` | **末尾间接**触发键：大写字母 | 无。大写现在只用作「双码第二码」的判别（`is_full_help_mode`），不是段首引导键 |
| `shuangpin_mid_sentence_helpcode` | `false` | **句中（间接）**：完整音节后敲触发键，后面的字母筛这个音节上的字，之后可以接着打 | 无 |
| `shuangpin_mid_sentence_helpcode_backtick` | `true` | 句中触发键：反引号 | 无 |
| `shuangpin_mid_sentence_helpcode_semicolon` | `false` | 句中触发键：分号（微软 / 搜狗 / 紫光里分号还是 `ing` 韵母，只在完整音节后才当触发键） | 无 |
| `shuangpin_mid_sentence_helpcode_uppercase` | `false` | 句中触发键：完整音节后的大写字母，等价于「反引号 + 这个字母」 | 无 |
| `shuangpin_direct_helpcode` | `false` | **句中（直接）「万象式」**：不要引导键，逐音节解析，三码自动识别、四码单独敲时直接定字，句中要补 `/` | **有等价实现，但没有独立开关**：`ShuangpinEngine::direct_helpcode_paths` / `direct_helpcodes_own_capitals`（[移动端直接辅助码](../../implemented/feature/2026-10-17-mobile-direct-helpcode.md)）。只要 `enabled` 为真就生效，且与末尾直接同时生效，先匹配先返回 |
| `shuangpin_direct_helpcode_slash` | `true` | 万象式四码句中用 `/` 结束 | 无。macOS 的 `/` 目前不进拼音组字 |
| `shuangpin_direct_helpcode_uppercase` | `false` | 万象式四码句中用第二位辅码大写结束 | 部分：`direct_helpcode_paths` 认「小写第一码 + 大写第二码」，但没有「用大写标记四码结束」这一层开关 |

Windows 侧这些键的详细语义见 `msime-windows` v0.11.1 的 `engine/contracts/mid_sentence_helpcode.h`、`engine/core/syllable_helpcode.h`、`engine/contracts/direct_helpcode.h`；末尾间接与句中间接共用同一套扫描规则（触发键在输入串里统一记成反引号 / `kEndMarker`），因为 TSF 的同步吃键、Server 的 raw 串和引擎解析三处必须看到同一种串。

## Decision

三层各落到哪一步如下。

**偏好层（全部落地）。** `HelpcodePreferences` 仍是全拼 / 双拼共用类型；双拼专属的触发方式收进独立的 `ShuangpinHelpcodeModes` 结构，挂在 `HelpcodePreferences.modes` 下（`#[serde(default)]` + `skip_serializing_if = is_default`）。`quanpin_helpcode` 不读这一段。字段与 Windows v0.11.1 `[helpcode]` 组一一对应：`direct`（末尾直接，默认 `true`）、`trigger_backtick` / `trigger_semicolon`（默认 `true`）/ `trigger_uppercase`（末尾间接）、`mid_sentence` 与 `mid_sentence_backtick`（默认 `true`）/ `mid_sentence_semicolon` / `mid_sentence_uppercase`（句中间接）、`direct_helpcode` 与 `direct_helpcode_slash`（默认 `true`）/ `direct_helpcode_uppercase`（万象式直接）。互斥规则在 `ShuangpinHelpcodeModes::normalized()` 里实现，由 `PreferencesStore::save_locked` 在写入端调用：`mid_sentence` 为真时同一枚触发键归句中、清掉末尾；`mid_sentence` 与 `direct_helpcode` 同真时按万象式直接（清 `mid_sentence`）。`host-api` 的 `into_engine_options` 在消费端再归一一次兜底手改的文档。

**引擎层（落地「末尾直接」开关）。** `QueryRequest.shuangpin_helpcode_modes` 携带整组开关；`ImeSession` / `SessionOptions` / `EngineOptions` 三层透传，`Session::new` 与 `InputSession::set_shuangpin_helpcode_modes` 应用。`ShuangpinEngine::active_helpcode_query`、`resolve_shuangpin_composition_base` / `active_shuangpin_helpcode_length`、`apply_shuangpin_helpcode_segmentation` 三处都先检查 `modes.direct`：关闭后「完整基座 + 末尾裸字母」不再被当辅助码（单码 `nihcc`、双码 `nihcAB` 都按纯拼音读），与辅助码整体关闭结果一致；开启时维持既有重排 / 筛选。

**设置页（落地末尾直接辅助码区）。** `helpcode-preferences.ts` 新增 `ShuangpinHelpcodeModes` 类型与 `defaultShuangpinHelpcodeModes`；`helpcode-page.tsx` 的 `modesRows` 只在双拼一族渲染已实现的「末尾直接辅助码」开关，写入 `modes` 段。尚未实现的间接触发键字段保留在契约中但不显示，避免用户打开无效开关。三种布局（当前族、MoreOptions、移动端单列）都接了。

**未做（P1，如实记录）。**

- **末尾间接触发段未实现**：`trigger_backtick` / `trigger_semicolon` / `trigger_uppercase` 已存进偏好并透传到 `QueryRequest`，但引擎不读；反引号 / 分号（非 `ing` 位置）目前仍不进组字串。卡点是要动「输入串契约」：`ShuangpinScheme::handle_key` 接受新键后，`segment_input`、`effective_input_length`、`segment_raw_boundaries`、预编辑与退格都要认识触发段，回归面比单点判定大得多（见 Risks）。
- **句中（间接）未实现**：`mid_sentence*` 只进偏好；约束带进整句解码需要词典按音节筛边（`generate_constrained_sentences` 那层）和缓存键带约束签名，明显超出本次切片。
- **万象式直接未加开关**：`direct_helpcode_paths` 仍恒开（`enable_shuangpin_helpcode` 为真即可），`/` 与「第二位大写四码结束」未做。原因是跨端默认值冲突没拍板：Windows 出厂 `direct_helpcode=false`，但 macOS / 移动端的直接辅助码是已发布功能（见 [2026-10-17-mobile-direct-helpcode](2026-10-17-mobile-direct-helpcode.md)），按偏好默认 false 接门会让既有行为回归；需要先决定移动端是否显式打开，再把开关接进 `ShuangpinEngine::query`。
- **`engine-wasm` 未同步**：`SessionOptions` 新字段在 wasm 侧没有映射入口；网页引擎仍拿到默认（`direct=true`，即既有行为）。

## Alternatives considered

- **只加 `shuangpin_helpcode_direct` 一个开关。** 最强的理由是改动最小、能立刻让用户关掉「末尾直接把字母当辅码」的意外。但关掉后没有任何触发键可用，等于把辅助码整个关掉；Windows 的「直接关」是「改用间接引导键」，两者必须一起做才有意义。
- **在宿主层（macOS `InputController` / `input-runtime`）拦住触发键再拼进原始串。** 可以少碰引擎。但 macOS 输入法在进程内、宿主没有音节表，判断不了「前一个键是否构成完整音节」和约束落在哪个音节；这等于把引擎已有的规则在宿主再写一份，正是[移动端直接辅助码](../../implemented/feature/2026-10-17-mobile-direct-helpcode.md)否掉的「三平台各写一套」。
- **原样移植 `msime-windows` 的 C++ 契约。** 语义最完整，注释也最全。但那份契约由 TSF 同步吃键、Server raw 串、引擎解析三处共同维护，本仓没有 TSF 这一层，必须把三处折进 Rust 的 `Session` / `Engine`，并重建「解码时按音节筛边」；移植的是设计，不是可以直接复制的代码。
- **句中辅助码改成「解完整句后再过滤候选」。** 实现小，能复用现有整句结果与缓存。但被锁住的低频字组合可能从未进入束搜索，与移动端直接辅助码遇到的是同一个问题：只有过滤，找不到目标句。
- **不做，仅靠文档说明差异。** 诚实且零风险，但 Windows 用户切到 macOS 后无法沿用已有习惯，[辅助码公共偏好](../../implemented/simplification/2026-10-16-helpcode-preferences.md)和共享设置页也会继续与来源分叉。

## Verification

- `cargo test -p msime-engine -- direct_off_reads_trailing_letters_as_plain_pinyin helpcode_length_matches_the_composition_base`：接 `direct` 门之前两条都失败（`direct off` 仍把 `nihcc` 重排成「拟好」在前、`uiu` 的 `helpcode_length` 仍为 1），接门后全过。
- `cargo test -p msime-client-core -- save_normalizes_conflicting_shuangpin_helpcode_modes`：`save_locked` 未接 `normalized()` 时失败（`trigger_semicolon` 未被归一），接上后过。
- `cargo fmt --all --check`、`cargo test -p msime-engine -p msime-client-core`（约 2800 条）、`cargo clippy -p msime-engine -p msime-client-core --all-targets --locked -- -D warnings`、`cargo check -p msime-host-api -p msime-input-runtime -p msime-mcp-server --all-targets` 全绿。
- `bash scripts/run-checks.sh` 全过；`pnpm --dir apps/desktop exec tsc --noEmit` 干净；`pnpm exec vitest run` 相关用例（`settings.test.tsx`、`helpcode-page.test.tsx`、schema/import/catalog/preview）全绿。
- 未在真实编辑器里手验：本机验证手段不在本次切片内。

## Consequences

后续修复见[双拼未完成尾音节保持较长候选优先](../bug-fix/2026-10-22-shuangpin-incomplete-tail-candidate-order.md)：末尾直接辅助码仍保留，但尾字母没有命中任何基础候选时，完整前缀行先于较短整段回退行。

- 双拼专属字段没有摊进共用类型：`modes` 是 `HelpcodePreferences` 上唯一的双拼子节，等于默认值时序列化不出来，旧文档缺这一段按 Windows 出厂值读；`quanpin_helpcode` 里的 `modes` 字段存在但不被任何路径读取。
- `direct=false` 目前等价于把末尾辅助码整个关掉（触发键引擎还没实现），与 Windows「直接关 = 改用间接引导键」的完整语义差一层；用户在 macOS 上关掉直接辅助码后暂时没有任何触发键可用。
- 偏好文档里出现 `shuangpin_helpcode.modes` 时，保存路径会把它归一到互斥合法的形态，读路径不受影响。

[三组触发方式接入引擎](2026-10-22-shuangpin-helpcode-trigger-modes.md)接管间接触发、句中约束和万象式直接辅助码；本篇保留偏好结构、默认值和互斥归一的决定。


## Review verification

集成版本的全量引擎单测为 2494 通过、55 忽略、0 失败。随后只读审查发现并由真实会话回归复现两处边界：微软双拼反引号两码段的第二码大写后，下一音节的 `ing` 分号被奇偶判断拒收；逐音节直接混用第二码大写和 `/` 结束时，字面判断只按大写结束解析，误把合法中文组字覆盖为原文。

分号判断把反引号段与末尾段一起纳入扫描；字面判断把真实 `/` 开关传给与候选查询相同的直接路径解析器。两项新增回归先失败，修复后的双拼会话专项 53 项通过。该专项不是修补后重跑全量引擎的证明；最终本地 quick 与 CI 仍待完成。#9296 的交付范围是引擎、输入串契约及回归，原生 Android、iOS、Windows 尚未增加这些触发方式的设置入口。
