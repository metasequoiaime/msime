# Agent Note: 「中／英」提示换肤后用匿名动态文字颜色

Status: implemented

## Problem

macOS 的「中／英」提示（`MSIMEInputModeHUDPanel`）由 `InputController.mm` 每次显示前按当前皮肤交出底色、边框和字色。这三者都是 `MSIMEThemedSkinColor` 造出来的动态 `NSColor`，名字分别是 `MSIMEInputModeHUDSurface`、`MSIMEInputModeHUDBorder`、`MSIMEInputModeHUDText`。

输入法进程保持运行、在设置里换皮肤以后，提示的底色和边框会跟着换，文字颜色却可能还是进程第一次显示提示时那一款皮肤的颜色：深底皮肤配深色字，几乎看不清（`#11869`）。

原因是 AppKit 对具名动态颜色的判等：`+colorWithName:dynamicProvider:` 交出的对象按名称比较，两个同名、但 light/dark 值完全不同的动态颜色 `isEqual:` 为真。`NSTextField` 拿到相等的文字颜色就不再更新自己的文字属性。报告者的离屏 AppKit 测试复现了这一点：底色已经是新皮肤的 `#23262E`，字色仍是 `#1A1A1A`，预期是 `#F0B90B`。

本机用同样的 AppKit 调用验证（`NSColor colorWithName:` + `NSTextField setTextColor:`，在 `performAsCurrentDrawingAppearance:` 里读回颜色）：同名的两个动态颜色 `isEqual:` 为真，换色后 `NSTextField` 读回的还是第一份（`0.1`），第二份（`0.94`）没有生效；把名字换成 `nil` 后两份不再相等，读回的是第二份。

## Decision

同一套颜色由面板自己造：`InputModeHUDPanel.h` 新增 `MSIMEInputModeHUDTextColor(lightColor, darkColor)`，用 `[NSColor colorWithName:nil dynamicProvider:]` 按浅色/深色外观各自解析。`InputController.mm` 的 `presentHUDText:announcement:` 改为传 `MSIMEInputModeHUDTextColor(SkinColor(light.text), SkinColor(dark.text))`，不再传具名的 `MSIMEThemedSkinColor(@"MSIMEInputModeHUDText", ...)`；换肤后每次都给 `NSTextField` 一个「不相等」的新颜色。

底色和边框保留原名：它们经 `CGColor` 交给图层的 `backgroundColor` / `borderColor`，不走 `NSTextField` 的判等，也没有观察到换肤不生效。

`InputModeHUDPanelTest` 加了一条回归：用两份不同的皮肤字色依次交给面板，`MSIMEInputModeHUDTextColor` 的两次结果必须不相等，并在 Aqua 里读回 `label.textColor` 依次是第一份、第二份。把实现改回具名颜色时这条会红（本机试过）。

`MSIMEInputModeHUDText` 这个名字仍然属于「中」「英」两个提示文字（`InputModeHUDPanel.mm`），与颜色名的移除无关。

## Alternatives considered

**在 `MSIMEInputModeHUDPanel` 里把字色解析成具体颜色再赋值。** 能绕开判等，但 `applyThemeColors` 每次运行在 `effectiveAppearance` 的绘图环境里，解析成具体值后，系统外观在提示显示期间改变时不再随动态颜色自动更新；提示只显示 0.6 秒，但这是把一条本来正确的行为换掉。而且面板层的改动会让所有颜色都变成具体值，不只文字。

**继续用具名颜色，在面板里比较名称后强制重建文字属性。** 需要知道颜色的名字，而 `NSColor.colorName` 不在公开 API 里能可靠拿到自定义动态颜色的名字，只能靠调用方另外传一份字符串，比直接改调用处更绕。

## Consequences

- 换肤后「中／英」提示的字色与底色一起更新；浅色/深色外观切换仍由动态颜色各自解析。
- 悬浮工具栏与键盘图等其他皮肤颜色的取用路径没有改动。
- 验证边界：本机直接编译并运行了 `InputModeHUDPanelTest`（`clang++ -std=c++17 -fobjc-arc`，链接 `InputModeHUDPanel.mm`），并确认把 `MSIMEInputModeHUDTextColor` 改回具名颜色时新断言会失败；`InputController.mm` 的编译由 macOS CI 覆盖，画面复验留给 `#11869` 的报告者用下一版安装包确认。
