# Agent Note: 把二三候选接进共享运行时并让 macOS 可用

Status: implemented

## Problem

#6501 要在组字时用 `;` 选当前页第 2 个候选、`'` 选第 3 个。共享偏好 `second_third_candidate { enabled, keys }` 和共享 TypeScript 类型早已存在（`crates/client-core/src/preferences.rs:1103`、`packages/ui/src/core/preferences-model.ts:81`），默认关闭、关闭时不写进文档（`crates/client-core/src/preferences/tests.rs:2274`）。当时只有 Windows 接入（[用分号和单引号选第二、第三个候选](../../implemented/feature/2026-10-09-second-third-candidate-keys.md)），macOS 侧的现状是：

- 共享 React 设置页不渲染这一项。
- `platforms/macos/tests/settings/preference_coverage.py` 把它列进 `NOT_APPLICABLE`，理由正是「只有 Windows 的 TIP 和 Server 路由这两个键，共享页不显示开关，macOS 用户没法打开一个宿主会忽略的东西」。
- 引擎只把 `b'1'..=b'9'` 映射成候选序号（`crates/engine/src/session/input.rs` 的 `handle_candidate_key`）；`'` 是拼音音节分隔符，`;` 是微软双拼 `ing` 的韵母，都不是选词键。
- 共享运行时 `crates/input-runtime/src/runtime.rs` 的 `candidate_shortcut` 是选词键的统一拦截点，但只查 `wubi_candidate_selection`；通用 `second_third_candidate` 没有分支。
- macOS 宿主已经把 `;`/`'` 作为 `Action::Character` 经 `typeASCII:` → `msime_client_character` 送进共享运行时——不是「宿主碰不到这两个键」，而是「运行时没有按通用偏好解释它们」。

## Decision

在**共享运行时**接线，而不是照抄 Windows 的宿主级策略。

- `crates/input-runtime/src/runtime.rs`：`Runtime` 加 `second_third_candidate` 字段与 `set_second_third_candidate` / `second_third_candidate()`（对照 `wubi_candidate_selection` 的写法）。`candidate_shortcut` 推广成通用入口：先走 `wubi_candidate_shortcut`，再走 `second_third_candidate_shortcut`，命中即返回 `Action::Select`。边界与五笔那支一致：`focused`、`snapshot_valid`、`editing_text` 非空、`local_mode == "none"`、非 `dedicated_english`；键位只认枚举里唯一的 `semicolon_quote`（`;` → 当前页 slot 1，`'` → slot 2），`SecondThirdCandidatePreferences::slot` 与五笔的 `slot` 同放在 client-core。
- 「哪些方案/模式不接管」照 Windows `SecondThirdCandidatePolicy.h` 的排除表，落在两个自由函数上：`second_third_candidate_scheme`（日文，以及由宿主自己组字或自己开候选列表的韩文、注音、越南文、藏文）与 `microsoft_shuangpin_final_position`（自上一个 `'` 起奇数长度的片段后，`;` 是韵母 ing，与引擎 `accepts_ing_key` 同一条判断）；`spelling_symbols` 列出该键时（网址、V 模式拼写）也不接管。
- 偏好下发：`crates/host-api/src/lib.rs` 的偏好应用路径与 `crates/host-api/src/ffi/session.rs` 的会话初始化都把 `second_third_candidate` 转给 runtime。
- 共享设置页：`packages/ui/src/settings/input-shared-settings-section.tsx` 的「选词与翻页」组在以词定字之后加一行 `二三候选` 开关，与 Windows 原生设置页同名同位；写入时 `enabled` 与 `keys: "semicolon_quote"` 一起写，偏好里不留缺字段的对象。
- `platforms/macos/tests/settings/preference_coverage.py` 的 `NOT_APPLICABLE` 删掉 `second_third_candidate`：宿主开始经共享运行时读它后，条目按脚本规则必须移除。

拍板（原提案列出的三处，均按推荐一支落地）：

1. **`'` 与音节分隔符 / 中文引号冲突 → 照 Windows：开关优先。** 用户显式打开后，组字中的 `'` 是选词键、不再是音节分隔符；设置页说明写明这一取舍。macOS 的引号成对补全只在没有组字时介入，而开关只在组字中接管，不受影响。
2. **`;` 与微软双拼 `ing` 冲突 → 照抄 Windows 排除。** 自上一个 `'` 起奇数长度的片段后 `;` 是输入，其余时候是选词键，与 Windows `microsoft_shuangpin_final_position`、引擎 `accepts_ing_key` 逐条对齐；同一偏好在两个平台不分叉。
3. **设置页可见范围 → 全平台放开，不新增 host capability。** 共享运行时对驱动它的每个宿主统一生效（macOS、Linux 的 Tauri 设置应用、HarmonyOS 的 ArkWeb 页），开关放共享页后这些平台同时可用。加 capability 门控反而制造「页面有开关、宿主不读」的不一致，正是覆盖表脚本要防的缺陷。

