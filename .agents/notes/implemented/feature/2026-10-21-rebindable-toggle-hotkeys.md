# Agent Note: 中英/全半角/标点三个切换热键可重绑

Status: implemented

2026-10-23：macOS 的可打印主键统一按无修饰布局字符认，录入时转换物理 DOM code，相关决定由[布局键身份修复](../bug-fix/2026-10-23-toggle-hotkey-layout-identity.md)部分取代；既有布局跟随语义和共享 chord 格式继续沿用。

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

按四个切片落地：chord 契约与冲突表、macOS 宿主按键路由、共享设置页录键编辑器与 MCP 读写已交付；其他宿主的按键路由仍写死，是后面的切片。

`crates/client-core/src/key_chord.rs` 持有共享契约：`KeyChord { key: ChordKey, modifiers: ChordModifiers }`。`ChordKey` 是枚举（`Letter(char)`、`Digit(char)`、`Function(u8)` 加 Space/Semicolon/Period 等十二个具名键），`ChordModifiers` 是 `ctrl`/`alt`/`shift`/`meta` 四个布尔，`meta` 是 macOS Command、Windows Win、Linux Super 的中立名。字符串写法沿用旧版 v0.11.1 `key_chord.h` 的 `Key`/`Digit`/`F` 前缀和 `NamedKeys[]` 键名，与旧文档和设置页录入的写法互读；解析比旧版宽：大小写不敏感、修饰键按集合比较、`Ctrl`/`Control`、`Alt`/`Option`/`Opt`、`Meta`/`Win`/`Cmd`/`Command`/`Super` 互为别名；格式化输出唯一规范形（修饰键固定 `Ctrl`/`Alt`/`Shift`/`Meta` 顺序、键名取规范名），所以相等比较就是冲突判定（`conflicts_with` 是语义命名入口）。

`KeybindingPreferences` 去掉 `Copy`，加三个 `String` 字段：`toggle_english_candidates` / `toggle_full_width` / `toggle_punctuation`，出厂值与旧版 `input_state_shortcuts.h` 的 `defaultBinding` 相同（`TOGGLE_*_DEFAULT` 常量）。缺键按出厂值、等于出厂值不落盘（`skip_serializing_if`）、空串表示「不绑定」——不用 `Option`，因为旧版就用空串表示禁用，一份文档里不该有两种「不绑定」的写法。`Preferences` 与 `KeybindingPreferences` 都没有 `deny_unknown_fields`：旧版本读到新字段会忽略，新代码读旧文档拿到出厂值，两个方向都不炸。宿主经 `english_candidates_chord()` / `full_width_chord()` / `punctuation_chord()` 拿到 `Option<KeyChord>`，`None` 即不绑定。

`Preferences::validate` 的冲突表判三件事：三个热键两两不同；不占随开关开合的占用键（`Ctrl+Space`、`Ctrl+Alt+Space`、`Ctrl+Shift+KeyF`、`Ctrl+Shift+KeyJ`、`Ctrl+F9`、`Ctrl+Shift+F9`——Windows 的语音开关只看 Ctrl 按住、不看 Shift，`VoiceHotkeyPolicy.h` 的 `modifiers.ctrl()` 是 `lctrl || rctrl`，所以 `Ctrl+Shift+F9` 同样触发语音）；不占 macOS 无开关的维护键（`Ctrl+Alt+Shift+KeyC/R/T`，即 `InputControllerPhysicalKeys.h` 的 Ctrl+Shift+Option+C/R/T）。非法写法报 `InvalidToggleHotkey`，撞键报 `ConflictingToggleHotkeys`，不复用 `key_conflict`（它在 UI 文案里写死成「以词定字和翻页」）。

## macOS 宿主认键

写法的解析只在 Rust 一侧，宿主不实现第二份解析器：host-api 新增 `msime_client_toggle_hotkey_chords`，把 `{"keybindings": object}` 一次解成三个键各自的 `{"key": 主键规范名, "ctrl", "alt", "shift", "meta"}` 或 `null`（写法为空或不合法即不绑定）；keybindings 缺省或为 null 按出厂值，不是 object 时报错由宿主保持现值。规范名就是 `ChordKey` 的 Display 输出，宿主按规范名认键。该符号进了 `platforms/android/verify-native.sh` 的导出清单。

