# Agent Note: Fcitx 中英文切换的修饰键生命周期与字段边界

Status: implemented

## Problem

Fcitx 输入上下文可能在收到 Ctrl 按下后失焦，而松开事件留在其他窗口。宿主保留 `ctrl_down_`，重新聚焦后又用这个旧值阻止单按 Shift 切换中英文。普通按键事件已经不带 Ctrl，重复轻按 Shift 仍无法恢复；单独按下并松开 Ctrl 可以清除残留状态。

另一条独立阻塞来自字段属性。Nautilus 50.2.2 的地址栏设置 `GTK_INPUT_HINT_NO_SPELLCHECK`，Fcitx GTK 前端将其映射为 `NoSpellCheck`。宿主的 `privateInput()` 把它与 `Sensitive` 合并，用于禁止学习、联网等行为；模式切换也复用了这个判断，导致普通地址栏无法单按 Shift 切换。焦点清理不会改变字段属性，重启输入法也不会消除这条限制。

## Decision

`FcitxState::cancelModifierTaps()` 在失焦、关闭和重置时结束当前修饰键手势，清除按住状态、布防和计时。已经观察到的未完成按键保留为取消标记，随后迟到的松开不构成新手势；新的完整按下／松开仍可重新布防。判断另一修饰键是否仍按住时同时核对原始事件的修饰位，覆盖重新聚焦后只收到松开的前端。

改动只属于 Linux Fcitx 适配层，不改变切换开关、长按阈值、输入模式记忆或其他平台。

修饰键松开与 `toggleInputMode()` 的切换判定只检查受限字段及 `Sensitive`，不以 `NoSpellCheck` 禁止本地切换。`privateInput()` 及其对学习、云候选、AI、剪贴板的限制保持不变；同时带 `NoSpellCheck` 和敏感或受限标志的字段仍不能切换。该边界与[桌面生命周期核心](../architecture/2026-10-11-desktop-lifecycle-core.md)逐个解释字段标志的约定一致，不改变其反馈规则。

与 [macOS 丢失松键后的 Shift 轻按恢复](2026-10-26-macos-shift-tap-recovery.md) 部分重叠：两者都结束失效的旧手势，但这里处理 Fcitx 输入上下文的生命周期，不共用 macOS 的物理按键查询。桌面生命周期核心、Windows TSF 组合键取消及其他候选／语音状态笔记不改变本修复的修饰键契约。

## Alternatives considered

- 仅删除 Shift 切换对 `ctrl_down_` 的检查：能解除阻塞，但没有结束跨焦点手势，也不能对称恢复 Ctrl 轻按。
- 失焦时直接清空全部修饰状态：足够简单，但会把仍按住的旧手势的松开误认成新的独立轻按。
- 查询全局物理键盘状态：可以额外核对真实按住状态，但当前支持的 Fcitx 版本和前端并非都能提供；本修复使用已有事件和生命周期边界。
- 从 `privateInput()` 中全局移除 `NoSpellCheck`：能一次放开所有本地入口，但也会改变学习及联网边界；这里只收窄中英文切换的两处判断。

## Verification

- 新增 `fcitx5-native-test <verified-resources> --modifier-focus` 及对应 CTest，驱动真实 Fcitx 输入上下文和 Host API，输入与配置均为合成值。原实现失败于「Ctrl 松开丢失后，重新聚焦的 Shift 应切到英文」，补丁后通过。
- 回归覆盖左右 Ctrl 丢失松开、旧手势跨焦点的松开、仅松开前端、仍带 Ctrl 的 Shift 松开、reset、输入法停用，以及开启单按 Ctrl 时的对称恢复。
- `fcitx5-mode-field-hints` 覆盖普通不检查拼写字段、URL 字段、左右 Shift、启用单按 Ctrl、学习及联网保持关闭、敏感／密码／禁用／数字字段限制，以及回到普通字段。0.14.0 回移夹具在修复前准确失败于「不检查拼写的字段应允许 Shift 切到英文」，修复后通过；贡献分支及 0.14.0 回移版的字段回归与前次跨焦点回归均通过。
- Linux 原生目标完整构建通过；Fcitx 隔离 daemon 前端及在线候选用例通过。
- 历史带词库全套中的 `fcitx5-native-context`、`fcitx5-native-ai` 直角引号外观断言，在未修改的同一上游源码构建中也同样失败；本次未重跑该全套，不将专项通过视为全套通过。
- 默认字体环境的 quick 仍在 `linux-candidate-native-canvas` 与 `linux-candidate-native-canvas-chrome` 的像素断言失败。仅给测试进程提供独立 Fontconfig（DejaVu Sans、灰度渲染、不载入用户字体）后，两项均通过，106 项无词库原生套件全部通过；未修改系统字体、绘图代码、测试断言或失败基线。
- 0.14.0 单插件回移版安装并重启后，已核对进程实际加载新插件且核心库未变；Nautilus 地址栏左右 Shift 双向切换及窗口往返经实际使用确认正常。

## Consequences

失焦后不再依赖另一窗口是否回送 Ctrl 松开；普通新手势可以重新切换中英文，观察到的旧手势则保持取消。仅松开前端无法证明不可见的物理按下属于哪一轮，因此保留现有兼容路径，并用原始修饰位避免把仍有其他修饰键的松开当成独立轻按。普通不检查拼写字段可以切换模式，隐私限制仍保留。自动化使用隔离输入上下文与合成数据，真实桌面验收针对 0.14.0 回移版，不将短期通过视为所有应用场景已覆盖。
