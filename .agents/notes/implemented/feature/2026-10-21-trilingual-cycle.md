# Agent Note: 中英日循环切换

Status: implemented

## Problem

旧版 Windows 正式版有一个开关 `keybindings.trilingual_cycle`（`installer/default_config/config.default.toml:432`，出厂 `false`）。打开后，**「切换中英文」那一组快捷键**（默认是单按 Shift，另有单击 Ctrl、Ctrl+Alt+Space）不再只是在中文和英文之间来回，而是按 中 → 日 → 英 → 中 转圈；有组字时先把正在组的原始字母上屏。

新架构里这个字段完全不存在（`rg trilingual crates/ packages/` 零命中）。现在只有：

- `keybindings.toggle_japanese_ctrl_shift_j`（`crates/client-core/src/preferences.rs:2188`）——**Ctrl+Shift+J** 单次在中文和日语之间来回，另一个键；
- 各宿主的「切换中英文」是简单的二态切换。

所以缺的不是「能不能切到日语」，而是「**一个键在 中 → 日 → 英 → 中 之间循环**」。

## Decision

### 语义按旧版 Windows 抄，不重新设计

三处旧源码定死了行为：

1. **哪一个键参与循环**：`windows/src/Key/KeyEventSink_Hotkeys.cpp:382-395` 只在 `GetPreservedKeyAction(hotkeyGuid) == ToggleImeMode` 时把这一键换成 `FUNCTION_CYCLE_INPUT_MODE`，也就是**「切换中英文」那一组快捷键**。Ctrl+Shift+J 是另一条路：`windows/src/Key/KeyEventSink.cpp:514` 的 `IsEnglishInputModeToggle` → `FUNCTION_TOGGLE_ENGLISH_CANDIDATES`，是 Engine 的专用英文候选模式，和循环无关。
2. **顺序**：`engine/contracts/trilingual_input.h:16-19` 的 `Next(ime_enabled, japanese)` 返回 `!ime_enabled ? Chinese : japanese ? English : Japanese`，即 中文 → 日文 → 英文 → 中文。测试把这一点钉住：`server/tests/src/test_input_key_policy.cpp:5-14` 的 `trilingual_shift_cycles_chinese_japanese_english_and_returns_to_chinese`。
3. **有组字时上屏什么**：`server/src/ipc/event_listener_keys.cpp:903-908` 先保留已选中的词段（`composition.creating_word.word`），再拼上还没选的原始拼音串（`get_pinyin_sequence_with_cases()`）；注释明说「retain an already selected word prefix, then commit the remaining raw spelling without choosing or learning a candidate」。**上屏的是原始拼音串（加上已选词段），不是高亮的候选**，也不学习、不算选词。

循环开启时，所有参与的切换键先提交原文；关闭时，修饰键轻点继续提交原文，Ctrl+Option+Space 与 Shift+Space 保留原来的取消组字规则。原文提交失败则消费这次切换键并保留当前方案与组字，不能继续换方案。

### 共享层是纯函数，四个宿主可以共用

`crates/client-core/src/language_cycle.rs` 是状态机本体，只依赖 `preferences::{InputScheme, ChineseScheme}`：

- `LanguageCycleState { mode, scheme, last_chinese_scheme, has_composition }` 是按下循环键那一刻的状态；
- `next(enabled, state) -> Option<LanguageCycleStep>`，`LanguageCycleStep { mode, commit_raw, scheme }`。`None` 表示这个键不循环（开关关着），调用方照旧走二态切换；
- `chinese_target(last_chinese_scheme, scheme)` 回答从英文回中文要切到哪个方案：优先历史中文方案，其次当前中文方案，否则全拼；宿主再按版本与词库可用性选择可运行的中文方案。

C ABI 入口 `msime_client_language_cycle(enabled, mode, scheme, last_chinese_scheme, has_composition)`（`crates/host-api/src/ffi/input.rs`，声明在 `crates/host-api/include/msime_client.h`）把同一个函数原样交给原生宿主，返回 `{mode, scheme, commit_raw}` 或 `null`。Foundation 包装是 `MSIMEClientSession -languageCycleEnabled:...`。这样 Windows、Linux 接上时不必重写顺序，只要在自己的键路径上把状态填进去。

### 偏好与设置页

- `KeybindingPreferences::trilingual_cycle: bool`，缺键读作 `false`（`#[serde(default)]`），对齐旧版 Windows 出厂值。
- 设置页一行在「快捷键 › 输入模式切换」，紧跟「切换中英文」下拉框，因为它是这一组键的行为而不是另一个组合键。标签「中英日循环切换」，说明写出顺序和「先上屏原始字母」。
- 显示条件 `offersTrilingualCycle(host)`：宿主声明 `HostCapabilities::trilingual_cycle`，并且本版本同时提供中文方案和日语（和 `offersJapaneseToggle` 同一条规则，用的是同一份版本表）。

### 目前只有 macOS 消费

`HostCapabilities::trilingual_cycle` 只在 `HostPlatform::Macos` 为真。macOS 的 `InputController.mm` 新增 `applyTrilingualCycleIfEnabled`，接进「切换中英文」的四条路径（单按 Shift、单按 Control、Ctrl+Option+Space、Shift+Space，后者两条经新增的 `toggleChineseEnglishMode` 汇合）：先核对版本同时支持中文与日语，再问共享层；`null` 退回各自原来的二态切换。

