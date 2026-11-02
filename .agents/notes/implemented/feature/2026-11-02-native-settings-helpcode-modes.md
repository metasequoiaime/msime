# Agent Note: 原生设置补上双拼辅助码触发方式

Status: implemented

## Problem

#10624 报告 Windows 和 Android 的设置里没有双拼辅助码的触发方式。macOS 有这些开关：末尾直接、末尾间接（分号、反引号、大写字母）、句中间接和它的三枚触发键、逐音节直接和它的两种结束方式。

偏好和引擎早已平台无关。client-core 的 `ShuangpinHelpcodeModes` 持有十一个字段、出厂值和写入端的 `normalized()`，host-api 的 `shuangpin_helpcode_modes()` 把它们交给引擎，只有 `manual_tab` 用 cfg 锁在 macOS，[触发方式全部接入引擎](2026-10-22-shuangpin-helpcode-trigger-modes.md)。缺的只是界面。共享 React 设置页的 `modesRows` 只到得了 macOS、Linux 和 HarmonyOS，而 Windows（WinUI 和 Win7）、iOS、Android 发布包都用原生设置（docs/ARCHITECTURE.md「设置界面按平台归属」）。这三端没有任何 `modes` 控件，Windows 和 iOS 连「一码严格筛选」（`strict_filter`）也没有。报告里没提 iOS，但它的缺口完全相同。

## Decision

在 Windows WinUI、Win7、Android 和 iOS 的原生设置里补上同样的开关，不改任何出厂值，也不改缺字段时的读法。

- **各端共用同一张表**：字段、顺序、标题和「?」里的说明与共享页 `modesRows` 逐字相同。每端的表放在一处，并由该端的测试核对：Windows 是 `platforms/windows/settings/HelpcodeModesModel.h`（WinUI 和 Win7 共用，`tests/ui/helpcode_modes_model.cpp`），Android 是 `HelpcodeModesPolicy`（`HelpcodeModesPolicySmoke`），iOS 是 `HelpcodeSettingsView` 的扩展（`KeyboardTests/settings/HelpcodeModesTests.swift`）。
- **缺值读法**：每一行缺字段时的值等于 `ShuangpinHelpcodeModes::default()`：`direct`、`trigger_semicolon`、`mid_sentence_backtick`、`direct_helpcode_slash` 为开，其余为关。这一段等于出厂值时 client-core 不写进文档，所以老文档里通常根本没有 `modes`。`strict_filter` 缺字段时为关。三端的测试都把这几个值钉住。
- **置灰规则与共享页相同**：双拼辅助码关着时，触发方式全部置灰（Android 在辅助码关着时整组不画）。句中的三枚触发键还要「句中间接辅助码」开着，两种结束方式还要「逐音节直接辅助码」开着。「一码严格筛选」随这一族的辅助码开关置灰。
- **互斥只在 client-core**：句中间接开着时同一枚触发键归句中，句中间接和逐音节直接不同时开。这两条由 client-core 在写入端归一。原生页面不复制这条规则，改动后整页重读并显示归一后的值：WinUI 的 `bool_row` 传 `redraw=true`，Win7 在保存完成后刷新表单，Android 的 `savePreferences` 会重读，iOS 的绑定在保存后调用 `reload()`。
- **写入只改一个字段**：`modes` 里的其他键原样保留，包括从 macOS 同步来的 `manual_tab`。WinUI 和 Android 写到 `shuangpin_helpcode.modes.<字段>`，缺的中间层由各自的写入工具补上。Win7 的 `PreferencesStore` 不会补造中间对象，所以 Win7 在读到的 `modes` 对象上改好这一项，再把整段写回；这条路径和两族的 `strict_filter` 都登记在它的可选路径白名单里。iOS 经 `merge` 写回 `modes` 对象。
- **布局**：WinUI 照共享页的桌面形态，只在当前方案是双拼时把触发方式画在主流程里，「更多选项」里的另一族不带这几行。「一码严格筛选」两族都有。Win7 的两族平铺，触发方式接在双拼一族后面。Win7 的控件 ID 仍从 `helpcode_first`（980）起：原来的六个不变，后面是两族的严格筛选，再后面是十一种触发方式，共 19 个，用 `static_assert` 保证不越过 1000。Android 只画当前方案那一族，双拼时接在严格筛选之后。iOS 的两族各占一个分区，触发方式在双拼分区里。
- **不画的两项**：「手动辅助码（Tab）」（`manual_tab`）和辅助码前导键（`trigger_key`）都由宿主限定在 macOS（前导键还包括 Linux 和 HarmonyOS），这次照样不画。
- **触屏的限制写进说明**：Android 和 iOS 的触屏键盘在中文方案下会把分号、反引号和 `/` 直接上屏。Android 的数字九宫格见 `NumberPadPunctuation.typesThroughSession`。iOS 的数字九宫格只在本地模式和越南文、藏文里把这些键交给会话，#+= 层没有这三个键。所以末尾分号和末尾反引号两种触发、句中的反引号和分号两枚触发键，以及 `/` 结束，在触屏上都按不到，只能用外接键盘。这些键从外接键盘经 `punctuation` 进入引擎，runtime 会先查 `spelling_symbols`。这五行的「?」说明里写明要用外接键盘。大写字母和直接接码在触屏上照常可用。

