# Agent Note: Android 双拼键提示在会话建好前按上次的方案画

Status: implemented

## Problem

双拼键盘换应用或换到同一应用的另一个页面时，键盘先画出没有提示的字母，过一会儿提示出现，整排字母往上一跳；收起再展开同一个输入框则没有这一跳（#6997 第 3 项的「加载卡顿」）。

提示只从会话的 view（`scheme`、`shuangpin_profile`）算。每个新输入框都经过 `onStartInput`：先 `stop(false)` 把 view 置空，再 `render()`，此时 `updateShuangpinKeyHints()` 拿到空方案，对每个字母键 `setHintText("")`，`KeyHintButton` 把下边距还原，字母居中。会话在后台建（`scheduleEngineStartup` 先在工作线程上同步个人词库、读实时偏好，再回主线程 `startEngineSession`），一两秒后 view 到达，`render()` 给每个键设上提示、上边距 0、下边距 11dp，整组重新布局。收起再展开只走 `finishInputViewPresentation`，不停会话，view 和提示都还在。九键布局有过同样的问题，靠 `touchLayoutHint` 解决，双拼提示没有跟上。

## Decision

照搬 `touchLayoutHint` 的做法，只改 `MSIMEInputService.updateShuangpinKeyHints()` 和 `ShuangpinKeyHintPolicy` 里两个纯函数：

- 有 view 时记下 `ShuangpinKeyHintPolicy.rememberedProfile(scheme, profile)`：双拼时是方案名，其他方案为空。和布局写在同一个 `keyboard-layout-hint` SharedPreferences 里（键 `shuangpin_profile`），只在变化时 `apply()`。本地模式和英文不影响记下的值，它们只决定此刻画不画。
- 没有 view 时用 `profileWithoutView(sessionExpected, remembered)`：会话仍在（光标移动取消组字后 view 暂时清空）或有一次启动在路上时，按记下的方案、scheme 1、本地模式 none 画；否则不画。
- 「启动在路上」用 `pendingEngineStartup` 判断：`scheduleEngineStartup` 记下这次的代次，`stop()` 清掉；只有 `session == 0` 且代次仍等于 `engineStartGeneration` 时才算。建会话失败（`startEngineSession` 的 catch）、输入连接失败（`fail()`）都经过 `stop()`，换输入框、收起、销毁都会换代，所以只能直接输入的键盘不会画出提示。密码框、数字框不建会话，也不画。

用户关掉提示的开关 `touch_shuangpin_key_hints` 在键盘进程启动时就从皮肤片段（`readSkinHint` → `applyTouchGeometry`）恢复，所以关掉提示的人在空窗里也不会看到提示闪一下。

### 九键首帧与底行的一致性

#7780 的截图里主键区是九键，底行却多出仅属于 26 键的句号。布局缓存用于会话尚未创建时；一旦 `view` 到达，底行原先独立读取可变快照，而主键区要等 `rebuildKeyRows` 才更换。`ImeLetterRows` 现在在构建开始时记住绘制布局，`ImeBottomRow` 的键位和数字层底行归属都读取这一布局；新快照不能让底行提前换成另一个键盘。真正切换方案仍经原有的重建路径，两部分同时采用新布局。

`MSIMEInputService.apply` 是否重建键区同样比较 `drawnTouchLayout()` 与新快照，而不是比较两份会话快照。#8321 的九键长按设备回归复现了旧快照仍为 26 键、实际键区和新快照都已是九键的时序；第一次退格却误判布局变化，把正在按住的删除键摘下，导致长按提示没有可用的锚点。与已绘制布局一致时保留现有键，真正变更布局或标点键面仍重建。相关提示行为见[快速删除笔记](../feature/2026-10-08-android-backspace-quick-delete.md)。

首次显示的回归使用独立包名的合成编辑器，允许强停编辑器而不终止测试执行进程。测试在打开和点击输入框之前开始观察，无障碍树里九键出现时不得同时出现 26 键底行句号，并重复五次强停与打开。不保存输入内容或截图。

## Alternatives considered

- 用 `!runtimeOptionsBase.isEmpty()` 判断会话在起：`runtimeOptionsBase` 在 `stop()` 和建会话失败时都不清空，失败后的直接输入键盘会一直画着双拼提示。
- 只在内存里记方案、不写 SharedPreferences：换应用、换页面时键盘进程通常还活着，内存就够；但进程被回收后的第一次弹出仍会跳。布局提示已经在同一个文件里，多存一个键不多读一次盘。
- 把 `NativeClient.create` 挪到工作线程、缩短空窗本身：每种方案都受益，但要重排会话建立的线程模型，改动远大于这一跳。

- 只在会话准备时隐藏底行句号：直接对症，但不能约束其它自带底行的布局，且会掩盖主键区与底行读取不同状态的问题。让底行跟随实际构建的键区覆盖这一状态边界，不改变标点配置。

## Consequences

双拼用户换应用、换页面时第一帧就带着提示和下边距，会话到达时提示字符串相同，`setHintText` 不再改边距，没有重新布局。在设置里把双拼换成别的方案后，第一个输入框会先按旧方案画提示，会话到达后去掉，反方向跳一次；这和 `touchLayoutHint` 的取舍相同。

这次只去掉了提示造成的那一跳。会话到达时还有别的变化（`updateSymbolKeyFaces` 换 ，。 键面，以及主线程上的 `NativeClient.create`、`readRuntimeOptions` 和每次 `onStartInput` 的整组 `rebuildKeyRows`）；用户说的「卡顿」如果有一部分是真正的掉帧，这里只解决了一部分。准备状态行的首帧高度问题由[隐藏引擎启动状态](2026-10-17-android-hide-engine-startup-status.md)处理，设置里隐藏双拼提示的开关仍不在本次范围内。

## Verification

`ShuangpinKeyHintPolicySmoke` 新增 `rememberedProfile` 和 `profileWithoutView` 的用例，改动前编译失败（方法不存在），改动后通过；`check-host.sh` 加了一道检查，`updateShuangpinKeyHints` 必须经 `profileWithoutView` 处理没有 view 的情况。没有在设备上测换应用时的第一帧，也没有用 systrace 量主线程耗时；HarmonyOS 是否有同样的空窗没有检查。

API 28 与 API 35 的修复 APK 均通过安装、词库准备、核心打字删除和首帧专项（五次强停合成编辑器后重开）。API 28 专项暂时停用 AVD 自带的崩溃键盘，结束后恢复。首帧回归在 API 35 的修复前代码上没有重现截图中的短暂句号，因此它是持续的一致性检查，不作为已复现设备问题的证据。修复约束主键区与底行采用同一绘制布局，vivo API 30 的实际表现仍需报告者确认。