宿主侧 `platforms/macos/src/input/ToggleHotkeys.h` 是纯逻辑（不依赖窗口，直接进 CTest）：规范名解成 `ToggleChord`——字母主键存小写字母、经 `ShortcutLetter` 按当前布局认（Dvorak 上标着 E 的键就是 E），非字母主键存 ANSI keyCode；命中要求修饰键四旗精确相等，与原来各分支的掩码语义一致。`AppearancePreferences` 在 `applySharedInputPreferences` 时把三个 chord 解出并缓存，`InputController` 的三个分支改成 `MSIMEToggleChordMatches` 调它。

全半角是两条触发臂（`ToggleFullWidthArm`）：绑定的 chord（出厂 `Ctrl+Shift+Space`）在中英文模式下都生效；`Option+Shift+H` 不算进 chord，仍是 macOS 既有快捷键，由 `keybindings.toggle_fullwidth_option_shift_h` 单独门控、只在中文模式——和改动前的行为逐项对应。

## 共享设置页录键与 MCP

快捷键页新增「切换热键」组（`ToggleHotkeysSection`），三个槽位各一行 `KeyChordRecorder`：点击进入「按下组合键…」录制态，按下的修饰键与主键实时回显（DOM `KeyboardEvent.code` 的物理键名与 `KeyChord` 规范名逐字一致，录入不需要第二份解析器）；按下非修饰键即提交，`Esc` 取消不写，`Backspace`/`Delete` 提交空串解绑、行上显示「未绑定」。提交经 `SettingsClient.checkToggleHotkey` 走 Tauri 命令 `check_toggle_hotkey` 调 `KeybindingPreferences::check_toggle_hotkey`——解析、归一化、`conflicts_with` 与占用表（`occupied_toggle_chords`，`Preferences::validate` 用同一份）全部只在 Rust 一侧；通过回规范写法写入草稿，非法写法回 `invalid`、撞键回 `conflict` 加 `ToggleChordOwner::id()`（另一个切换槽位用文档字段名、开关占用用开关字段名、语音/维护键用 `voice_hotkey`/`macos_maintenance`），页面按 id 行内指出「与『全角 / 半角切换』相同」一类的冲突项，不写草稿。整组只在 `host.toggle_hotkey_chords`（宿主认这三个 chord，目前只有 macOS）且 `checkToggleHotkey` 在时渲染——没有桥的宿主不可能绕过校验写入。`save_locked` 在 `validate` 前调 `normalize_toggle_hotkeys`，任何写入方（MCP、原生宿主）写下的别名写法落盘时归一成规范形。

MCP 侧 `PreferencesView` / `PreferencesChange` 加了同名字段，`apply` 直接写 `keybindings`，归一化与冲突拒绝走 store 同一入口，错误串是 `InvalidToggleHotkey`/`ConflictingToggleHotkeys` 的 Display。

「一个 chord 是另一个的前缀」在这个模型里不存在：chord 是修饰键集合加单主键的单击组合，不是序列，`conflicts_with` 的相等判定完备，没有扩展语义。

原生侧范围：macOS AppKit 原生设置窗口冻结为回退（`docs/ARCHITECTURE.md`），本切片**不向它加这三个值的显示或录入**；它连三个切换键的既有行都没有，只读展示也无的放矢。可录入口只有共享 React 页（Tauri 设置应用、HarmonyOS 设置），其中能录的只有声明 `toggle_hotkey_chords` 的宿主。

## Alternatives considered

- **只做 macOS，其他宿主保持写死。** 一个宿主改完就能验收，改动面小一个数量级，也不碰 Windows TSF 的 compartment 语义。不选：chord 契约与冲突检测必须落在共享 `client-core`（`docs/ARCHITECTURE.md` 要求规则下沉），共享层有了字段却只有 macOS 消费，正是「已迁但无消费」的危险状态；Windows 两行「无条件吞掉」恰是冲突点。
- **先只做「字母键可绑」，Space / Period 留作固定。** 不用碰 `ShortcutLetter` 与 `PhysicalAnsiLetter` 的字母假设。不选：出厂值里就有两个非字母键，只支持字母等于做完就与旧版不一致。
- **往 `KeybindingPreferences` 直接加三个 `String`、不做 `KeyChord` 类型。** 复用现有字段位置最省事。不选：合法 chord 与互不冲突就没有单一真源，五个宿主会各自解析一遍，是 `trilingual_cycle` 那种语义漂移的来源。实际落的是折中形态：字段存 `String`（文档形态与旧版一致），解析收敛到 `KeyChord`。
- **做成「开关 + 固定组合」的 SelectRow。** 复用现成控件、零新结构。不选：只是把「写死」改成「在几档预设里选」，旧版的「可重绑」没有恢复。

## Consequences

