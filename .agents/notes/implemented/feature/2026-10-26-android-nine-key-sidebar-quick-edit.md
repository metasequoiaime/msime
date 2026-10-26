# Agent Note: Android 九键左侧符号栏长按快速编辑

Status: implemented

## Problem

#9757：九键左侧符号栏可以点按输入，但用户必须离开键盘、进入设置页才能调整符号顺序和内容。

## Decision

符号栏每个项目支持长按，通过带 `edit_sidebar_key` 参数的 `KEYBOARD_OPTIONS` 深链打开现有 `NineKeySidebarSheet`。编辑器继续使用 Android 本地设置的既有默认值、校验、排序、添加、删除和保存流程；数字键盘符号栏与字母键面使用各自的设置键。

## Alternatives considered

- **在输入法进程复制一套编辑面板**：会重复设置页的校验和保存逻辑，并扩大输入法进程的 UI 负担。
- **长按只打开键盘设置首页**：比当前更容易找到入口，但仍需用户再次定位并点击符号栏设置，不能满足快速修改。

## Consequences

长按会收起输入法并打开设置页编辑器，点按输入行为不变。深链参数只控制首次打开的编辑器，返回设置页后仍可正常浏览键盘设置。需要在 Android 设备上确认长按手势、正确的字母/数字符号栏选择和保存后键盘重建。

## Verification

待运行 Android host、quick 门禁、笔记校验和模拟器触摸验收。