**这是有意的平台差异，不是漏迁。** Windows 的 TIP 和 Server 是两个进程：循环要新开一条协议（旧版是 `FanyImeProtocol::TrilingualCycle` 能力位 + `DataFromServerMsgType::TrilingualCycle` 回包 + `TrilingualCycleChanged` 推送，见旧版 `engine/contracts/README.md:47-62`）。Linux 的 IBus 和 Fcitx5 各自处理切换键。这两端接上时只需在 `HostCapabilities::for_platform` 里加上自己，并调用已经存在的 `msime_client_language_cycle`，不用改顺序。

## Alternatives considered

- **把循环做成每个宿主各写一份**：最省事，Windows 有旧代码可抄。但顺序、有组字时的行为、英文那一态的回程会在四个地方各写一遍，旧版已经因为「Server 从全局状态算目标」在 `ime_mode_scope=app` 下踩过坑（见 `trilingual_input.h` 的注释：每个宿主有自己的状态，要从请求方的状态算）。共享层的纯函数一笔写清，四个宿主只做状态翻译。
- **复用 `toggle_japanese_ctrl_shift_j`**：把 Ctrl+Shift+J 改成三态循环，不动「切换中英文」。改动更小，但和旧版语义不符：旧版参与循环的是「切换中英文」那一组键，Ctrl+Shift+J 在旧版里根本不存在（它是新架构加的中日快捷键）。用户按 Shift 得到的是二态切换，按 Ctrl+Shift+J 才会转圈，和文档、和用户从旧版带过来的习惯都对不上。
- **英文那一态也继续转回日语**：更「对称」，但 Windows 的 `Next()` 在 IME 关着时返回中文。照抄它，英文里按循环键就等于二态切换的回程，用户不会因为多按一下就掉回日语。
- **顺手把 Windows 和 Linux 也接上**：范围会膨胀到改 TIP↔Server 协议和两个 Linux 引擎的键路由，正是任务里说的「降级」条件。状态机、偏好、ABI 和 macOS 消费先落地，另外三端留给后续改动。

## Consequences

- **收益**：一个键走完 中 → 日 → 英 → 中；有组字时上屏的是用户打出来的字母，不是替他选的字；顺序、开关语义和「英文回中文」的判断只有一份实现，四端将来接上时不会各写一套。默认关，关着时所有宿主的行为一字不变。
- **代价**：`HostCapabilities::trilingual_cycle` 目前只有 macOS 是 `true`，Windows、Linux 的用户在设置页看不到这一行。这是显式的平台差异，不是「已迁但无消费」：共享层有单测、ABI 有单测、macOS 的翻译层有 ctest，只有消费端少两个平台。
- **`msime_client_language_cycle` 是新导出**，Android 的 `verify-native.sh` 导出清单同步加上了它（`scripts/test-android-jni-names.py` 会核对头文件声明）。
- **验证**：
  - Rust：`cargo test -p msime-engine -p msime-client-core -p msime-input-runtime`；`crates/client-core/src/language_cycle.rs` 的 6 个单测（关闭时二态、完整一圈、英文只回中文、有组字时先上屏、无组字不上屏、回到记下的中文方案），`preferences/tests.rs` 的 `trilingual_cycle_defaults_off_and_persists_an_explicit_enable`，`host_surface/tests.rs` 的能力位，`crates/host-api/src/tests.rs` 的 `language_cycle_abi_publishes_the_shared_state_machine`。
  - macOS：`trilingual-cycle-routing`（`platforms/macos/tests/input/TrilingualCycleRoutingTest.mm`）覆盖两侧翻译；`InputController.mm` 只做了 `clang -fsyntax-only`，没有装进输入法在真机上按过。
  - 设置页：`apps/desktop` 的 `input-mode-shortcuts-section` 两个新用例、`pnpm format:check`。
  - 没验证到的：真机上的循环、Windows 和 Linux 的接线、以及循环在日语方案缺词库时的回退（日语缺词库时仍可纯假名输入；版本不提供日语时不启用循环）。

审查补充：`ShortcutTest.mm` 新增真实引擎、偏好持久化与控制器按键回归，覆盖没有中文历史的日语用户从英文回中文、原文提交失败保留组字、完整三态循环，以及关闭开关后组合键取消/修饰键轻点原文提交的既有差异。远端 run `20261021-193234` 的 quick 门禁通过；Rust 路由、偏好与 ABI 定向测试及严格 Clippy 通过。原生源码强制重编并移除临时诊断后，run `20261021-194628` 的 `shortcut`、`trilingual-cycle-routing`、`input-mode-identifiers` 三项 CTest 全部执行通过，共 41.30 秒。早期复用镜像曾因跨分支源文件时间戳回退而读到旧对象；之后显式触碰相关原生源文件再重编，不能把旧缓存的中止计作产品失败或通过。

最新 develop 合并后的 run `20261021-212441` 完整 quick 通过，`shortcut`、`trilingual-cycle-routing` 两项共 43.58 秒通过；单独执行的 `input-mode-identifiers` 通过（0.30 秒）。未做系统输入源中的实体按键验收。