HarmonyOS 手机用共享页，`mobile` 分支已经在「更多选项」里画出 `modesRows`，这次不用改。

## 出厂值差异表

这次只补控件，没有任何出厂值需要对齐。四端的缺值读法与 client-core 完全相同，没有差异要记录。

| 字段 | client-core 出厂值 | 各原生端的缺值读法 |
| --- | --- | --- |
| `direct` / `trigger_semicolon` / `mid_sentence_backtick` / `direct_helpcode_slash` | 开 | 开 |
| 其余七项 | 关 | 关 |
| `strict_filter`（两族） | 关 | 关 |

## Alternatives considered

- **Android、iOS 只露出触屏上按得到的开关**（末尾直接、大写字母触发、逐音节直接和大写结束）。理由最强的一点是不做「看得见却按不出来」的开关，#9265 和 #10431 都回避过这种情况。不采用的原因有两个。第一，外接键盘上这些触发键确实有效，藏起来会让外接键盘用户无从设置。第二，同一份偏好会随账号从桌面同步过来：要是在移动端看不到一项已经打开的触发方式，就没法关掉它。所以开关照常显示，限制写进「?」说明。
- **这次就把 Android、iOS 数字九宫格接上 `spelling_symbols` 触发键**，像鸿蒙 #10431 的 `layerTrigger` 那样。这是键盘交互的改动，要三端一起改，还要在设备上验收，超出 #10624 补设置项的范围，另开 issue 处理。
- **WinUI 在 `helpcode_rows` 里两族都画触发方式**。这样实现最省事，但全拼方案下「更多选项」会出现只对双拼生效的十一行，和共享页的桌面形态不一致，所以不采用。
- **Win7 改成逐字段写 `shuangpin_helpcode.modes.<字段>`**。这需要让 `PreferencesStore` 允许补造中间对象，会放宽「未知路径和缺失的中间对象一律不能补造」这条守卫，所以改成整段写回，只在白名单里登记 `modes` 这一个对象。

## Consequences

- 在 Windows、Android 和 iOS 的设置里都能打开或关闭每一种双拼辅助码触发方式和一码严格筛选，行为由已有的引擎实现决定。Windows 的按键路由已经会把引擎发布的 `` ` ; / `` 当字符交给会话（`EditPolicy.h` 的 `spelled_by_engine`），打开开关就能用。
- 以后共享页的 `modesRows` 改了文案或顺序，三端的表要跟着改。各端测试钉的是文案和顺序，不会自动对齐，需要人工同步。
- 加了新的触发方式字段时，四个地方都要补：client-core 的 `Default`、共享页，以及三端的表。三端测试会在字段数不对时变红。

## Evidence

- `tests/ui/helpcode_modes_model.cpp`：头文件补上之前编译失败，补上之后用 clang++ `-std=c++17 -Wall -Wextra -Wshadow -Werror` 编译运行通过，`x86_64-w64-mingw32-g++ -fsyntax-only` 通过。
- Win7 `HelpcodeForm.h`、`AdvancedInputForm.h`、`PreferencesStore.h` 和 `tests/ui/windows7_settings.cpp` 用 MinGW 做语法检查通过。`windows7-settings` 只在 CI 的 Windows runner 上运行，新增的断言覆盖缺字段时的显示、只写一项，以及打开句中间接后末尾反引号被归一清掉。
- Android：`HelpcodeModesPolicySmoke` 在策略类存在之前编译失败，之后通过；`SettingsSearchIndexSmoke` 按策略表核对设置搜索的关键词。
- iOS：`HelpcodeModesTests` 覆盖表、缺值读法、置灰、`merge` 保留其他键，以及经 client-core 保存后读回归一后的值。

## Existing notes

- [双拼辅助码补齐直接、间接与句中触发方式](2026-10-21-shuangpin-helpcode-trigger-modes.md)持有偏好结构、出厂值和互斥规则。本篇只补原生设置入口，它末尾「原生 Android、iOS、Windows 尚未增加设置入口」那句改为指向本篇。
- [触发方式全部接入引擎](2026-10-22-shuangpin-helpcode-trigger-modes.md)和[双拼手动辅助码](2026-10-23-shuangpin-manual-tab-helpcode.md)与本篇部分重叠，`manual_tab` 仍只在 macOS 生效。
- [鸿蒙辅助码入口](../bug-fix/2026-10-31-harmony-helpcode-entry.md)记录了 Android、iOS 触屏符号层没有接 `layerTrigger`，本篇的触屏限制说明以它为依据。