`cargo test -p msime-client-core` 全绿（`key_chord` 模块 11 项单测加偏好层 10 项：`check_toggle_hotkey` 的规范归一、非法拒绝、按 owner 归因冲突、空串/解绑/重绑自己、开关开合改变占用表、保存归一化）；MCP 读写字段由 `msime-mcp-server` 的 `toggle_hotkeys_are_read_and_written_as_canonical_chords` 覆盖（别名写法归一落盘、非法与撞键拒绝且文档不动）。录键交互由 `apps/desktop/tests/settings/toggle-hotkeys.test.tsx` 六项 vitest 覆盖（三行渲染、录制提交、Esc 取消、Backspace 解绑显示未绑定、冲突与非法的行内错误）。`cargo fmt --all --check`、`cargo clippy -p msime-client-core --all-targets --locked -- -D warnings`、`cargo check -p msime-host-api -p msime-input-runtime -p msime-mcp-server --all-targets`、`pnpm --dir apps/desktop exec tsc --noEmit`、`bash scripts/run-checks.sh` 都过。

macOS 侧两条 CTest：`toggle-hotkeys` 覆盖规范名解码、chord 命中和 FFI 回答的一致性（把 `Period` 的 keyCode 改错时它先失败、恢复后转绿）；`shortcut-toggle-hotkeys`（`shortcut-test --toggle-hotkeys`）走完 `applySharedInputPreferences` → 解码 → `handleEvent` 的真实路径：改成新键后按新键命中、按旧键不再命中、空串不绑定，Option+Shift+H 不随 `toggle_full_width` 改动。全量 `shortcut` suite 在无图形会话的机器上先倒在皮肤颜色断言（上游自身问题），所以重绑路径单独注册成一条 CTest。

偏好文档形态是纯字符串：设置页和四份原生设置直接读写 `keybindings.toggle_*`，不再需要额外的序列化层；TS 契约随录键编辑器落地，三个字段已移出 `scripts/test-preferences-field-parity.py` 的 `RUST_ONLY`。非法 chord 在 `load`/`save` 的 `validate` 被拒，整份文档进恢复流程逐字段打捞，坏字段回到出厂值而不是整份重置——与既有校验失败的恢复语义一致。`KeybindingPreferences` 不再 `Copy`：全仓只有本 crate 与它的测试按值用它，下游 `msime-host-api`、`msime-input-runtime`、`msime-mcp-server` 编译零改动。

`preference_coverage.py` 曾把三个字段登记为「预留、尚未接线」；macOS 宿主开始消费后该登记按门禁规则删除（「已读却挂着不适用理由」也是失败）。剩余切片的边界：输入菜单里写死的键位标签（`InputController.mm` 的菜单项 `keyEquivalent` 只是展示用、不构成第二绑定）没跟着 chord 更新显示；Windows / Linux / HarmonyOS 的路由仍写死，`Ctrl+Shift+Space` 在 Windows 上依旧无条件吞掉，这三处的录键入口由 `toggle_hotkey_chords` 门控、随各自路由切片打开。

偏好测试新增在三态循环测试闭合之后，每项都有独立 `#[test]`：集成时漏掉旧函数闭合与第一项的属性会让整个 `lib test` 编译失败，也让出厂值断言无法被单独发现。修复后六项 `toggle_hotkeys` 回归通过。

已知边界：保留表只管「输入法自己占用」的组合键；操作系统级占用（旧版 `Warnings` 表里的 `Ctrl+A`/`Ctrl+C`/`Ctrl+Space` 等）只做提示不拒绝，与旧版语义一致。macOS 的「按 keyCode 认」与「按布局字母认」两条路径在 `ToggleChord` 里并拢（字母存字母、其余存 keyCode），Option+Shift+H 与维护键的行为逐项保留，但三个 chord 分支按声明顺序先者命中——`validate` 已保证共享文档里它们两两不撞，宿主不再另判。后面切片的风险原样保留：Windows 从「无条件吞掉」改成「可关」是行为变化，关掉后 `Ctrl+Shift+Space`/`Ctrl+.` 归应用；fcitx5 在输入法关闭状态下也吃 `Ctrl+.`，HarmonyOS 与 Linux 的「关闭状态下也生效」分支要显式建模。


审查补齐 macOS 偏好覆盖清单：三项绑定当时仅为共享契约预留、宿主仍用既有固定键，登记要求接入宿主时移除条目；macOS 路由落地后登记已按规则删除，设置页的录键入口仍未做。偏好测试之间保留各自的闭合与 `#[test]`，避免三语循环测试和新默认键测试被错误连接。
