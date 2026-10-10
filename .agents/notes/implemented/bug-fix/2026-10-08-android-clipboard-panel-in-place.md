# Agent Note: Android 剪贴板面板的确认和操作画在面板里

Status: implemented

## Problem

#5605：点剪贴板面板顶行的「清空」，键盘直接消失，没有出现确认框；重新打开面板，那一条还在。

两个原因叠在一起。其一，`confirmClearClipboardHistory` 用 `new AlertDialog.Builder(this)`，`this` 是 `InputMethodService`。它没有 Activity 的窗口令牌，对话框的窗口加不上去，输入法进程随之结束，键盘被系统收起，清空根本没有执行。其二，就算清空执行了，`ImePanels.showClipboardHistory` 打开面板时会补读系统剪贴板（为了补上键盘进程不在时复制的内容），而系统剪贴板里还是刚才那一条，它会被重新记进历史，看上去仍是「清空不生效」。删除单条也有第二个问题。

#5653：在 Via 这类用系统 WebView 的浏览器里，长按一条历史，「固定 / 删除」菜单闪一下，键盘随即被收起；Firefox 和普通应用里正常。菜单是 `PopupMenu`，它的弹出窗口可获得焦点，打开时编辑器所在窗口失去窗口焦点。录屏里菜单出现后键盘才收起，且只发生在 WebView 浏览器里，据此判断是 WebView 在窗口失焦时让输入框失焦、收起输入法，而 GeckoView 不这样做；这个机制是从录屏和差异推断的，没有在设备上抓日志确认。

## Decision

- 清空的确认画在面板顶行：点「清空」后顶行换成「清空全部历史（含固定项）？ 取消 清空」，分段在这时让出位置；换分段或重开面板都会撤销。确认后由 `MSIMEInputService.clearClipboardHistory` 执行。剪贴板面板 never 弹 `AlertDialog`。
- 每条剪贴板内容有一个身份：`ClipDescription.getTimestamp()`（读不到时为 0）加文字散列和长度，由 `ClipboardCapturePolicy.identity` 算出，只用来比较，不含文字。上一次处理过的身份存在 `:ime` 进程的 SharedPreferences `android-clipboard-capture`。
- `captureClipboard(trigger, announce)`：复制时的监听（`Trigger.COPIED`）一律记录；打开面板的补读（`Trigger.PANEL_OPENED`）只记录身份与上一次不同的那一条。共享存储有了答复之后才记下身份：收下或明确拒收（`add` 返回原因）都记，拒收的内容不会每次打开面板都重试；存储写不进去时 `add` 抛异常，这一条不记，下次补读再试。第一版在交给存储之前就记下，写入抛异常时这一条被当成处理过，之后的补读永远跳过它。
- 清空和删除单条之后，把系统剪贴板当前那一条记为已处理（`forgetCurrentClip`）。系统标为敏感（`EXTRA_IS_SENSITIVE`）的内容跳过：补读本来就不记录它，而身份里的散列和长度足以穷举还原短密码或验证码，不能为它落盘。
- 长按一条历史，操作（固定或取消固定、删除、编辑、分词、添加到常用语、发到云剪贴板、收起）画成紧贴在它下方的两行按钮（`ImePanels.renderClipboardItemActions`；后加的几项见 [存为常用语](../feature/2026-10-09-android-clipboard-to-phrases.md) 和 [在应用里编辑](../feature/2026-10-09-android-clipboard-edit.md)），执行由 `MSIMEInputService.setClipboardItemPinned` / `removeClipboardItem` 负责。剪贴板面板 never 弹 `PopupMenu`。
- `check-host.sh` 守住：清空、面板渲染和条目操作路径里没有 `new AlertDialog`/`new PopupMenu`，`manageClipboardItem` 不再出现，捕获经过 `ClipboardCapturePolicy.captures`，`forgetCurrentClip` 跳过敏感内容。

## Alternatives considered

- **给对话框设置输入法窗口的令牌**（`lp.token = keyboardRoot.getWindowToken()`，`TYPE_APPLICATION_ATTACHED_DIALOG`，AOSP LatinIME 的做法）— 改动最小，确认框还是系统样式；但对话框仍是可获得焦点的窗口，会把窗口焦点从编辑器拿走，WebView 一类的编辑器失焦就会收起键盘（见 #5653），用户要求的「清空时键盘不关闭」做不到。
- **弹出窗口设为不可获得焦点**（`PopupWindow.setFocusable(false)`，九键长按候选就是这样）— 不抢焦点，菜单仍浮在键盘上；但 `PopupMenu` 不暴露这个开关，要自己写一个菜单弹窗和它的定位、换肤、无障碍，面板里一行按钮更简单，也不会被键盘外框裁掉。
- **清空时连系统剪贴板一起清掉**（`ClipboardManager.clearPrimaryClip()`）— 补读自然没东西可读；但这会动到用户在别处还要粘贴的内容，而用户点的是「清空剪贴板历史」。
- **补读时按文字去重**（历史里已有就不加）— 刚清空时历史是空的，挡不住；身份必须记在历史之外。

## Consequences

- **收益**：清空和长按操作都在键盘里完成，任何编辑器里键盘都不再被收起；清空和删除之后，系统剪贴板里剩下的那一条不会回来；键盘进程被回收后这份记忆仍在。
- **代价**：同一段文字再次复制时，只有复制时刻变了才算新的一条；系统给不出复制时刻（返回 0）的设备上，同样的文字清空后再复制，监听仍会记录，补读则不会。删除一条时系统剪贴板里若是别的内容，那一条也被记为已处理；面板开着就说明它在打开时已经补读过，不会因此漏记。

## Verification

`platforms/android/tests/clipboard/ClipboardCapturePolicySmoke.java`；`bash platforms/android/check-host.sh` 编译服务和面板代码（`-Werror`）并执行源码守卫。真机上清空、删除、长按操作和重新打开面板的流程，以及 Via 浏览器里的表现，没有在设备上验证。
