# Agent Note: macOS 托管英文长按变音符号

Status: implemented

## Problem

开启 `english_online_candidates` 后，macOS 英文模式的字母由 Engine 托管组字。长按字母产生的重复 `keyDown` 会继续被输入法消费，Cocoa 因而没有机会显示系统的变音符号菜单；普通英文模式未托管组字时没有这个缺口。

## Decision

在 `MSIMEInputController` 的托管英文路径中，只对无 Command、Control、Option 修饰键的 ASCII 英文字母重复按键生效。输入法先用 `commitManagedEnglishRaw` 上屏当前原始字母，再返回 `NO`，让 `handleEvent:` 的未消费事件交给 Cocoa。单次字母、非 ASCII 字符、数字、结束键和带竞争修饰键的快捷键继续沿用原有 Engine 路径。

Apple InputMethodKit 的事件契约以 `handleEvent:` 返回值区分归属：返回 `NO` 表示输入法未消费事件，Cocoa 可以继续处理。回归测试通过真实会话建立托管英文组字，验证重复字母返回 `NO`、原始字母已经提交且不再保留 marked text。

## Alternatives considered

- 在输入法内自行绘制或模拟变音符号候选：需要复制系统键盘布局、菜单定位和选择行为，无法保持 Cocoa 的原生体验，因此不采用。
- 所有托管英文按键都在重复事件时让出：会把数字、方向键、结束键和快捷键的既有输入法行为一起改变，边界过宽，因此只放行 ASCII 字母且排除竞争修饰键。

## Consequences

长按英文字符时，原始字母会先提交，系统随后可以显示 macOS 原生变音符号菜单；没有发生长按或字符不满足边界时，英文在线候选行为不变。测试覆盖 `ShortcutTest` 的真实会话路径；本地 macOS CTest 的 `shortcut` 用例通过。真实编辑器中的报告者验收仍需后续补做。
