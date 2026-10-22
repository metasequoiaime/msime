# Agent Note: 中英/全半角/标点三个切换热键可重绑

Status: implemented

## Problem

旧版 Windows 正式版（v0.11.1，`metasequoiaime/msime-windows` @ `dd018f79`）有三个可重绑的输入状态切换热键，工厂值在 `installer/default_config/config.default.toml:427-429`，定义来自 `engine/contracts/input_state_shortcuts.h:21-23`：

| 键 | 出厂值 | 用途 |
| --- | --- | --- |
| `keybindings.toggle_english_candidates` | `"Ctrl+Shift+KeyE"` | 中英切换（英文候选模式） |
| `keybindings.toggle_full_width` | `"Ctrl+Shift+Space"` | 全角 / 半角 |
| `keybindings.toggle_punctuation` | `"Ctrl+Period"` | 中英文标点 |

新架构里这三个功能都在，热键却被写死，而且**五个宿主各写死一遍**。这三个键在新架构里**完全不存在**：全仓搜 `toggle_english_candidates` / `toggle_full_width` / `toggle_punctuation` 零命中，`platforms/windows/installer/config.default.toml` 的 `[keybindings]` 只有 4 项，也没有它们。

| 宿主 | 位置 | 认法 |
| --- | --- | --- |
| macOS 标点 | `platforms/macos/src/input/InputController.mm:750-753` | `event.keyCode == 47` 且修饰键恰为 Control |
| macOS 全半角 | `platforms/macos/src/core/FullWidthInput.h:11-20` | `keyCode == kVK_Space` 且恰为 Control+Shift |
| macOS 中英 | `platforms/macos/src/input/InputController.mm:6225-6228` | `ShortcutLetter == 'e'` 且恰为 Control+Shift |
| Windows 中英 | `platforms/windows/tsf/Key/KeyEventSink.cpp:303-307`（`IsEnglishInputModeToggle`） | `code == 'E'` 且修饰键位掩码为 3 |
| Windows 全半角 | `platforms/windows/tsf/Key/KeyEventSink.cpp:717-721` | 无条件吞掉 `VK_SPACE` + Ctrl+Shift，不读任何开关 |
| Windows 标点 | `platforms/windows/tsf/Key/KeyEventSink.cpp:737-741` | 无条件吞掉 `VK_OEM_PERIOD` + Ctrl |
| Linux IBus | `platforms/linux/src/core/ClientEngine.cpp:6535-6537`、`:6546-6547`、`:6548` | 三个布尔，与设置页无关 |
| Linux fcitx5 | `platforms/linux/fcitx5/FcitxEngine.cpp:7859-7863`、`:7906-7909`、`:7911-7914` | 同上 |
| HarmonyOS | `platforms/harmony/entry/src/main/ets/keyboard/InputModeRouting.ts:130-161` | `KEYCODE_E` / `KEYCODE_SPACE` / `KEYCODE_PERIOD` 三个字面量 |

用户可见后果：和别的软件抢键时无法避让。`Ctrl+Shift+Space` 在 Windows 上被无条件吞掉（不看任何开关），`Ctrl+Shift+F`（简繁）有开关，同文件 `:722-735` 的 Alt+Shift+H 也是可关写法——只有这三个没有。

### 前提不成立：仓库里没有可重绑机制

- **`keybindings` 全是布尔，没有键位串。** `crates/client-core/src/preferences.rs` 的 `KeybindingPreferences` 原有 8 个字段，全部 `bool`，且派生 `Copy`。没有 `String` 字段，没有解析器，没有格式化器。
- **设置有「开关」，没有「绑定」。** `packages/ui/src/settings/keybindings` 全部是 `SwitchRow`（开关）与 `SelectRow`（在**预定义**的几档里选一个），没有一个是录键控件。
- **语音的「可绑」是枚举，不是自由录入。** `packages/ui/src/settings/voice-hotkeys.ts` 的 `VOICE_HOLD_KEYS` 是三个布尔里选一个。
- **面板快捷键是只读速查。** `packages/ui/src/settings/panel-shortcuts-section.tsx` 用字符串模板拼出 `Ctrl+Shift+Command+K` 交给 `ShortcutRow`，没有任何写入口。
- **全仓没有 chord 类型、没有录键控件。** `crates/client-core/src/panels.rs:52-59` 的 `ClientKeyEvent` 是**注入按键**的载荷，不是绑定，也不存偏好。
- **历史那套只存在于 v0.11.1。** `engine/contracts/key_chord.h` 的 `FanyImeKeyChord::Parse`、`Binding{key, modifiers}`、`NamedKeys[]`（Space/Semicolon/Period/… 12 个具名键）都在旧仓，没有迁进本仓。

## Decision

按四个切片落地，本提交只交付第一步「chord 契约与冲突表」；宿主按键路由和设置页编辑器都还没接，是第二、三、四步。

`crates/client-core/src/key_chord.rs` 持有共享契约：`KeyChord { key: ChordKey, modifiers: ChordModifiers }`。`ChordKey` 是枚举（`Letter(char)`、`Digit(char)`、`Function(u8)` 加 Space/Semicolon/Period 等十二个具名键），`ChordModifiers` 是 `ctrl`/`alt`/`shift`/`meta` 四个布尔，`meta` 是 macOS Command、Windows Win、Linux Super 的中立名。字符串写法沿用旧版 v0.11.1 `key_chord.h` 的 `Key`/`Digit`/`F` 前缀和 `NamedKeys[]` 键名，与旧文档和设置页录入的写法互读；解析比旧版宽：大小写不敏感、修饰键按集合比较、`Ctrl`/`Control`、`Alt`/`Option`/`Opt`、`Meta`/`Win`/`Cmd`/`Command`/`Super` 互为别名；格式化输出唯一规范形（修饰键固定 `Ctrl`/`Alt`/`Shift`/`Meta` 顺序、键名取规范名），所以相等比较就是冲突判定（`conflicts_with` 是语义命名入口）。

