# Agent Note: 空输入框的退格交给编辑器处理引用

Status: implemented

## Problem

#7813 报告 QQ 左滑引用消息后，未输入文字时删除键不能移除艾特与引用。无组字、无选区时，Android 宿主只调用 `deleteSurroundingTextInCodePoints(1, 0)`；这只处理光标前的文字，空输入框外的引用控件不会收到退格事件。返回成功也不意味着界面中一定删掉了文字，不能据此推断引用已移除。

## Decision

保留组字由引擎消耗、选区以空串替换的优先级。无选区时询问光标前两个 UTF-16 单元：明确为空才把一对 `KEYCODE_DEL` down/up 交给当前编辑器，由应用决定是否取消引用；存在文字或返回 `null` 时仍按码位删除，不额外发送事件，避免双重删除和拆开代理对。转交按键时作废选区回声预期，不猜测外部应用如何变更界面。读取两个单元同时支持[按压内左右滑动恢复](../feature/2026-10-23-android-backspace-scrub.md)记录完整末尾码位；判空和按码位删除的分支保持一致。

## Historical audit

[删除键先删选区](../../implemented/bug-fix/2026-10-09-android-backspace-deletes-selection.md)部分重叠，仍保留以 `commitText` 删除选区及不普遍发送退格事件的理由；本篇只处理没有可删前文的情况。[删除键快速清空](../../implemented/feature/2026-10-08-android-backspace-quick-delete.md)不变，批量文本删除不触发外部引用移除。其余 QQ 检索命中是应用标识、复制与通知，无相同删除契约。

## Alternatives considered

- 总是发送 `KEYCODE_DEL`：引用、选区、普通文字都可交给应用，但依赖每个编辑器的按键实现，会丢掉既有 InputConnection 的码位删除与选区修复；只在没有前文可删时使用。
- 删除 API 返回假时再发送按键：少一次读取，但空框可返回成功且没有任何效果，无法覆盖本报告；异步 InputConnection 的成功也不能证明编辑器改动了内容。
- 用缓存选区位置判断光标在开头：免去一次 IPC，但可能落后于应用主动改动，按下时读实际前文更可靠；读取失败不当作空框。

## Verification

独立合成编辑器使用真实系统 InputConnection，在输入框外放置合成引用状态，只响应退格按键。相同 core 与资源、仅 Java 修复前后的 APK 对照：API 35 旧包在初始空框移除艾特步骤失败，新包通过；拼音版 API 28/35 均检查连续退格移除艾特和引用、组字只删拼音、中文与代理对按码位删除、选区优先、光标位于有文字输入框开头时不删后文、前文读取为 `null` 时不转交按键。full 版 API 35 也通过同一套件。

`smoke.sh` 的 full 默认设备矩阵注册该回归；API 28 可单独运行该 instrumentation，不依赖 `--core` 跳过的其他面板。测试支持完整版与拼音版，拼音版同样通过真实按钮的无障碍动作进入产品 InputConnection，避开 API 35 对键盘坐标注入的限制。空 EditText 的无障碍文字 `null` 与 InputConnection 读取失败的 `null` 分开处理；选区已到目标时不要求没有变化的动作返回成功。

arm64 native 库按当前基线重建，16 KiB 对齐与依赖检查通过；full/pinyin 原生 Gradle release、签名、锁定资源哈希与 ABI 核验通过。230 个 JVM 冒烟、96 个设备源文件编译、quick 与笔记门禁通过。实际 QQ 未在专用 AVD 安装，不把合成契约回归称为 QQ 真机验收。

## Consequences

无组字时每次退格多一次前文读取 IPC；空框按键可被编辑器忽略，应用行为没有统一 API 可保证。技术契约参考 [Android InputConnection](https://developer.android.com/reference/android/view/inputmethod/InputConnection)。
