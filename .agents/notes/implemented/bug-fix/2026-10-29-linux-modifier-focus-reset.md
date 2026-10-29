# Agent Note: Fcitx 失焦后结束修饰键轻按手势

Status: implemented

## Problem

Fcitx 输入上下文可能在收到 Ctrl 按下后失焦，而松开事件留在其他窗口。宿主保留 `ctrl_down_`，重新聚焦后又用这个旧值阻止单按 Shift 切换中英文。普通按键事件已经不带 Ctrl，重复轻按 Shift 仍无法恢复；单独按下并松开 Ctrl 可以清除残留状态。

## Decision

`FcitxState::cancelModifierTaps()` 在失焦、关闭和重置时结束当前修饰键手势，清除按住状态、布防和计时。已经观察到的未完成按键保留为取消标记，随后迟到的松开不构成新手势；新的完整按下／松开仍可重新布防。判断另一修饰键是否仍按住时同时核对原始事件的修饰位，覆盖重新聚焦后只收到松开的前端。

改动只属于 Linux Fcitx 适配层，不改变切换开关、长按阈值、输入模式记忆或其他平台。

与 [macOS 丢失松键后的 Shift 轻按恢复](2026-10-26-macos-shift-tap-recovery.md) 部分重叠：两者都结束失效的旧手势，但这里处理 Fcitx 输入上下文的生命周期，不共用 macOS 的物理按键查询。桌面生命周期核心、Windows TSF 组合键取消及其他候选／语音状态笔记不改变本修复的修饰键契约。

## Alternatives considered

- 仅删除 Shift 切换对 `ctrl_down_` 的检查：能解除阻塞，但没有结束跨焦点手势，也不能对称恢复 Ctrl 轻按。
- 失焦时直接清空全部修饰状态：足够简单，但会把仍按住的旧手势的松开误认成新的独立轻按。
- 查询全局物理键盘状态：可以额外核对真实按住状态，但当前支持的 Fcitx 版本和前端并非都能提供；本修复使用已有事件和生命周期边界。

## Verification

- 新增 `fcitx5-native-test <verified-resources> --modifier-focus` 及对应 CTest，驱动真实 Fcitx 输入上下文和 Host API，输入与配置均为合成值。原实现失败于「Ctrl 松开丢失后，重新聚焦的 Shift 应切到英文」，补丁后通过。
- 回归覆盖左右 Ctrl 丢失松开、旧手势跨焦点的松开、仅松开前端、仍带 Ctrl 的 Shift 松开、reset、输入法停用，以及开启单按 Ctrl 时的对称恢复。
- Linux 原生目标完整构建通过；Fcitx 隔离 daemon 前端及在线候选用例通过。
- 带已校验词库的原生套件有 4 个未通过用例：`fcitx5-native-context`、`fcitx5-native-ai` 的直角引号外观断言，在未修改的同一上游源码构建中也同样失败；`linux-candidate-native-canvas` 与 `linux-candidate-native-canvas-chrome` 的像素断言来自未修改的绘图源码。没有修改这些断言或失败基线。
- 字体环境复核：当前 Fontconfig 对 Sans 返回 RGB 子像素渲染；仅在独立测试进程中使用灰度渲染后，`linux-candidate-native-canvas` 通过。`linux-candidate-native-canvas-chrome` 的灰色像素断言仍失败，原因待确认；没有修改系统字体配置。
- 本地 quick 已执行但未全绿；桌面外壳的独立编译复核通过，无词库的 Linux 原生阶段为 104／106 项通过，剩余为上述两个绘图断言。核心修饰键回归独立运行通过，已安装输入法尚未验收。

## Consequences

失焦后不再依赖另一窗口是否回送 Ctrl 松开；普通新手势可以重新切换中英文，观察到的旧手势则保持取消。仅松开前端无法证明不可见的物理按下属于哪一轮，因此保留现有兼容路径，并用原始修饰位避免把仍有其他修饰键的松开当成独立轻按。自动化使用隔离输入上下文与合成数据，尚未替换已安装插件，不将隔离验证等同于已安装输入法的真实验收。
