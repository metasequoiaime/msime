# Agent Note: 不把 Windows 光标状态提示的位置与聚焦播报迁到 macOS 输入模式 HUD

Status: implemented

## Problem

遗留清点把 v0.11.1 的三个键列为「遗留未迁」，并称新架构「只剩一个布尔」：

- `general.caret_state_indicator`（出厂 `false`）
- `general.caret_state_indicator_on_focus`（出厂 `false`）
- `general.caret_state_indicator_position`（出厂 `"top-left"`）

要回答的问题是：macOS 的对应物 `MSIMEInputModeHUDPanel`（`platforms/macos/src/input/InputModeHUDPanel.mm`）是否真的缺「位置」和「聚焦才显示」，缺了就该补。

先纠正两处前提，它们影响判定：

1. `docs/macos-parity.md` 没有记录过「来源的 `caret_state_indicator` 换名为输入模式 HUD」。该文两处都把输入模式 HUD 列在「目标有而来源没有」的差集里（第 127、165 行），`docs/macos-feature-inventory.md` 第 74 行同样。三个旧键与新 HUD 之间没有书面的换名映射。
2. 清点把 `_on_focus` 说成「不能设成『聚焦输入框时才显示』」，这是误读。v0.11.1 的设置页文案是「焦点移到另一个输入框时（包括同一窗口内和其他窗口中的），也提示当前中英文状态」，`ShouldShowInputModeEvent` 对 `FocusEntered` 只返回 `focusAnnouncementEnabled`（`caret_state_indicator_policy.h:152-169`）。它是一个**额外的触发**，不是隐藏其余时机的前置条件，出厂还关着。

旧键的实际覆盖面也比「一个布尔」大。徽标共四类：中/英（`InputModeBadge`）、中英标点（`PunctuationBadge`，两格「，。」+「中」）、全/半（`CaretStateKind::Width`）、简/繁（`CaretStateKind::CharacterSet`，`event_listener_keys.cpp:1043`），由 `IMESwitch`、`PuncSwitch`、`DoubleSingleByteSwitch` 三类事件触发。位置是六向，不是四角：`top-left`、`top`、`top-right`、`bottom-left`、`bottom`、`bottom-right`，`top`/`bottom` 是正上、正下（`caret_state_indicator_policy.h:81-83`、`CaretStateIndicatorX` 第 100 行）。

本次迁移审查时，新架构的 macOS 对应物是另一件东西：只画「中」/「英」一个格子，18pt 字号、32pt 高、显示 0.6 秒（`InputModeHUDPanel.mm:12,19-20,201-207`），摆放规则固定在 `MSIMEInputModeHUDFrame` 里——水平居中于光标、优先下方、下面放不下翻到上方、夹进可见区域、没有可用光标时屏幕下方居中（第 36-49 行）。它没有位置字段，也没有焦点进入时的额外播报。

## Decision

**不迁。macOS 的输入模式 HUD 不是这三个键的等价物，也不应该为了对上键名去加位置和聚焦触发。**

后续用户明确要求的[繁简切换反馈](2026-10-25-macos-character-set-hud.md)扩展同一个单格 HUD，改变「仅中英文」的状态范围；本篇不迁入六向位置、焦点播报及旧版整套徽标的决定继续成立。

- 位置属于被裁掉的那套徽标体系。新架构只保留了「中/英」一格，标点、全/半、简/繁三类徽标没有对应物；把六向位置单独搬过来，留下的只是一个 32pt、0.6 秒提示的方向选择。
- 摆放规则已经是跨宿主的一份契约。Windows 的 `input_mode_hud_placement`（`platforms/windows/src/candidate/InputModeHudLayout.h:81`）按 macOS 的 `MSIMEInputModeHUDFrame` 逐条对齐，两端各有纯函数测试钉住「下方优先 / 放不下翻上 / 夹进工作区 / 无光标时回退」。给 macOS 单开六向位置会让这两份实现分叉，或者要求 Windows 一起接一个新偏好字段与设置项。
- 聚焦播报是跨宿主的增量触发，不是 macOS 遗漏。Linux 已有默认关闭的独立 `input_mode_focus_hud`，IBus 与 Fcitx5 都按它控制焦点及延迟提示；`input_mode_hud` 只控制真正的模式切换。Windows 和 macOS 尚不消费聚焦字段。详见 [Linux 聚焦提示](../bug-fix/2026-10-22-linux-focus-mode-hint.md)。为 macOS 增加额外触发仍需独立确定宿主能力与设置入口，不能用“Linux 无条件显示”作为迁移依据。
- 用键名对上是这次审计自己警告过的陷阱：`docs`/笔记里已记「名字和粒度都变了，按名字对不上」。

一句话：这是被新架构有意收窄的旧形态，不是 macOS 少做的两个开关。验收判据因此是「判定成立且可复核」，不是新增代码。

