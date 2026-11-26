# Agent Note: macOS 单按 Shift 进入大写英文

Status: implemented

## Problem

#11846 提了两件事。一是 macOS 单按 Shift 切到英文后字母仍是小写，要连续输入大写字母就得一直按住 Shift。二是带 Shift 的游戏快捷键会被输入法的英文直写路径消费。本次只处理第一件，游戏快捷键的路由拆成单独的工作，本 PR 不改英文模式下 Shift 组合键的处理。

## Decision

维护者决定把大写英文做成可选设置，出厂默认关闭。共享偏好新增 `keybindings.shift_tap_uppercase_english`（`crates/client-core/src/preferences.rs` 的 `KeybindingPreferences`），缺字段的旧文档读成关闭，经偏好文档交给宿主，目前只有 macOS 宿主读它。

开关打开时，只有单按 Shift 真的把中文切成英文，控制器才进入大写英文：切换前记下 `wasEnglish`，切换后 `_uppercaseEnglishMode = shiftTap && 开关 && !wasEnglish && englishMode`。在这个状态下，英文模式里的 a–z 由输入法改成 A–Z 写进客户端，不交还应用，因为应用收到的是原始的小写按键；全角或中文标点规则照旧作用在大写字母上。数字、标点和小键盘不受影响，Shift+标点照常走中文标点锁定和全角规则。

其它改变或重新推导中英文的入口都清掉这个状态：`setEnglishInputMode:`（菜单、系统输入源报告、应用规则、单按 Control 或 Caps Lock 都经过它）、`setDedicatedEnglishInputMode:`、激活时的 `activateInputModeForApplication:` 调用处（含 `activateServer:` 和按键接管会话）。状态只在控制器运行时，不写进偏好。

界面按 ARCHITECTURE.md 的「macOS 的 AppKit 设置窗口冻结为回退」：规则先放进共享 Rust，macOS 的开关随原生设置迁移接入；冻结的 AppKit 回退窗口和 `packages/ui` 都不加控件。字段在 `scripts/test-preferences-field-parity.py` 的 `RUST_ONLY` 里登记了原因。迁移前可以直接编辑偏好文档打开。

## Alternatives considered

- **让单按 Shift 进入的英文一律大写，不加开关**：之前的版本就是这样，实现最少，但会改变所有老用户单按 Shift 后的手感，维护者要求默认关闭，放弃。
- **大写状态下把带 Shift 的组合键都交还应用**：能顺带照顾游戏快捷键，但会让 Shift+标点绕过中文标点与全角规则，而且游戏快捷键是另一件事，拆出去单独处理，放弃。
- **在共享 React 设置页或 AppKit 回退窗口加开关**：用户马上能找到入口，但两者都在迁移期冻结，不加新设置，放弃。

## Consequences

默认设置下行为与改动前一致。打开开关后，单按 Shift 进英文即可直接打出大写字母，再单按 Shift 回中文或从任何其它入口切换都回到普通英文。`ShortcutTest.mm` 的 `TestShiftTapUppercaseEnglish` 覆盖默认关闭、只在中文切英文时生效、大写字母由输入法写入、Shift+标点仍走中文标点、菜单、Control 轻点和英文候选模式都不会进入或保留大写；`crates/client-core/tests/shift_tap_uppercase_english.rs` 覆盖默认值和持久化往返。macOS 的设置界面入口留给原生设置迁移。
