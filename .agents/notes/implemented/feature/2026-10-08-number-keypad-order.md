# Agent Note: 九宫格数字层的计算器排列

Status: implemented

## Problem

用户建议数字键盘能选成像计算器那样排列（7 8 9 在上）。三端都没有专门给数字输入框用的数字键盘，数字只出现在两处：26 键的「123」页是一排 1 到 0，没有上下之分；九宫格的数字层是 3×3，1 2 3 在最上面。所以「计算器排列」实际指的是九宫格数字层。

## Decision

- 新增共享偏好 `touch_number_keypad_order`，取值 `phone`（默认，1 2 3 在上）或 `calculator`（7 8 9 在上，1 2 3 在下），定义在 client-core 的 `NumberKeypadOrder`。桌面宿主原样保留、不使用（`preference_coverage.py` 记为不适用）。
- 只改九宫格数字层，字母层永远是 1 2 3 在上：Android `NineKeyLayout.rows(digits, calculator)`，iOS 数字层的位置到数字的映射，HarmonyOS `NineKeyLayout.digits(order)`。按下的输入和无障碍文本跟着显示的数字走。
- 设置入口：共享设置页「屏幕键盘 › 布局」在移动宿主上显示「数字键盘顺序」；Android 的键盘设置页和 iOS 的键盘布局设置页各加一行。
- 账号同步：Android 和 iOS 走 client-core 的 `platform.android.number_keypad_order`，HarmonyOS 按它自己的命名空间用 `platform.harmony.number_keypad_order`。服务端字段表还没有这两个键，上传时会被按字段表过滤掉，所以目前只存在本机，等服务端登记后自动开始同步。

## Alternatives considered

- **存成宿主本机设置（Android 的 `AndroidLocalSettings`、iOS 的 App Group）**：不必改 client-core。但三端各存一份，共享设置页（HarmonyOS 用的就是它）读不到，以后同步也要三端各接一次；它和 `touch_voice_shortcut` 一样是触屏键盘的共享偏好，放在共享文档里最自然。
- **26 键的「123」页也做成计算器样的 3×3 小键盘**：这已经是另一种键盘布局，不是排列顺序的选择，用户提的也不是这个。

## Consequences

- **收益**：习惯计算器小键盘的用户可以在九宫格上用同样的手感输数字。
- **代价**：多一个偏好要在三端和共享设置页维护。HarmonyOS 的打字统计在计算器排列下仍按数字记键（左上角的 7 记成 `Nine7`），不按位置。
- **待办**：服务端账号字段表登记上面两个键之后才会跨设备同步。

## Verification

- `cargo test -p msime-client-core`（旧文档缺省为 `phone`、保存 `calculator`、同步导出与导入）。
- Android `NineKeyLayoutSmoke`，iOS `NineKeyKeyboardTests` 与 `KeyboardGeometrySettingsTests`，HarmonyOS `keyboard-logic.test.ts`（计算器排列是 `789456123`），apps/desktop `touch-keyboard-geometry-section.test.tsx`。