`KeybindingPreferences` 去掉 `Copy`，加三个 `String` 字段：`toggle_english_candidates` / `toggle_full_width` / `toggle_punctuation`，出厂值与旧版 `input_state_shortcuts.h` 的 `defaultBinding` 相同（`TOGGLE_*_DEFAULT` 常量）。缺键按出厂值、等于出厂值不落盘（`skip_serializing_if`）、空串表示「不绑定」——不用 `Option`，因为旧版就用空串表示禁用，一份文档里不该有两种「不绑定」的写法。`Preferences` 与 `KeybindingPreferences` 都没有 `deny_unknown_fields`：旧版本读到新字段会忽略，新代码读旧文档拿到出厂值，两个方向都不炸。宿主经 `english_candidates_chord()` / `full_width_chord()` / `punctuation_chord()` 拿到 `Option<KeyChord>`，`None` 即不绑定。

`Preferences::validate` 的冲突表判三件事：三个热键两两不同；不占随开关开合的占用键（`Ctrl+Space`、`Ctrl+Alt+Space`、`Ctrl+Shift+KeyF`、`Ctrl+Shift+KeyJ`、`Ctrl+F9`、`Ctrl+Shift+F9`——Windows 的语音开关只看 Ctrl 按住、不看 Shift，`VoiceHotkeyPolicy.h` 的 `modifiers.ctrl()` 是 `lctrl || rctrl`，所以 `Ctrl+Shift+F9` 同样触发语音）；不占 macOS 无开关的维护键（`Ctrl+Alt+Shift+KeyC/R/T`，即 `InputControllerPhysicalKeys.h` 的 Ctrl+Shift+Option+C/R/T）。非法写法报 `InvalidToggleHotkey`，撞键报 `ConflictingToggleHotkeys`，不复用 `key_conflict`（它在 UI 文案里写死成「以词定字和翻页」）。

## Alternatives considered

- **只做 macOS，其他宿主保持写死。** 一个宿主改完就能验收，改动面小一个数量级，也不碰 Windows TSF 的 compartment 语义。不选：chord 契约与冲突检测必须落在共享 `client-core`（`docs/ARCHITECTURE.md` 要求规则下沉），共享层有了字段却只有 macOS 消费，正是「已迁但无消费」的危险状态；Windows 两行「无条件吞掉」恰是冲突点。
- **先只做「字母键可绑」，Space / Period 留作固定。** 不用碰 `ShortcutLetter` 与 `PhysicalAnsiLetter` 的字母假设。不选：出厂值里就有两个非字母键，只支持字母等于做完就与旧版不一致。
- **往 `KeybindingPreferences` 直接加三个 `String`、不做 `KeyChord` 类型。** 复用现有字段位置最省事。不选：合法 chord 与互不冲突就没有单一真源，五个宿主会各自解析一遍，是 `trilingual_cycle` 那种语义漂移的来源。实际落的是折中形态：字段存 `String`（文档形态与旧版一致），解析收敛到 `KeyChord`。
- **做成「开关 + 固定组合」的 SelectRow。** 复用现成控件、零新结构。不选：只是把「写死」改成「在几档预设里选」，旧版的「可重绑」没有恢复。

## Consequences

`cargo test -p msime-client-core` 447 项全绿：`key_chord` 模块 11 项单测（桩实现先跑出 10 项失败、实现后转绿）加偏好层新增 6 项；`cargo fmt --all --check`、`cargo clippy -p msime-client-core --all-targets --locked -- -D warnings`、`cargo check -p msime-host-api -p msime-input-runtime -p msime-mcp-server --all-targets` 都过。

偏好文档形态是纯字符串：设置页和四份原生设置直接读写 `keybindings.toggle_*`，不再需要额外的序列化层；TS 契约（`packages/ui/src/settings/keybinding-preferences.ts`）随第四步一起进。三个字段在 `scripts/test-preferences-field-parity.py` 的 `RUST_ONLY` 登记为「Rust 先行」，第四步录入控件落地时移出。非法 chord 在 `load`/`save` 的 `validate` 被拒，整份文档进恢复流程逐字段打捞，坏字段回到出厂值而不是整份重置——与既有校验失败的恢复语义一致。`KeybindingPreferences` 不再 `Copy`：全仓只有本 crate 与它的测试按值用它，下游 `msime-host-api`、`msime-input-runtime`、`msime-mcp-server` 编译零改动。

已知边界：保留表只管「输入法自己占用」的组合键；操作系统级占用（旧版 `Warnings` 表里的 `Ctrl+A`/`Ctrl+C`/`Ctrl+Space` 等）只做提示不拒绝，与旧版语义一致。第二步的风险原样保留：macOS 合并「按 keyCode 认」与「按布局字母认」两条路径时不能碰坏 Option+Shift+H 和维护键；Windows 从「无条件吞掉」改成「可关」是行为变化，关掉后 `Ctrl+Shift+Space`/`Ctrl+.` 归应用；fcitx5 在输入法关闭状态下也吃 `Ctrl+.`，HarmonyOS 与 Linux 的「关闭状态下也生效」分支要显式建模。
