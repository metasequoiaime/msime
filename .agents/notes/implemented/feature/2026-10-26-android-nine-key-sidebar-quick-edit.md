# Agent Note: Android 九键左侧符号栏长按快速编辑

Status: implemented

## Problem

#9757：九键左侧符号栏可以点按输入，但用户必须离开键盘、进入设置页才能调整符号顺序和内容。

## Decision

符号栏每个项目支持长按，通过带 `edit_sidebar_key` 参数的 `KEY_LAYOUT_STYLES` 深链打开「按键布局」页并直接进入 `NineKeySidebarSheet`，参数是那张表的共享偏好名（`touch_nine_key_sidebar_symbols` / `touch_nine_key_digit_sidebar_symbols`）；两张表提为共享偏好、入口挪进「按键布局」的经过见[九键左侧符号栏三端共享](2026-10-29-nine-key-sidebar-shared.md)。编辑器沿用既有的默认值、校验、排序、添加、删除和保存流程；数字键盘符号栏与字母键面各是一张表。

## Alternatives considered

- **在输入法进程复制一套编辑面板**：会重复设置页的校验和保存逻辑，并扩大输入法进程的 UI 负担。
- **长按只打开键盘设置首页**：比当前更容易找到入口，但仍需用户再次定位并点击符号栏设置，不能满足快速修改。

## Consequences

长按会收起输入法并打开设置页编辑器，点按输入行为不变。深链参数只控制首次打开的编辑器，返回设置页后仍可正常浏览键盘设置。需要在 Android 设备上确认长按手势、正确的字母/数字符号栏选择和保存后键盘重建。

符号键的读屏描述是 `按键 符号 <符号>；长按快速修改符号栏`：长按提示经 `keyboardKey` 的描述参数传入，保留所有键共用的「按键 」前缀。最初的实现在 `keyboardKey` 之后另调 `setContentDescription` 写成 `符号 <符号>；…`，把前缀覆盖掉了，`NineKeySidebarPairDeviceSmoke` 按描述找不到侧栏键，API 35 设备验收在「prepare nine-key sidebar」失败（ci-platforms run 40356646105）。

## Verification

待运行 Android host、quick 门禁、笔记校验和模拟器触摸验收。
