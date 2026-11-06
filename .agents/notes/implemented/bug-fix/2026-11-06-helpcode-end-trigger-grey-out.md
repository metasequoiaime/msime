# Agent Note: 末尾直接或逐音节直接打开时置灰末尾间接触发

Status: implemented

## Problem

#11010：双拼辅助码设置里，「末尾直接辅助码」开着时，「末尾间接触发（分号 / 反引号 / 大写字母）」三行仍可勾选，但勾了没有效果。出厂值正是「末尾直接」和「分号触发」同时打开（`ShuangpinHelpcodeModes::default()`），所以每个用默认配置的双拼用户看到的都是一个打着勾、实际不生效的开关。「逐音节直接辅助码」开着时同理。

引擎沿用 Windows 参考实现（`ime_session.cpp:386-404`）的规则：末尾间接段只在「辅助码开、末尾直接关、逐音节直接没接管」时生效。判断写在三处：

- `crates/engine/src/session/input.rs` 的 `shuangpin_helpcode_marker`：末尾触发键只在 `!modes.direct && !modes.direct_helpcode` 时记成标记字符。
- `crates/engine/src/ime/mod.rs` 的 `apply_request_options`：条件不满足时清掉 `end_helpcodes`，串里残留的段降级成分隔符。
- `crates/engine/src/shuangpin/mid_sentence.rs` 的 `uppercase_triggers`：末尾大写要求 `!modes.direct` 且 `!modes.direct_helpcode`。

六个平台的设置行表里，这三行的父开关都是「无」，只随辅助码总开关置灰，与引擎的判据对不上。

## Decision

引擎规则不变，设置页照引擎的判据置灰这三行。

- **置灰条件**：`direct || direct_helpcode` 为真时，`trigger_semicolon`、`trigger_backtick`、`trigger_uppercase` 三行不可改，叠加在原有的辅助码总开关（以及 macOS 的 Tab 手动辅助码）之上。句中间接（`mid_sentence`）不参与这条判据，引擎也不看它。
- **只置灰，不改值**：存着的值原样显示、原样保存。关掉末尾直接以后，三行立刻恢复成用户原来的选择；出厂的分号触发此时直接生效。
- **说明写明原因**：「末尾直接辅助码」的「?」说明改为「完整音节后直接接辅助码；打开时末尾间接触发不生效」，各端逐字相同。逐音节直接的说明不加这句：它本来就是另一种整句模式，用户打开它时不会期待末尾触发键还在。
- **各端的落点**：每端仍各有一张行表，这次给它们加同一个父开关类型。
  - 共享页 `packages/ui/src/settings/pages/helpcode-page.tsx` 的 `modesRows`：`endTriggerDisabled = disabled || modes.direct || modes.direct_helpcode`。macOS、Linux、HarmonyOS 用它。
  - Windows `platforms/windows/settings/HelpcodeModesModel.h`：`ModeParent::end_trigger`，`mode_enabled(mode, helpcode_on, direct, mid_sentence, direct_helpcode)`。WinUI（`main.cpp` 的 `helpcode_mode_rows`）和 Win7（`HelpcodeForm::enable`）都传入 `direct`。
  - Android `HelpcodeModesPolicy`：`Parent.END_TRIGGER`，`enabled(mode, direct, midSentence, directHelpcode)`，调用处是 `TypingPage.helpcodeModeRows`。
  - iOS `HelpcodeSettingsView`：`Mode.Parent.endTrigger`，`modeEnabled` 从 `stored` 读 `direct` 和 `direct_helpcode`。

判据与引擎一致，不是另一套规则：以后引擎放宽或收紧末尾间接的生效条件，四张行表要跟着改。

## Alternatives considered

- **改引擎，让末尾直接和末尾间接同时生效。** 理由最强的一点是用户勾了就该生效，而且出厂值两项都开，改完以后默认配置下分号触发「自然就能用」，设置页不用加任何规则。不采用：出厂值是 `direct=true`、`trigger_semicolon=true`，改引擎就等于改了所有默认配置双拼用户按 `;` 的结果。今天 `;` 在这些用户手里是标点（微软双拼里还先让给 `ing` 韵母），改完以后它会开一段辅助码。这是对现有用户手感的静默改动，按 AGENTS.md 需要单独授权；它也会让引擎偏离 Windows 参考实现，两边的串契约和互斥判断要重新对齐。
- **在 client-core 写入端归一，打开末尾直接时清掉三枚触发键。** 理由最强的一点是它和现有的「句中间接占用同一枚触发键时清掉末尾」是同一种做法，页面显示的就是存下的真值，不会出现「打着勾但置灰」。不采用：这会改写用户存着的值，关掉末尾直接以后原来的选择回不来。出厂值本身两项都开，归一以后默认文档也不再等于出厂值，`modes` 段会被写进每个用户的文档并随账号同步出去。这又是一次改变缺字段读法的改动。
- **末尾直接或逐音节直接开着时把三行藏起来。** 理由是看不见就不会误以为能用。不采用：藏起来以后用户不知道这三项存在，也看不到从别的设备同步过来的值；置灰加上「?」里的那句说明，能直接告诉用户关掉哪一项才能用它们。#10417 隐藏设置组时连带藏掉了一个缺字段读作开启的开关，鸿蒙用户再也关不掉，同类做法有前车之鉴。

## Consequences

- 默认配置下，三行显示为灰色，分号那一行打着勾。这是存着的出厂值，不代表生效；「末尾直接」的说明里写明了这一点。
- 收益：设置页的可改状态与引擎行为一致，不再有勾了不生效的开关，而且不改任何人的输入手感、不改任何存着的值。
- 代价：行表多了一种父开关，四个地方（共享页、Windows 头文件、Android 策略、iOS 扩展）都要人工保持同步。各端测试钉住了这条规则和说明文字，任何一端漏改都会变红，但不会自动对齐引擎。
- Win7 的 `windows7_settings.cpp` 原来在末尾直接开着时直接点「末尾反引号」，现在那一行置灰，测试改为先关掉末尾直接再点，并补了打开逐音节直接后三行重新置灰、分号的值不被改写的断言。

## Testing

- 共享页：`apps/desktop/tests/settings/helpcode-page.test.tsx` 新增用例，覆盖出厂值下三行置灰且勾选状态不变、两者都关时可改、逐音节直接打开时又置灰，以及「末尾直接」的说明文字。改动前这条用例失败，改动后通过。
- Windows：`platforms/windows/tests/ui/helpcode_modes_model.cpp` 覆盖新的父开关类型和 `mode_enabled` 的全部组合，本机用 clang++ 编译运行。Win7 的 `windows7_settings.cpp` 只在 CI 的 Windows runner 上跑。
- Android：`HelpcodeModesPolicySmoke`（`platforms/android/check-host.sh`）。
- iOS：`HelpcodeModesTests.testEndTriggersFollowDirectAndDirectHelpcode`。

## Existing notes

- [原生设置补上双拼辅助码触发方式](../feature/2026-11-02-native-settings-helpcode-modes.md)持有各端行表和原有的置灰规则，它的置灰一条已就地补上末尾间接的判据并指向本篇。
- [触发方式全部接入引擎](../feature/2026-10-22-shuangpin-helpcode-trigger-modes.md)持有引擎侧的末尾间接实现和它的生效条件，本篇不改它。
- [双拼辅助码补齐直接、间接与句中触发方式](../feature/2026-10-21-shuangpin-helpcode-trigger-modes.md)持有偏好结构、出厂值和写入端互斥，本篇不新增互斥。
