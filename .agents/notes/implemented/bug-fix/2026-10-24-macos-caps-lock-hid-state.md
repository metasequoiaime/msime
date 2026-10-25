# Agent Note: 用 HID 实体事件补足 Caps Lock 按住状态

Status: implemented

## Problem

#9094 的外接蓝牙键盘在 macOS 26.4.1 上报告 `NX_ALPHASHIFT_STATELESS_MASK` 始终为零，`0x39` 与 `0xff` 事件交替，系统锁定位约半秒后仍会被改写。现有控制器只能看到逻辑锁定状态，因而无法可靠区分轻点和长按；仅靠 `CGEventSourceFlagsState` 会把短按误判成长按，保留大写锁定。

## Decision

在 macOS 输入法进程中建立只读 `IOHIDManager` 监听，只匹配键盘页的 `Caps Lock`（`0x39`）和 locking Caps Lock（`0x82`）输入元素。输入值回调按设备记录当前按住状态，设备移除时清除记录；读取实体状态时优先使用 HID 结果，管理器无法打开或尚未得到结果时回退到 `NX_ALPHASHIFT_STATELESS_MASK`。监听只接入主运行循环，不抢占设备、不写入 HID 状态，也不把系统键码或逻辑锁定位当作实体松开证据。

监听不是一开始就建：只有某次按下 Caps Lock 时，事件和 `kCGEventSourceStateHIDSystemState` 都没报实体按住位、HID 也还没有结果，`observeCapsLockTap` 才调 `MSIMEStartCapsLockHIDTracking`。报这个位的键盘（Apple 内置键盘就是）在按下时走不到这一步，从不打开 HID 管理器。打开键盘的 HID 管理器要「输入监控」授权：`IOHIDCheckAccess(kIOHIDRequestTypeListenEvent)` 为未询问时，在后台队列调一次 `IOHIDRequestAccess` 请系统询问，授权前不调 `IOHIDManagerOpen`；拒绝时保持回退。没打开不记为最终结果，下一次同样的按键再检查，用户之后在系统设置里允许也不必重启输入法。这一次按键本身照常保留大写锁定，HID 结果出来之后的轻点才能切换。设置页的 Caps Lock 说明（共享页 `capsLockLanguageSwitchDescription` 与原生 `AppearancePreferences.mm`）写明这类键盘会请求授权。

为保持单测不触碰真实键盘，抽出带 `known` 标记的 HID 状态函数指针和启动监听的 `MSIMECapsLockHIDStart` 函数指针；测试替换该边界并继续验证生产的 Caps Lock 轻点判定。管理器初始化失败、权限不足、设备没有匹配元素和回调丢失都安全回退为保留系统大写锁定。

## Alternatives considered

- **继续只读 `NX_ALPHASHIFT_STATELESS_MASK`**：实现简单且没有新的监听边界，但报告设备已经证明该位不能作为外接键盘的实体状态来源，短按仍会被误判。
- **继续使用键码 `0x39` 或逻辑锁定位**：能在部分键盘上得到值，但两者都随锁定状态保持，无法证明实体松开，会重新引入已记录的缺陷。
- **第一次读取实体状态时就打开 HID 管理器**（#9495 合入时的做法，`dispatch_once` 包住）：实现最简单，监听越早建，第一次轻点越可能已经有结果。但轮询每 10 毫秒都会读状态，所以每个开了 Caps Lock 切换的用户都会走到 `IOHIDManagerOpen`，包括本来不需要 HID 的内置键盘用户；授权未询问时系统会在用户毫无准备时以输入法的名义要求「接收所有应用的按键」，拒绝或失败后 `dispatch_once` 又让它到输入法重启前都不再尝试。
- **在设置页放一个「授予输入监控」按钮**：授权时机由用户掌握，说明文字也能放在旁边。但 macOS 的设置页由 Tauri 设置应用拉起，在另一个进程里申请，授权记在设置应用的 bundle 上，输入法进程照样没有；要让输入法申请，还得为一次按钮点击另加跨进程通道。在用户正在用 Caps Lock 切换、而键盘确实不报按住位的那一刻申请，上下文同样清楚。
- **按事件立即切换并撤回锁定**：不需要读取松开状态；但长按 Caps Lock 会失去保留大写锁定的行为，且撤回失败时会留下错误的英文模式，不符合既有契约。

## Verification

- `InputController.mm` 与 `ShortcutTest.mm` 使用 Xcode 27 macOS SDK 通过 `clang++ -fsyntax-only`；`git diff --check` 通过。
- 单测边界新增 HID 状态已知且分别为按下、松开，以及未知时回退 stateless 位；`shortcut-test --caps-lock` 还断言报实体按住位的按键从不启动 HID 监听、不报的那台设备只启动一次，HID 有结果之后同一台设备的轻点照常切换；既有 `shortcut-test --caps-lock` 用例继续替换原生查询边界，不初始化真实 HID 管理器。设备表的匹配、输入值更新和拔出清理由同一实现路径维护，未在没有 HID 设备的本机单独伪造回调。
- 授权询问框、拒绝后的回退和事后在系统设置里允许的路径没有实机验证：本机终端已有输入监控授权，测不出输入法进程未询问时的系统行为。
- 本机没有可用于复现 #9094 的外接蓝牙键盘现场，尚未在报告人的 macOS 26.4.1 环境验证 `IOHIDManager` 回调；不能据此宣称所有外接设备均已修复。

## Consequences

HID 可用时，短按判定读取实体按下和松开，不再依赖外接键盘可能缺失的 Quartz stateless 位；多个键盘并存和设备拔出不会把另一台设备的状态误留在表中。HID 不可用时仍保留原先的安全回退，代价是该设备只能保留系统大写锁定，不能冒险切换。内置键盘的用户不会看到授权询问；不报按住位的键盘第一次轻点会弹出系统的「输入监控」询问，询问那次和授权后第一次按键都只保留大写锁定，之后的轻点才切换。

`IOHIDManagerOpen` 在不同 macOS 版本或权限状态下可能不可用，真实外接键盘验收仍需现场设备；若 HID 回调延迟，短按只能安全地保留大写锁定，不能保证每次都切换。