## Alternatives considered

- **加 `input_mode_hud_position`（六向，默认 `top-left`），只让 macOS 读。** 最强的理由：用户确实在旧版里选过方向，旧版六向齐全，改动面看起来只有一个纯函数加一个字段。不选：位置原本服务于四类徽标和 30 DIP 的双格标点徽标，收窄成单个「中/英」后收益很小；（订正：`Preferences` 本身并没有 `deny_unknown_fields`，未知键由 `deserialize_keeping_unknown` 记下并在保存时原样写回，见 `preferences.rs` 的 `UnknownPreferenceField`；所以「旧版拒读整份文档」不是这里的理由。真正的理由是前一条：位置服务的四类徽标已被裁掉。）还会逼出「Windows 要不要同样读」的问题，等于把一次平台补缺变成跨宿主契约变更。
- **加 `input_mode_hud_on_focus`（默认关），macOS 与 Windows 同读。** 最强的理由：能同时补上两个宿主相对 Linux 与相对旧版的差距，语义清楚，出厂关着不改变现有手感。不选：旧键的存在理由是 Windows 语言栏看不到光标旁的反馈，而 macOS 已有菜单栏状态项与 HUD 两条反馈路径（[菜单栏输入模式状态项](2026-10-12-macos-input-mode-status-item.md)）；再叠一个焦点播报更容易在换窗口时刷屏。真要加，应作为独立能力位与独立笔记讨论。
- **完整对齐 v0.11.1：恢复四类徽标 + 六向位置 + 聚焦触发。** 最强的理由：这是唯一能让升级用户「什么都不丢」的方案，也是「遗留未迁」这个判定最直接的字面解。不选：新架构在 [Windows 中英文切换对齐 macOS](2026-10-10-windows-input-mode-parity.md) 里明确把 Windows 拉向 macOS 的单格 HUD，方向是收窄而不是扩张；[紧凑尺寸](../bug-fix/2026-10-14-macos-compact-input-mode-hud.md) 又进一步固定了 18pt/32pt 并写明「保持原主题和位置策略」。撤销这两条要新的产品论证。

## Consequences

- **收益**：共享偏好、能力位、设置页与两端的摆放测试都不动；`MSIMEInputModeHUDFrame` 与 `input_mode_hud_placement` 继续是一份规则。
- **代价与已知上限**：从 v0.11.1 升级的 Windows 用户拿不回六向位置和「换输入框也提示」，macOS 用户从一开始就没有这两项。若以后要补位置，扩展点是 `MSIMEInputModeHUDFrame` 的参数化纯函数 + 一个共享字段 + 能力位门控，六向的语义可以直接照 `CaretStateIndicatorX` 的「左侧 / 居中 / 右侧」三档；补 macOS 聚焦播报则应评估如何复用现有默认关闭的 `input_mode_focus_hud`，以及能力位和设置页的显示范围；Linux 已由该字段独立控制。
- 清点文档里 L6 的措辞（「聚焦才显示」「四角」）应改成「额外的焦点触发」「六向」再引用，否则下一次审计会按同一处误读重新立项。

## Verification

- 只读核对，未改产品代码。固定来源 `metasequoiaime/msime-windows` @ `dd018f79858314ed542a47c528608752b13bd261`（`v0.11.1`）：`installer/default_config/config.default.toml:18-25` 三个键的注释与出厂值；`server/src/settings/native/native_page_floating_toolbar.cpp:176-198` 卡片标题「光标状态提示」与三行控件、六个位置选项；`server/src/window/caret_state_indicator_policy.h:53-104` 显示条件、六向集合与 `CaretStateIndicatorX`；`server/src/ipc/event_listener_worker.cpp:380-410` 三类切换事件；`windows/src/Thread/ThreadFocusSink.cpp:62-90` 与 `windows/src/IME/MetasequoiaIME.h:156` 的 120ms 焦点播报。
- 当前新架构以 `3579090da7` 为固定基线核对生产源码：`Preferences` 同时具有默认开启的 `input_mode_hud` 和默认关闭、仅 Linux 消费的 `input_mode_focus_hud`。`ClientEngine.cpp::show_input_mode_hint` 与 Fcitx5 的对应路径按触发类型分别检查两个字段；设置页仅在 Linux 暴露聚焦提示。macOS 的 `MSIMEInputModeHUDFrame` 继续采用固定布局，HUD 由 `setEnglishInputMode:` 的模式变化分支触发，`activateServer:` 不播报聚焦。笔记本身引用旧键名，因此不能把包含笔记的全仓搜索写成零命中证据。
- 核对 [Linux 聚焦提示](../bug-fix/2026-10-22-linux-focus-mode-hint.md) 后原地更新事实，保留“不把旧版整套徽标迁入 macOS”的决定；本篇不撤销 Linux 已落地的独立开关。笔记门禁与 quick 随本次审查执行。
