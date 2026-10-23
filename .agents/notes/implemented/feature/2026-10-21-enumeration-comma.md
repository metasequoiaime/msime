# Agent Note: 顿号自定义（`\` 与 `/` 输出「、」）

Status: implemented

## Problem

新架构的共享标点表只把 `\` 映射成顿号「、」，`/` 既不映射，也没有任何偏好能开关这两个键。Windows v0.11.1 有「顿号自定义」：用户选中文标点下用哪些键输出 `、`。这一项没有搬进新架构，是四个平台共有的能力缺口。

原始审计位置（第 2 版契约时的快照）：引擎固定表 `crates/engine/src/punctuation.rs` 的 `SIMPLE` 只有 `(b'\\', "、")`；`crates/client-core/src/preferences.rs` 的标点族只有 `chinese_punctuation`、`paired_punctuation`、`punctuation_lock`、`caps_lock_ascii_punctuation`、`quote_style`、`bracket_style` 六项；Windows 来源在 `_refs/msime-windows`（v0.11.1，提交 `dd018f7`）的 `installer/default_config/config.default.toml:284-286`、`engine/contracts/enumeration_comma.h`、`windows/src/Composition/CompositionProcessorEngine_Modes.cpp:69-89`、`server/src/config/ime_config.cpp:701-708`、`server/src/config/ime_config_keys.cpp:279-322`、`server/src/config/ime_config_template.cpp:111-115`、`server/src/settings/native/native_page_input.cpp:1872-1881`。

## Windows 规格（v0.11.1，已确证）

| 项 | 值 / 行为 | 证据 |
| --- | --- | --- |
| 可调项 | 两个独立布尔：`[input] enumeration_comma_backslash`、`enumeration_comma_slash` | `config.default.toml:284-286` |
| 出厂模板缺省 | 两个都 `true`（新装） | `config.default.toml:285-286` |
| 运行时缺键缺省 | 两个都 `true` | `ime_config.cpp:701-702` |
| 升级用户 | 旧配置没有 `enumeration_comma_slash` 时并成 `false`，保持「只有 `\`」；已有该键则以用户值为准 | `ime_config_template.cpp:111-115`；`server/tests/src/test_config_template_merge.cpp:36-57` |
| 至少留一个 | setter 拒绝关掉最后一个；手改成两个都关时按两个都开读 | `ime_config_keys.cpp:287-297, 306-314`；`ime_config.cpp:704-708` |
| 生效语义 | 勾选的键在中文标点下出 `、`；没勾的键不是中文标点，按原样输出 | `CompositionProcessorEngine_Modes.cpp:69-89`（`ResolveEnumerationCommaKey`）、`:98-104`（`IsPunctuation`）、`:148-153`（`GetPunctuation`） |
| 与标点开关 | 只作用于中文标点模式；`IsPunctuation` 对没勾的键返回 FALSE，键交回普通字符路径 | 同上 |
| 与智能标点 | `、` 在空格转换名单里，按空格转回 ASCII | `config.default.toml:274` |
| IPC | Worker 帧 `EnumerationCommaKeysChanged`：载荷 `'1'` 只有 `\`，`'2'` 只有 `/`，`'3'` 两个都开；`'0'`/非法退回 `'1'` | `enumeration_comma.h:8-29`；`windows/src/IPC/Ipc.h:285-290`；`ime_config_keys.cpp:318-322`；`server/src/window/ime_windows_candidate.cpp:468, 619-625` |
| 设置页 | 原生「顿号自定义」段，两个勾选 `\ 反斜杠` / `/ 斜杠`，说明「中文标点状态下用哪些键输出顿号「、」，至少保留一个；未勾选的键按原样输出」 | `native_page_input.cpp:1872-1881`；`settings_app.cpp:337-338, 683-686`；`native_settings_bindings.cpp:137-167`（`AtLeastOneChecks`） |

## Decision

按 Windows 的语义补成共享能力，一处解析、各宿主只消费。本提案落地时契约已从第 2 版演进到第 3 版（`{`/`}` 进 `alternating` 与 `cornerQuotes`），本次升到第 4 版。

- **偏好**：`Preferences` 加 `enumeration_comma_backslash: bool`（缺省 `true`）与 `enumeration_comma_slash: bool`（缺省 `false`，见拍板 D2）。等于缺省时不写进文档（`skip_serializing_if`），与 `quote_style`、`bracket_style` 一致。
- **至少留一个**：`crates/client-core/src/punctuation.rs` 的 `punctuation_marks(&Preferences)` 是唯一的偏好解析处，把「两个都关」归一成「`\` 开」（`backslash || !slash`），与 Windows IPC 无效值回退一致，但不照搬旧版配置读取时的双开兜底。serde 表达不了跨字段不变量，文档照样读得进来、读出时归一；设置页另有一道守卫（唯一开着的开关变灰）。引擎不再兜底：它收到的 `PunctuationMarks` 已是归一后的值，两个都关就只按开关执行。
- **契约**：`shared/contracts/punctuation/policy.json` 新增顶层段 `enumerationComma`，列出 `\` 和 `/` 两个键都出 `、`；`contractVersion` 升到 4，重跑 `generate.py` 生成 `policy.h` 的 `enumeration_comma` 数组和 `enumeration_comma_output` 查找函数，C++ 契约测试与 README 同步。`simple` 保持「无条件映射」语义不动：`\` 仍留在 `simple`（第 1 版起的历史行为，`is_supported`/`simple` 读者不受影响），`/` 只进 `enumerationComma`——所以 `is_supported('/')` 仍为假，读 `simple` 的宿主不会把 `/` 当成永远的中文标点。`generate.py` 校验：段内键唯一、可来自 `simple` 或是新键、不得与交替/嵌套键重叠、输出同为 `、`。
- **引擎**：`PunctuationMarks` 带 `enumeration_comma_backslash` / `enumeration_comma_slash` 两个布尔（缺省「`\` 开、`/` 关」），`translate` 对没启用的键返回 `None`，键按普通字符交回宿主。`SessionOptions.punctuation_marks` 与 `EngineOptions`（`enumeration_comma_backslash`、`enumeration_comma_slash` 两个扁平布尔，与 `corner_quotes`/`hollow_brackets` 同形状）照常接。
- **`/` 与命令模式：命令模式优先**（拍板 D3）。`handle_character` 里 `local_mode_for_symbol` 只在无组字时先于标点翻译执行，所以词首的 `/` 仍打开命令模式；其余位置（组字中、命令模式关闭、方案不开表模式、`commit_bare_prefix` 的裸前缀）按偏好出顿号。引擎本来就按位置分得清，不需要额外判据。`commit_bare_prefix` 经 `translate` 翻译前缀键，slash 开着时 `/` `,` 出 `、，`。
- **host-api / View**：`apply_punctuation_marks` 把两个布尔写进 `EngineOptions`；`PunctuationMarks`（client-core）序列化两个新字段，`msime_client_view` 的 `punctuation_marks` 报告 `{"quotes","brackets","enumeration_comma_backslash","enumeration_comma_slash"}`；`msime_client.h` 的 View 注释同步。
- **设置页**：共享 React 页「标点」组的「更多选项」里加「顿号自定义」两个开关（`\` 默认开、`/` 默认关），唯一开着的那个开关变灰作「至少留一个」守卫；`punctuationControlState` 的显示归一与读出归一一致（两个都关按「`\` 开」显示）。桌面 `PunctuationSection` 与鸿蒙手机 `PhonePunctuationSection` 都加。点击任一顿号开关时，连同当前显示的归一状态一起保存两个字段，避免手改双关闭文档在下一次点击后撤掉仍显示为开的反斜杠。
- **账号同步**：`settings_sync.rs` 导出/应用 `input.enumeration_comma_backslash` 与 `input.enumeration_comma_slash` 两个布尔（Android、桌面、鸿蒙共用 `insert_input_settings`）。沿用 `quote_style` 的先例：msime-cloud 字段表收录之前，上传前的过滤会去掉它们；应用侧写出原值，远端理论上的「两个都关」由读出归一兜底。

## 拍板记录

- **D2 `enumeration_comma_slash` 缺省 = `false`。** 新架构没有「升级合并」，所有现有用户相当于 Windows 的升级用户；`false` 让缺省行为逐字不变——`/` 在词首仍是命令模式、其余位置仍是普通字符，与「缺省值下行为不变」的惯例一致。Windows 新装的「两个都开」不被采纳：新架构用户不是从 Windows 迁移来的新装用户，`/` 出顿号改成默认会把「组字中按 `/` 上屏顿号」这个可见行为变化推给所有人。
- **D3 命令模式优先。** `/` 在词首（无组字）仍打开命令模式；其余位置按偏好出顿号。理由：命令模式是 `/` 的现行语义且只有词首一个入口位置，引擎天然按 `has_composition()` 分得清，不需要超时/前缀之类不稳定判据；用户要 `/` 在词首也出顿号时，关掉命令模式即可（`/` 掉到标点路径按偏好走）。

## 未做（有意留给后续）

- **触屏键面与空格反查的本地表同步**（契约 README 要求的跟随清单）：已落地，见 [六宿主本地标点表跟进契约与顿号空格反查随开关](./2026-10-22-punctuation-host-tables-enum-comma-space.md)。
- **新架构 Windows 的 Server→TIP 新帧** `EnumerationCommaKeysChanged`（载荷 `'1'/'2'/'3'`）：`windows_ipc.h` 追加帧、`ReplyCodec.cpp` 发帧、TIP 侧 `PunctuationTable` 与 `Global::StyledPunctuation` 接住。本轮 Windows Server 读偏好 JSON 时只看到 `quote_style`/`bracket_style`，顿号键仍是「只有 `\`」（旧 TIP 的历史行为，兼容）。
- **各平台原生设置页**：Windows 原生设置页、iOS/Android 原生标点页、HarmonyOS 原生页的本机控件。共享 React 页已覆盖 macOS/Linux/HarmonyOS（ArkWeb）。
- **msime-cloud 的 `internal/account/preferences_fields.json`**：外部仓库，两个键在字段表收录前只本地生效（上传被过滤）。收录后同步即刻生效，代码已在 `settings_sync.rs`。
- **`、` 的空格反查（D4）**：已随宿主表同步一并定规则并落地——`source` 是开着的顿号键时转回它，否则 `\` 优先、`/` 其次，两个都关按 `\`；实现与逐宿主表结论见 [六宿主本地标点表跟进契约与顿号空格反查随开关](./2026-10-22-punctuation-host-tables-enum-comma-space.md)。
- **多个引擎层的「两个都关」兜底**：归一只在偏好解析处和设置页，引擎照开关执行，不再重复守。

## 历史检索

按 `顿号`、`enumeration`、`标点`、`punctuation` 检索 proposed + implemented + rejected，分类如下：

- [引号样式与方头括号样式](../../implemented/feature/2026-10-13-punctuation-mark-styles.md)：**部分重叠**。同一份 `policy.json`、同一批手抄表、同一族偏好；那篇持有 `styles`/`quote_style`/`bracket_style` 的行为，本篇持有「按键可开关」与第 4 版 `enumerationComma` 段，互链保留。
- [共享标点偏好的叶子类型](../../implemented/simplification/2026-10-15-punctuation-preferences.md)：**部分重叠**。持有 React 偏好叶子类型与字段归属；新增两个字段走同一处，互链保留。
- [大写锁定时使用英文标点](../../implemented/feature/2026-10-09-caps-lock-ascii-punctuation.md)：**部分重叠**。同为「标点族新偏好」；它那次没动契约、没往表里加字所以先只上了 macOS，本篇动了契约，`/` 只进 `enumerationComma` 段使宿主本地表不跟也不漂移，互链保留。
- [拼音标点保留原始输入](../../implemented/bug-fix/2026-10-14-punctuation-raw-commit.md)：**无关**。那是未完成拼音的回退路径，不涉及标点表。

## Alternatives considered

- **只给 macOS 加、其它宿主「待跟进」。** 最强理由是仓库有先例（[大写锁定标点](../../implemented/feature/2026-10-09-caps-lock-ascii-punctuation.md) 就先只上了 macOS）。没采用：那次没动契约、没往表里加字；本项把 `/` 放进 `enumerationComma` 而不是 `simple`，核心引擎先统一接线，已暴露的桌面英文模式也必须消费开关；触屏键面和 Windows TIP 仍按下列切片边界跟进。
- **引擎里另起一张 `/`/`\` 表，完全不碰契约。** 最强理由是不用升版本、改动最小。没采用：契约 README 写明 `policy.json` 是唯一真源、`Nobody copies the mapping into a platform-specific table`；`matches_the_shared_contract` 的存在就是为了防这件事。
- **`/` 直接进 `simple`，偏好只做运行时抑制（D1a）。** 最强理由是 `simple` 加一行最省事，`generate.py` 不用新增段。没采用：会把 `/` 写成无条件中文标点，`is_supported('/')` 变真，任何读 `policy.h` 的消费者不再能区分「永远映射」和「偏好可关」；`/` 进 `simple` 后本轮没跟进的英文模式表也会立刻把它当顿号键。
- **只做 `/` → 、，不暴露偏好。** 最强理由是改动最小。没采用：改变所有现有用户的 `/` 行为，丢掉 Windows「任一个可关」的能力。
- **单个三值选择代替两个布尔（D5）。** 最强理由是一个控件天然互斥、不用守卫。没采用：与 Windows 的两个勾选和 TOML 两个键对不上，偏好迁移和设置页都要另做映射；「至少留一个」由唯一开着的开关变灰 + 读出归一解决。
- **`/` 命令模式与顿号在词首共存（D3 备选）。** 最强理由是两边功能都保留。没采用：没有稳定判据区分「要命令模式」和「要顿号」，来源产品也没有这个做法；用户关掉命令模式即可让词首 `/` 出顿号。
- **`enumeration_comma_slash` 缺省 `true`（D2 备选）。** 最强理由是跟 Windows 新装一致、`/` 出顿号是中文输入法的常见默认。没采用：新架构没有升级合并，缺省对所有人生效，会把「组字中 `/` 上屏顿号」推给全部现有用户。
- **引擎再兜一层「两个都关」归一（D7 备选）。** 最强理由是防御纵深。没采用：归一处只有一个（`punctuation_marks`），引擎收到的是归一后的值，重复归一只会让两处规则将来可能不一致；引擎的行为是「照开关执行」，两个都关就都是普通字符，简单可预测。

## Consequences

- **收益**：`\`/`/` 两个键的开关在共享引擎与共享设置页就位；契约第 4 版的 `enumerationComma` 段把「可被偏好关掉的键」这个概念写成了明确语义，`simple` 仍是无条件表；View 报告解析结果，宿主不解析偏好。
- **代价**：`Preferences` 带 `deny_unknown_fields`，选过非默认值的用户回退到旧版本时会拒读整份文档——与 `quote_style`、`bracket_style`、`caps_lock_ascii_punctuation` 是同样的代价。`/ `在词首出不出顿号取决于命令模式开关，设置说明里写明了这层关系。
- **缺口**：宿主本地表与 Windows TIP 帧未跟进前，`/` 顿号只覆盖引擎标点路径与共享设置页；macOS 与 Linux 的英文模式固定中文标点也读取 View 的两个开关，关闭的键按原字符或全角字符处理；触屏键面与空格反查仍在「未做」清单里。账号同步在 msime-cloud 收录字段前只本地生效。
- **验证**：`matches_the_shared_contract` 对 `policy.json`/`policy.h` 逐行断言（版本 4、`enumerationComma` 两行、`enumeration_comma` 数组）；`punctuation_contract.cpp` 断言 `version == 4`、`is_supported('/')` 为假、`enumeration_comma_output` 两键出 `、`。引擎 `enumeration_comma_keys_follow_their_switches` 覆盖「默认 `/` 不出顿号 → 打开 `slash` 出 → 关掉 `backslash` 不出 → 两个都关都是普通字符」；`contract_table` 把 `/` 放进 `None` 键列。client-core `enumeration_comma_keys_keep_at_least_one` 覆盖四种开关组合与两个都关的归一；preferences 测试覆盖缺省不写文档、缺键按缺省读、两个都关的文档照样读入、写盘往返。host-api 测试断言 View 报告两字段、`\` 出顿号、`/` 打开后出顿号、关掉 `\` 后不处理。input-runtime 测试断言 View 序列化含两字段。设置页测试覆盖缺省显示、唯一开着的开关变灰、两个都关的文档按「`\` 开」显示。`pnpm run verify-notes` 与 `scripts/run-checks.sh`（含 `test-punctuation-contract.py`）通过。

审查补齐 `EnglishModeOutput.h` 及 macOS、IBus、Fcitx5 的 View 解码，防止同一偏好只在中文模式生效。回归覆盖四种已解码组合、英文标点和小键盘排除、全角回退，以及真实 macOS `handleEvent` 的「反斜杠关闭、斜杠开启」路径；会话回归另覆盖词首命令模式、裸前缀和组字中的斜杠。
