# Agent Note: iOS 双击品牌键看键盘内存读数

Status: implemented

## Problem

键盘扩展被 iOS 按进程内存上限杀掉时，「键盘突然消失」在用户那里完全无声。仓库里已经有观测手段（`DiagnosticLog.MemorySample`：`phys_footprint` + `os_proc_available_memory()`，附在 `keyboard_loaded`、`keyboard_built`、`keyboard_shown`、`focus_in`、`preferences_applied`、`memory_warning`、`memory_released`、`engine_cache_reset` 这几行后面），但那条链路要三样东西同时成立：诊断日志开着（共享偏好的 `diagnostic_log.server`）、键盘有完全访问（写 App Group）、App 能读到同一个 App Group。

未签名的测试包（Sideloadly / AltStore 重签）没有 App Group entitlement，第二、三条都不成立：键盘写的和 App 读的不是同一个目录，开关本身也是共享偏好。设备上也没有 Instruments——真机验收的人在 Windows 上，只有 `idevicesyslog` 之类的手段。所以「现在这个键盘占多少内存」在实际测试里拿不到。

## Decision

- **双击品牌键**（工具栏左端 logo）在键盘上弹一个读数气泡，一行写「占用 41.2 MiB · 余量 12.0 MiB · 峰值 48.0 MiB」。不读任何共享偏好，也不写文件，因此未签名包同样能用。
- 气泡 `KeyboardMemoryBubble` 盖住整块键盘，接住「点外面关掉」的那一下（那一击不落到下面的按键上）；3 秒自动淡出；重复关闭只回调一次。键盘收起时（`viewWillDisappear`）一并收掉。
- 数据仍是 `DiagnosticLog.MemorySample`，新增 `peak` 字段取 `task_vm_info.ledger_phys_footprint_peak`（启动以来的物理占用高水位）。键盘进程可以活好几天，内存警告前后的高水位比当前值更能说明问题。**日志行里不写峰值**：`fields` 保持 `mem=`、`avail=` 两项，老日志的解析和 `KeyboardMemoryTests` 里的字段断言都不用改。
- 显示取一位小数（`%.1f`），日志仍取整到 MiB：气泡是给人看的，日志要保持紧凑和可比较。
- `available` 为 0 时写「余量未知」而不是「余量 0」：按 `os/proc.h`，进程「不是 App」、已经超限、以及模拟器上都返回 0，这三件事分不开，写成 0 会把人吓一跳。日志里对应写 `avail=-`。
- 峰值取 `max(peak, footprint)`：系统取不到峰值（`task_info` 失败）时为 0，不能让它显示得比当前占用还小。
- **单击不加延时**：功能菜单是高频操作，为了等双击判定把它拖慢两百多毫秒不值得。代价是双击时菜单会先闪一下——第一下的 `primaryActionTriggered` 已经把菜单打开，第二下的双击识别器把它收掉再弹气泡。`brandDoubleTapConsumedTap` 处理「手势先认出双击、按钮动作后到」的顺序：那一次动作被吞掉，菜单不会在气泡底下重新打开。
- 长按品牌键不变，仍是打开 App 的「键盘工具栏」设置页。
- 读屏：气泡出现时 `UIAccessibility.post(.announcement)` 念同一行文字；气泡只停 3 秒，等读屏用户自己摸过去就没了。

## Alternatives considered

- **长按品牌键**：已被「编辑工具栏」占用，且与工具栏其他按钮同一条路径（`handleToolbarEditorHold`），换掉会影响一致性。
- **双击候选栏空白**：也能用，但候选栏在组字时被读音行和候选占满，空白不一定存在；品牌键始终在。
- **双击并延迟单击**：手势语义最干净，但 `moreShortcut` 在测试里被按了十几处、都要求立刻出菜单，而且给高频操作加 0.24 秒延迟是实打实的手感退化。改成「第一下照开菜单、第二下收掉换气泡」，测试和手感都不动。
- **在 App 里加一个内存页**：读不到键盘进程的内存——那是另一个进程，且需要 App Group 才能传值。
- **把读数写进诊断日志、让 App 显示**：未签名包没有 App Group，这条链路断在写文件那一步。
- **一直显示在候选栏角落**：占位置，而且每次重画都要采样；调试信息默认不出现更合适。

## 验证

- `KeyboardTests/settings/KeyboardMemoryTests.swift`：气泡文字三个读数（含余量未知、峰值取不到、`mebibytes` 取整）、3 秒自动消失与点外面消失各一次回调、品牌键上挂着 `brandDoubleTap` 且 `toolbarEditorHold` 还在。
- 本机没有 Xcode，Swift 未在本地编译；靠 `ios-*` 分支的 fork CI 编译并跑 XCTest。