## Alternatives considered

- **照抄 Windows，在 macOS 宿主 `InputController.mm` 里判断物理键。** 最强理由是它与 `platforms/windows/common/SecondThirdCandidatePolicy.h` 一一对应，评审熟悉，宿主也最清楚物理键位与布局。但 macOS 宿主已经把这两个键当普通字符送进共享运行时，在宿主层再判断会多复制一份「方案 / 本地模式 / 拼写符号」的排除逻辑；Linux、HarmonyOS 等同样驱动这个 runtime 的宿主之后还要各写一遍。共享运行时里已有 `candidate_shortcut` 这个现成拦截点，五笔的二三候选就是这么做的。
- **下沉到引擎，让 `handle_candidate_key` 接受 `;`/`'`。** 最强理由是改动最集中，所有宿主自动受益。但引擎收到的是字符而不是物理键，`'` 在引擎里已经是音节分隔符、`;` 是微软双拼的 `ing`，改这两个字符的语义会直接波及组字本身；[2026-10-09 的笔记](../../implemented/feature/2026-10-09-second-third-candidate-keys.md)也记录了 Windows 刻意不把判断放进引擎。共享运行时的 `candidate_shortcut` 是在引擎之上、宿主之下的一层，不属于当时被否掉的那条路。
- **不做。** 最强理由是 `wubi_candidate_selection` 已经覆盖五笔用户的快捷选词，而通用偏好只有 Windows 在用。但 #6501 的诉求是全拼/双拼用户，Windows 已用通用 `second_third_candidate`；不做会让同一个偏好在不同平台行为不一致。

## Consequences

- 打开后，组字中的 `'` 不再是音节分隔符，`;`/`'` 也不再在组字时上屏标点——Windows 已接受的取舍，设置页说明写明。关闭时行为逐字节不变。
- `;`/`'` 只在组字中且当前页有对应座位时接管；页内无第 2/3 候选时照旧走原标点路径，不会像越界数字键那样被吞掉。
- 所有驱动共享运行时的宿主同时获得这项能力；Windows 原生路径不变，排除表与 `SecondThirdCandidatePolicy.h` 一一对应，两边规则同源。
- 运行时是按「打出的字符」判断的，不像 Windows 同时校验虚拟键码：非美式布局在这两个键上打出别的字符时不受影响，但别的键打出 `;`/`'` 同样会被当作选词键。这是共享层拿不到物理键的固有边界，与偏好语义一致（它说的就是字符）。
- 回滚成本低：默认关闭且不写进文档，删掉 runtime 分支与设置页那一行即可，无数据迁移。

## Testing

- `crates/input-runtime/src/tests.rs` 新增三个用例，验证过先失败后通过（临时断开 `candidate_shortcut` 的接线时两个用例失败：`';` 上屏为 `合成候选3-remaining-segments;` 而非选中候选；接线后全绿）：
  - `second_third_candidate_shortcuts_select_current_page_on_all_character_routes`：`Character`、`Punctuation`、`PunctuationAscii` 三条路由下 `;` 选第 2、`'` 选第 3，且选的是当前页。
  - `second_third_candidate_shortcuts_keep_original_route_when_disabled_or_target_is_missing`：关闭或当前页无对应座位时走原标点路径；无组字时不被吞。
  - `second_third_candidate_shortcuts_respect_scheme_mode_focus_and_engine_input`：日文、韩文、注音、越南文、藏文不接管；英文模式、本地模式、失焦不接管；`spelling_symbols` 列出时不接管；微软双拼奇数片段（含 `'` 后重计）的 `;` 仍是 ing，偶数片段后是选词键。
- `cargo test -p msime-input-runtime -p msime-client-core` 全绿；`cargo check -p msime-host-api -p msime-input-runtime -p msime-mcp-server --all-targets` 与 `cargo fmt --all --check` 通过。
- `preference_coverage.py` 不再把 `second_third_candidate` 列为 `NOT_APPLICABLE` 并通过（93/114 偏好可达）。
- `pnpm --dir apps/desktop exec tsc --noEmit` 干净；新增 `apps/desktop/tests/settings/second-third-candidate.test.tsx` 断言开关只写 `{ enabled, keys: "semicolon_quote" }`；相关设置页 vitest 全绿。
- macOS 宿主路径未改 `InputController.mm`：`;`/`'` 已作为 `Action::Character` 进共享运行时，现有测试基建没有覆盖 `typeASCII:` 到运行时选词的集成路径，真机上按过这两个键的验证仍缺——与 Windows 笔记记录的 TIP 键路未实按属同类缺口。
