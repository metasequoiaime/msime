# Agent Note: macOS 字母快捷键按当前键盘布局认键

Status: implemented

## Problem

#5552 报告开了简繁切换快捷键后 Control+Shift+F 不切换繁体，Option+Shift+H 也不切换全半角，Shift 切中英正常。维护者在美式布局、中文状态下用合成按键测试两个快捷键都能切换，剩下没排除的是键盘布局。

`InputController.mm` 的 `handleKeyEvent:client:` 用美式布局的物理 keyCode 认这些字母快捷键：Control+Shift+F 是 `keyCode == 3`，Control+Shift+E 是 `14`，Control+Shift+Command+K 是 `40`，`FullWidthInput.h` 的 `IsFullWidthInputToggle` 认 `kVK_ANSI_H`，维护快捷键 `PhysicalMaintenanceShortcut` 认 C/R/T 的 `8/15/17`。Dvorak、Colemak 等布局把字母放在别的物理位置上，用户按标着 F 的键打出的是 f，keyCode 却不是 3，快捷键就不响应；反过来物理 F 键（Dvorak 上打出 u）会触发简繁切换。同一个宿主组字时按布局字符取字母，所以只有快捷键对不上布局。Windows 宿主用 `KeyEventSink.cpp` 的虚拟键码匹配 Control+Shift+F，Windows 的字母虚拟键码随键盘布局变化，Dvorak 上本来就是标着 F 的键。

## Decision

字母快捷键按当前键盘布局在这个键上打出的字母认键。`InputControllerPhysicalKeys.h` 的 `ShortcutLetter(keyCode, layoutCharacter)` 是唯一规则，`InputController.mm` 的 `MSIMEShortcutLetter` 把事件的 `charactersIgnoringModifiers` 交给它：

- `charactersIgnoringModifiers` 只保留 Shift 的作用，Control、Option 不会把它变成控制字符或 Option 字符，大小写一律折成小写，所以 Caps Lock 和 Shift 不影响匹配。
- 布局在这个键上给的是 ASCII 标点或数字（Dvorak 的物理 E 键打出 `.`）时，它不是任何字母快捷键，不退回物理位置，免得同一个快捷键有两个键能触发。
- 只有拿不到 ASCII 字符时才退回美式布局的物理位置（`PhysicalAnsiLetter`）：没有字符的合成事件，或者布局在这个键上给的是非拉丁字母。输入法总是配一个 ASCII 可用的布局，后者在真实输入里基本不出现。

Control+Shift+F、Control+Shift+E、Option+Shift+H、Control+Shift+Command+K 和 Control+Shift+Option+C/R/T 都改用这个字母。`PhysicalMaintenanceShortcut` 改名为 `MaintenanceShortcut` 并改收字母，`IsFullWidthInputToggle` 多收一个字母参数，Control+Shift+Space 仍按空格的 keyCode 认。美式布局上字符和物理位置一致，行为不变。

不改的部分：Control+. 标点切换、候选翻页的 `-`/`=`/`,`/`.`/`[`/`]`、数字选词和 Control+Shift+Option+数字删词仍按物理位置认，它们是标点和数字键，与本问题无关，按物理位置认是这些路径各自的既有决定（见 `InputControllerPhysicalKeys.h` 的注释）。

## Alternatives considered

- 用 `TISCopyCurrentKeyboardLayoutInputSource` 加 `UCKeyTranslate` 把 keyCode 按当前布局翻译成不带修饰键的字符。它最强的理由是完全不依赖事件里的字符，合成事件也能按布局翻译。不用它是因为那要在每次按键时另查一次输入源、自己管死键状态，而事件自带的 `charactersIgnoringModifiers` 已经是 AppKit 按这次按键的布局算好的，组字取字母和 `AppearancePreferences.mm` 的 Command+F 也用它；另外测试会依赖跑测试的机器装的是什么布局。
- 只改 Control+Shift+F 和 Option+Shift+H 两个报告里提到的快捷键。改动更小，但其余字母快捷键会留在物理位置，同一个输入法里一半快捷键跟布局、一半不跟，Dvorak 用户仍会被 Control+Shift+E 等误触发。
- 布局字符不是字母时退回物理位置。这样能多认一些合成事件，但 Dvorak 上物理 E 键（打出 `.`）和标着 E 的键会同时触发 Control+Shift+E。

## Consequences

Dvorak、Colemak 等布局的用户按标着对应字母的键就能用这些快捷键，原来误触发的物理位置不再触发；美式和 AZERTY 等这些字母位置相同的布局不受影响。代价是这些快捷键依赖事件带着正确的 `charactersIgnoringModifiers`；带空字符的事件退回旧的物理位置，不会比原来更差。macOS 设置页和帮助里写的快捷键名称不变。

## Verification

`shortcut` 原生测试的 `TestShortcutLetterFollowsKeyboardLayout` 覆盖 26 个字母键在美式布局下的大小写、空字符和非拉丁字符退回物理位置，以及 Dvorak 位置上的字母和标点；`TestCharacterSetShortcut`、`TestDedicatedEnglish`、`TestFullWidth`、`TestScreenKeyboardShortcut`、`TestMaintenanceShortcuts` 各自加了 Dvorak 位置的按键：标着该字母的键触发，原来的物理位置不再触发。`ModeKey` 测试辅助现在给字母键带上美式布局打出的字母，和真实事件一致。没有在装了 Dvorak 布局的真机上实测。
