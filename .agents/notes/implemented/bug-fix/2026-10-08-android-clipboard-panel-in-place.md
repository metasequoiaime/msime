# Agent Note: Android 剪贴板面板的确认和操作画在面板里

Status: implemented

## Problem

#5605：点剪贴板面板顶行的「清空」，键盘直接消失，没有出现确认框；重新打开面板，那一条还在。

两个原因叠在一起。其一，`confirmClearClipboardHistory` 用 `new AlertDialog.Builder(this)`，`this` 是 `InputMethodService`。它没有 Activity 的窗口令牌，对话框的窗口加不上去，输入法进程随之结束，键盘被系统收起，清空根本没有执行。其二，就算清空执行了，`ImePanels.showClipboardHistory` 打开面板时会补读系统剪贴板（为了补上键盘进程不在时复制的内容），而系统剪贴板里还是刚才那一条，它会被重新记进历史，看上去仍是「清空不生效」。删除单条也有第二个问题。

## Decision

- 清空的确认画在面板顶行：点「清空」后顶行换成「清空全部历史（含固定项）？ 取消 清空」，分段在这时让出位置；换分段或重开面板都会撤销。确认后由 `MSIMEInputService.clearClipboardHistory` 执行。剪贴板面板 never 弹 `AlertDialog`。
- 每条剪贴板内容有一个身份：`ClipDescription.getTimestamp()`（读不到时为 0）加文字散列和长度，由 `ClipboardCapturePolicy.identity` 算出，只用来比较，不含文字。上一次处理过的身份存在 `:ime` 进程的 SharedPreferences `android-clipboard-capture`。
- `captureClipboard(trigger, announce)`：复制时的监听（`Trigger.COPIED`）一律记录；打开面板的补读（`Trigger.PANEL_OPENED`）只记录身份与上一次不同的那一条。交给共享存储之前就记下身份，存储拒收的内容也不会每次打开面板都重试。
- 清空和删除单条之后，把系统剪贴板当前那一条记为已处理（`forgetCurrentClip`）。
- `check-host.sh` 守住：清空和面板渲染路径里没有 `AlertDialog`/`PopupMenu`，捕获经过 `ClipboardCapturePolicy.captures`。

## Alternatives considered

- **给对话框设置输入法窗口的令牌**（`lp.token = keyboardRoot.getWindowToken()`，`TYPE_APPLICATION_ATTACHED_DIALOG`，AOSP LatinIME 的做法）— 改动最小，确认框还是系统样式；但对话框仍是可获得焦点的窗口，会把窗口焦点从编辑器拿走，WebView 一类的编辑器失焦就会收起键盘（见 #5653），用户要求的「清空时键盘不关闭」做不到。
- **清空时连系统剪贴板一起清掉**（`ClipboardManager.clearPrimaryClip()`）— 补读自然没东西可读；但这会动到用户在别处还要粘贴的内容，而用户点的是「清空剪贴板历史」。
- **补读时按文字去重**（历史里已有就不加）— 刚清空时历史是空的，挡不住；身份必须记在历史之外。

## Consequences

- **收益**：清空在键盘里完成，键盘不再被收起；清空和删除之后，系统剪贴板里剩下的那一条不会回来；键盘进程被回收后这份记忆仍在。
- **代价**：同一段文字再次复制时，只有复制时刻变了才算新的一条；系统给不出复制时刻（返回 0）的设备上，同样的文字清空后再复制，监听仍会记录，补读则不会。删除一条时系统剪贴板里若是别的内容，那一条也被记为已处理；面板开着就说明它在打开时已经补读过，不会因此漏记。

## Verification

`platforms/android/tests/clipboard/ClipboardCapturePolicySmoke.java`；`bash platforms/android/check-host.sh` 编译服务和面板代码（`-Werror`）并执行源码守卫。真机上清空、删除和重新打开面板的流程没有在设备上验证。
