# Agent Note: Android 键盘只在窗口显示时每秒重读偏好

Status: implemented

## Problem

#9916 报告 vivo Android 11 上水杉一天约 320 mAh 电量，几乎全记在「后台耗电」，分项里 98% 是 CPU。

Android 键盘的偏好轮询（`PreferencesReloader`）在 `startEngineSession` 里启动，每 1000 ms 一轮，直到 `stop()` 才停；而 `stop()` 只在会话结束时调用，也就是输入框失焦（`onFinishInput`）或服务销毁。键盘收起只走 `finishInputViewPresentation`，会话和轮询都留着。按返回键收起键盘后，大多数应用的输入框仍保有焦点，于是在用户读聊天记录、看网页的整段时间里，`:ime` 进程每秒做一轮：工作线程上 `NativeClient.loadPreferences` 读盘解析整份偏好文档、`CandidateSkinResolver` 为浅色和深色各解析一次主题，候选皮肤带图时还把每张图经 base64 取回并算摘要；主线程上 `applyPreferencesSnapshot` 复制几份 JSON、用 `NativeClient.updatePreferences` 把快照推给引擎。键盘窗口不可见，系统把这些全记为后台耗电。

## Decision

偏好轮询只在输入法窗口显示时运行：

- `onWindowHidden` 停掉轮询，`onWindowShown` 在已有会话时重新启动（`start` 立即读一次再接着每秒轮询）。
- 新字段 `imeWindowVisible` 记录窗口是否可见。`startEngineSession` 和 `refreshPreferencesOnInputView` 只在窗口可见时启动轮询。会话可能先于窗口建好（`onStartInput` 在 `onWindowShown` 之前），也可能在收起之后为隐私模式重建；这两种情况都交给 `onWindowShown` 来启动。

窗口可见时的行为不变：每秒一轮，修订号比较和组字中推迟应用照旧由 Rust 负责。

## Alternatives considered

- **按文件 mtime 跳过未变的一轮**：Windows 读 Vim 模式用的就是这个办法，能把可见期间的每一轮也降到几次 `stat`。没有采用：一轮快照的来源不止 `preferences.json`，还有本地设置文件、应用主题的季节种子、皮肤目录下的包和图片，要列全才能保证不漏更新，而漏掉一项就是「设置改了键盘不变」。窗口收起时这一轮本来就没有可画的东西，直接停掉更简单，也不用证明哪些文件需要盯。可见期间的开销要等真机抓到 trace 再决定是否值得做。
- **键盘收起时结束会话**：同样能停掉轮询，但再弹出时要重新建会话，回到首次唤起的路径，等于把这次要解决的卡顿换个地方；还会丢掉同一输入框里收起再弹出保留的数字层等状态。

## Consequences

- 键盘收起、输入框仍有焦点时，`:ime` 进程不再每秒醒来，这正是 #9916 截图里后台耗电的来源。窗口可见时的开销和以前一样。
- 收起期间在设置页改的偏好，要到下次窗口显示时才读到。`onWindowShown` 会立即读一次，读完之前的几十毫秒里键盘仍按旧快照工作。云剪贴板中继这类按偏好决定去向的功能（见 `2026-10-12-android-cloud-clipboard-relay.md`）原本就只靠每秒重读，最多慢一秒；现在差异只在窗口刚显示、首次读完之前，这段时间里用户来不及点开面板发请求。
- 跟随光标的候选栏空闲时自己收起窗口（`CaretCandidatePolicy.ownHide`）同样会触发 `onWindowHidden`/`onWindowShown`，轮询随之停、起，在窗口再显示时补读一次。

## Verification

`bash platforms/android/check-host.sh`：288 项 JVM 冒烟通过，247 份设备测试源码编译通过，宿主资源检查通过。本机剩余磁盘低于新构建门槛，可用的模拟器属于另一个会话，所以没有构建 APK，也没有在模拟器或报告者的 vivo 设备上测过耗电。验证办法是在真机上收起键盘、让输入框保持焦点，用 `adb shell top -p <:ime pid>` 或 `dumpsys batterystats` 对比改动前后 `:ime` 进程的 CPU 时间增量。#9916 的首次唤起约 1 秒卡顿不在本次范围内，仍需真机 trace 定位。
