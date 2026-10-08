# Agent Note: Android 实体键盘打字时收起成候选条

Status: implemented

## Problem

#5584：小米平板接官方键盘打字时，水杉的软键盘照样占着屏幕下半，候选也没有序号；同平台其他输入法这时只留一条带序号的候选栏。

宿主原来完全没有处理这件事：`onEvaluateInputViewShown` 沿用系统默认，系统在「使用实体键盘时显示虚拟键盘」打开时显示整副软键盘，关着时则整个输入法窗口都不显示——组词照常进行，但候选无处可看，只能靠空格盲选。候选 chip 按 Apple 的样式从不画序号，数字行选词（`number_row_selection`）选的是第几个只能靠数。

## Decision

- 纯判断放在 `policy/HardwareKeyboardModePolicy`，服务只接线。模式由 `MSIMEInputService.hardwareKeyboardMode` 持有，跨输入框保留。
- 进入：`onKeyDown` 收到一次来自实体键盘的按下（有设备、`InputDevice.isVirtual()` 为假、键盘类型是字母型）。另外，系统因实体键盘而不显示窗口（`super.onEvaluateInputViewShown()` 为假且配置报告接着键盘）时，走引擎的输入框仍允许显示窗口，并直接进入这个模式。
- 退出：候选条上的展开键（工具栏最右的收起键，这时 chevron 朝上，描述「显示软键盘」），或配置报告键盘从有到无（`afterConfiguration`）。
- 展开键的选择要粘住：系统每次 `showSoftInput`（点已聚焦的输入框挪光标、换输入框）都会重新调 `onEvaluateInputViewShown`，系统开关关着时上一条「直接进入」规则会立刻把刚展开的键盘收回去。所以点展开键同时记下 `hardwareKeyboardUserExpanded`，`afterSystemDecision` 见到它就不再替用户收起；它只在同一次接着期间有效，配置报告键盘从有到无或从无到有时清掉（`userExpandedAfterConfiguration`）。实体键盘再打字照常重新收起。
- 模式开着且没有要占键区高度的面板（工具栏面板、展开候选、内联高度条、手写）时，`ImeFrame.setKeysCollapsed` 收起键行、功能行和单手侧栏所在的整行，只留顶部一行；空闲时工具栏总显示，即使用户设了「显示方式：隐藏」。面板打开时临时展开，关上后回到候选条。
- 候选前面标「N 」（N 为 1–9）：模式开着、`number_row_selection` 开着、不是英文直输时。序号用候选样式已有的序号色（`candidateAppearance.number()`）。`candidateLabel` 带序号而没有释义时不再追加空的第二行。
- 窗口被系统收着时，实体键盘打出组词后 `requestShowSelf(0)` 把候选条叫出来；这是显式请求，系统的默认 `onShowInputRequested` 会放行。

## Alternatives considered

- **按 `Configuration.keyboard` 一接上就收起**：最贴近「接入外置键盘时隐藏」的字面说法，不用等第一次按键。但有的平板常驻报告 QWERTY，打开了系统「显示虚拟键盘」的用户也明确要软键盘，接上就收起会替他们做反向的决定；第一次实体按键是更可靠的信号，代价只是第一次按键前软键盘还在。
- **用系统的候选视图（`onCreateCandidatesView` / `setCandidatesViewShown`）**：框架原生的「只显示候选」位置。但宿主的候选、读音行、工具栏都长在输入视图里，另起一套候选视图要把候选渲染复制一份，皮肤、释义、翻页都得再接一遍。
- **读系统设置 `show_ime_with_hard_keyboard` 判断用户意图**：能精确区分两类用户；但它不是公开 SDK 常量，新版本对非 SDK 设置项的读取有限制，系统默认实现已经用它算出了 `onEvaluateInputViewShown` 的结果，直接用那个结果即可。

## Consequences

- 收益：实体键盘打字时屏幕下半让给应用，候选带序号，与数字行选词一一对应；系统不显示虚拟键盘时也看得到候选。
- 代价：第一次实体按键之前软键盘仍在；配置从不报告键盘的设备拔掉键盘后，模式要等用户点展开键才退出。候选条模式不浮动，与浮动键盘同时打开时候选条在底部。
- 验证：`tests/settings/HardwareKeyboardModePolicySmoke.java` 钉住连接判断、实体按键判断、配置变化、窗口显示、展开键粘住、收起和序号规则，由 `platforms/android/check-host.sh` 运行。真机（小米平板 + 官方键盘、蓝牙键盘）上的收起、展开和 `requestShowSelf` 没有在设备上验收。
