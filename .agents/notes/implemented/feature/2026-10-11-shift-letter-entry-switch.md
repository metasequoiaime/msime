# Agent Note: Shift+字母进入快捷模式的总开关

Status: implemented

## Problem

#6807：中文模式下按住 Shift 连续打大写，只要首字母是 U/T/K/E/M/J/Y/R（以及打开了的 V），没有组字时这一下就进入对应的快捷模式，后面的字母都被模式吃掉，「TEST」打不出来。这些入口字母沿用来源 Windows 产品，写死在 `crates/engine/src/session/input.rs` 的 `local_mode_for_entry`，默认全开；用户只能逐个关掉模式。Apple 和搜狗的习惯是 Shift+字母直接上屏大写。

## Decision

- 共享偏好 `local_modes.shift_letter_entry`，缺省开（serde 缺省也是开，旧文档读出来行为不变）。经 host-api 写进 `EngineOptions::local_shift_letter_entry`，再到 Engine 的 `LocalModeOptions::shift_letter_entry`。
- 关掉时 `local_mode_for_entry` 直接返回 `None`：没有组字时 Shift+字母不进任何模式，落到普通路径，大写字母 `unhandled` 交还宿主输出。各模式自己的开关保留原值；`/`、`@` 不是 Shift+字母，不受影响；`date_time` 同时管着的拼音日期时间关键词（`riqi`、`sj`）也不受影响。
- 判定放在 Engine，所以六个宿主行为一致，宿主不用改按键路径。`HostCapabilities::shift_letter_entry_switch` 只在三个桌面宿主上为真，只有那里显示这个开关：iOS 的「更多 → 本地输入」和其他触屏入口经同一个 Engine 入口打开模式，在那里关掉它会让菜单失灵。
- 界面：共享设置页「输入 › 快捷模式」组顶上一行总开关，关掉时 Shift+字母的各模式开关保留勾选、置灰；macOS 原生设置窗口的快捷模式卡片、Windows 原生设置窗口的「快捷模式」组同样各加一行。快捷模式仍在「输入」页，不回「快捷键」页（`docs/windows-parity.md` 2026-10-02 一节的分组决定不变）。

## Alternatives considered

- **连续 Shift 大写自动退出模式**（分诊推荐的最小改法）：刚进模式、缓冲区只有入口字母时，下一键又是 Shift+大写字母，就退出模式并把两个字母原样交给应用。不用新开关，默认行为就对按住 Shift 打大写友好，而 Shift+T 接小写 `rq` 仍进 T 模式。但入口字母已经被模式吃进去并显示成预编辑，退出时要「上屏入口字母并放行当前键」的新结果类型，六个宿主都要接；第二个字母之前用户已经看到了模式的提示，体验上是闪一下；而且只覆盖两个及以上连续大写，单个大写字母（句首的「T」）仍会进模式。维护者选了总开关。
- **改默认值，默认关掉部分模式**：现有用户的习惯被悄悄改变，也和来源产品不一致；想要快捷模式的人要一个个找回来。
- **入口字母可自定义**：`LocalModePreferences` 从布尔改成「开关加入口键」，Engine 的映射查表，各宿主的说明文案都要动态化。改动面大，和来源产品不一致，这次不做。

## Consequences

- **收益**：一个开关解决「Shift+字母打不出大写」，缺省开，现有用户无变化；关掉后各模式设置原样保留，再打开即恢复。
- **代价**：
  - 关掉后大部分快捷模式仍没有别的桌面入口；macOS 的 K 模式例外，可用默认未绑定的[附加快捷键](2026-11-13-quick-phrase-extra-hotkey.md)独立进入，其余模式仍需要总开关。
  - `shift_letter_entry` 在 `LocalModePreferences` 里，结构带 `deny_unknown_fields`，早于它的设置应用或 `msime-mcp` 会拒读写过这个字段的文档；正式包里输入法和设置应用版本一致。没有加进账号设置同步。
  - 网页引擎（`crates/engine-wasm`）本来就关掉了所有本地模式，总开关在那里也设为关，行为不变。

## Verification

- Engine：`shift_letter_entry_switch_off_types_every_capital` 覆盖关掉后九个入口字母都 `unhandled`、不进模式、不留组字，各模式开关不变，`/` 照常进指令模式，以及开着时 Shift+T 进日期时间模式。全部 engine、golden、client-core、host-api、input-runtime 测试通过。
- `client-core`：`shift_letter_entry_defaults_on_and_survives_a_save` 覆盖缺省开、旧文档缺键读作开、关掉后保存读回且只改它自己。
- 共享设置页：`local-modes-section.test.tsx` 覆盖没有能力时不显示、缺键读作开、关掉写回、关掉后 Shift+字母的模式开关保留勾选并置灰而 `/` 模式仍可改。
- macOS CTest `local-mode-preferences`：原生开关共九个，关掉总开关后模式勾选保留并置灰、写进共享文档，真实会话里 Shift+T 交还应用、不进模式，打开后恢复。
- Windows 原生设置窗口的一行只能交给 Windows CI 编译。没有在任何平台上实机打字验证。
