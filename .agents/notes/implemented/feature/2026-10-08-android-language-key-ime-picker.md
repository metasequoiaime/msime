# Agent Note: Android 长按中/英弹出输入法选择框

Status: implemented

## Problem

#5615：用户常要临时换到 KeePassDX 之类密码管理器自带的键盘。Android 9 起键盘里不放地球键（`offersGlobeKey` 只在 API 28 以下为真，理由是系统导航栏有切换按钮），可全面屏手势导航下那个按钮不一定在，键盘里就没有任何地方能换输入法。

## Decision

- `MSIMEInputService.bindInputMethodPicker(Button)` 给按钮挂长按：播按键反馈后调用 `InputMethodManager.showInputMethodPicker()`，并给读屏的长按动作起名「切换输入法」。挂在底栏的 `languageButton`（九键、注音、日语等布局复用同一个实例）、`globeButton`，以及日语九键侧列自建的「英」和「切换」上。
- 弹出选择框时不结束组字。用户可能只是看一眼就关掉；真的换了输入法时 `onFinishInput` → `stop(true)` 照常收尾。
- `languageButton` 不再在 `session == 0` 时禁用。禁用的 View 收不到长按，而密码框（不建会话）正是最需要换到密码管理器键盘的地方；点按在没有会话时仍由 `toggleInputLanguage` 直接忽略。

## Alternatives considered

- **长按时先结束组字，和地球键的 `switchToNextInputMethodAfterCommit` 一样** — 地球键是立即切走，必须先收尾；选择框可能被取消，提前上屏会把用户还没确认的组字写进文档。
- **在工具栏或功能面板加一个「切换输入法」入口** — 不占键位，也不改中/英的状态；但多一层点按，issue 要的就是在键上长按，主流键盘（Gboard 的地球键、搜狗的中/英）也是这个手势。
- **没有会话时保持禁用、改用透明度画成灰色** — 能保留「点按无效」的视觉提示，但要在中/英键上另起一套样式通道，而密码框里中/英本来就画着「英」，点按无效不会误导。

## Consequences

- **收益**：任何布局、任何输入框里长按中/英都能换输入法，不依赖系统导航栏的按钮。
- **代价**：密码框和会话启动的那一瞬间，中/英键看着可点、点按却什么也不做，并且这次点按仍计入按键统计。
- **验证**：`check-host.sh` 守住中/英键的长按绑定、`showInputMethodPicker` 调用、日语九键侧列的绑定，并禁止 `setEnabled(languageButton, session != 0)` 回来；真机上的选择框弹出没有在设备上验收。
