# Agent Note: 鸿蒙手机上「设为默认输入法」改为引导长按 🌐

Status: implemented

## Problem

欢迎流程第一步、首页状态卡片、对话页的「点此切换」，在水杉不是当前输入法时都调用宿主的 `showInputMethodPicker`，鸿蒙宿主转给 `inputMethod.getSetting().showOptionalInputMethods()`。弹出来的是 OpenHarmony 自带的选择对话框（`com.ohos.inputmethodchoosedialog`）：粉色标题、绿色条目，没有任何样式，盖在页面左上角。仓库负责人两次指着它说「这个垃圾」。

第三方应用不能自己切换输入法：`inputMethod.switchInputMethod` 和 `switchCurrentInputMethodAndSubtype` 都要 `ohos.permission.CONNECT_IME_ABILITY`，这是系统权限。

## Decision

- 模拟器上实测：键盘下方系统画的 🌐（场景板的 `keyboardLeftButton`）单击没有反应，长按弹出系统界面自己的输入法菜单。菜单是圆角白卡片，列着华为输入法的各种键盘和「水杉输入法」，选中就切换。
- 共享界面新增 `keyboard/keyboard-switch-guide.tsx`：
  - `openKeyboardSwitchGuide()` 打开一个顶部引导卡片：「长按键盘左下角的 🌐，在弹出的菜单里选『水杉输入法』」，带一个自动聚焦的输入框，让系统键盘和 🌐 出现。
  - 水杉成为当前输入法后，卡片显示「已设为默认」，0.9 秒后自己关掉。
  - `KeyboardSwitchGuideHost` 挂在设置外壳和欢迎流程里，继承页面配色；没打开时什么也不画。
- 鸿蒙宿主在手机上（`mobile_settings !== false`）把首页动作和欢迎流程动作里的 `showInputMethodPicker` 换成 `openKeyboardSwitchGuide`，各个入口不用改。2in1 用实体键盘，键盘下方没有 🌐，照旧弹系统对话框。
- 从 🌐 菜单切换输入法不会触发宿主监听的 `imeChange`，模拟器上切成水杉后，引导和欢迎流程都没更新。所以：
  - `ImeSetupClient` 加可选的 `refresh()`；
  - 鸿蒙宿主新增桥接方法 `refreshSetupStatus`，调用原生的 `refreshSetupState`，读到变化时照常经 `msimeHarmonySetupChanged` 推给页面；
  - 引导打开且还没完成时每秒调一次。

## Alternatives considered

- **打开系统设置的输入法页让用户自己选。** `input_method_settings` 这个深链在模拟器上用 `aa start` 打开是空白页，真机上能不能用没法验证；而且要离开应用，回来还得自己找路。
- **保留系统对话框。** 它就是被嫌弃的那个东西。模拟器上的样式是不是和真机一样没法确认，但 🌐 菜单是系统界面本身，在哪都一样。
- **单击 🌐 就切换。** 实测单击没有反应，只能写长按。

## Consequences

- **收益**：「设为默认」不再弹那个粉绿列表，用户按引导在系统自己的菜单里选，选完引导自动消失、检查打勾。
- **代价**：用户要多做一步长按。键盘冷启动慢时，引导打开后键盘可能要等一会儿才出来，或者第一次没出来，要再点一下输入框（模拟器上见过键盘进程加载十几秒、第一次请求失败）。

## Verification

- `apps/desktop/tests/core/keyboard-switch-guide.test.tsx`：引导打开时聚焦输入框、写明长按 🌐；宿主报告水杉成为当前输入法后显示「已设为默认」并自动关闭；打开期间每秒请宿主重读，完成后停止。
- HarmonyOS 6.0.2 phone 模拟器：先用 `ime -s` 把当前输入法切成华为输入法，冷启动进欢迎流程第一步，点「设为默认」，出现引导和华为键盘；长按 🌐、选「水杉输入法」，引导在 1.5 秒内关闭，第一步两项都打勾。全程没有出现系统的选择对话框。
